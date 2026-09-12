use causm_core::{Program, SpannedStatement, Statement};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod arena_parser;
pub mod lexer;
pub mod pratt;
pub mod registry;

static GLOBAL_ARCHIVE: std::sync::LazyLock<
    std::sync::Mutex<causm_stdlib::archive::CsaArchive>,
> = std::sync::LazyLock::new(|| {
    std::sync::Mutex::new(
        causm_stdlib::archive::CsaArchive::get_or_load_standard_archive(),
    )
});

fn get_or_register_module(path: &str, source: &str) -> anyhow::Result<Program> {
    {
        let store = registry::global_module_store().read().unwrap();
        if let Some(id) = store.get_by_path(path).map(|m| m.id) {
            if let Some(prog) = store.get_module_ast(id) {
                return Ok(prog);
            }
        }
    }

    {
        let archive = GLOBAL_ARCHIVE.lock().unwrap();
        if let Some(ast_bytes) = archive.get_ast(path) {
            if let Ok(prog) = postcard::from_bytes::<Program>(ast_bytes) {
                return Ok(prog);
            }
        }
    }

    let mut store = registry::global_module_store().write().unwrap();
    if let Some(id) = store.get_by_path(path).map(|m| m.id) {
        if let Some(prog) = store.get_module_ast(id) {
            return Ok(prog);
        }
    }
    let id = store
        .get_or_parse_module(path, source)
        .map_err(|e| anyhow::anyhow!("failed parsing module '{}': {}", path, e))?;
    let prog = store.get_module_ast(id).ok_or_else(|| {
        anyhow::anyhow!("module '{}' registered but ast projection failed", path)
    })?;

    if let Ok(ast_bytes) = postcard::to_allocvec(&prog) {
        let mut archive = GLOBAL_ARCHIVE.lock().unwrap();
        archive.insert_ast(path, ast_bytes);
        let _ = archive.save_to_disk(None);
    }

    Ok(prog)
}

pub fn parse_causm(source: &str) -> anyhow::Result<Program> {
    arena_parser::lower::parse_arena_program_to_ast(source)
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn parse_causm_to_hir(source: &str) -> anyhow::Result<causm_core::HirProgram> {
    arena_parser::lower::parse_arena_program_to_hir(source)
        .map_err(|e| anyhow::anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::parse_causm;
    use causm_core::Statement;

    #[test]
    fn test_actor_and_send_syntax_parses() {
        let program = parse_causm(
            "actor FlightController { require System.Log on FlightCommand::SetThrottle taking 5ms { print(1) } } send(consume cmd) to FlightController",
        )
        .unwrap();

        assert!(program.timelines.iter().any(|tl| {
            tl.statements
                .iter()
                .any(|stmt| matches!(stmt.stmt, Statement::Isolate(_)))
        }));
        assert!(program.timelines.iter().any(|tl| {
            tl.statements
                .iter()
                .any(|stmt| matches!(stmt.stmt, Statement::Send { .. }))
        }));
    }
}

fn tag_stdlib(mut s: SpannedStatement) -> SpannedStatement {
    use causm_core::{Attribute, AttributeKind, Span};
    let already_tagged = s.attributes.iter().any(|a| {
        matches!(&a.kind, AttributeKind::Custom { name, .. } if name == "stdlib_internal")
    });
    if !already_tagged {
        s.attributes.push(Attribute {
            kind: AttributeKind::Custom {
                name: "stdlib_internal".to_string(),
                args: Vec::new(),
            },
            span: Span { start: 0, end: 0 },
        });
    }
    s
}

fn expand_spanned_statements(
    stmts: Vec<SpannedStatement>,
    base_dir: Option<&Path>,
    loaded_files: &mut HashSet<String>,
    is_stdlib: bool,
) -> anyhow::Result<Vec<SpannedStatement>> {
    let mut result = Vec::new();
    for spanned in stmts {
        match spanned.stmt {
            Statement::Import { path, alias } => {
                let (imported_prog, sub_base_dir, sub_is_stdlib) =
                    if let Some(embedded) = causm_stdlib::get_module(&path) {
                        let mod_key = format!("embedded::{}::as::{:?}", path, alias);
                        if loaded_files.contains(&mod_key) {
                            continue;
                        }
                        loaded_files.insert(mod_key);
                        (get_or_register_module(&path, embedded)?, None, true)
                    } else {
                        let target_path = if let Some(dir) = base_dir {
                            dir.join(&path)
                        } else {
                            PathBuf::from(&path)
                        };
                        let path_str = target_path.to_string_lossy().to_string();
                        if loaded_files.contains(&path_str) || !target_path.exists()
                        {
                            continue;
                        }
                        loaded_files.insert(path_str.clone());
                        let source = std::fs::read_to_string(&target_path)?;
                        let sub_base_dir =
                            target_path.parent().map(|p| p.to_path_buf());
                        (
                            get_or_register_module(&path_str, &source)?,
                            sub_base_dir,
                            false,
                        )
                    };

                let mut item_stmts = Vec::new();
                for imp_tl in imported_prog.timelines {
                    let expanded = expand_spanned_statements(
                        imp_tl.statements,
                        sub_base_dir.as_deref(),
                        loaded_files,
                        sub_is_stdlib,
                    )?;
                    item_stmts.extend(flatten_container_statements(expanded));
                }

                for s in item_stmts {
                    let s = if sub_is_stdlib { tag_stdlib(s) } else { s };
                    result.push(s.clone());
                    if let Some(ref ns) = alias {
                        match &s.stmt {
                            Statement::RoutineDef {
                                name,
                                params,
                                return_type,
                                taking_ms,
                                state_constraint,
                                required_capabilities,
                                body,
                            } => {
                                if !name.starts_with(&format!("{}.", ns)) {
                                    let qualified_name = format!("{}.{}", ns, name);
                                    let mut ns_stmt = SpannedStatement::new(
                                        Statement::RoutineDef {
                                            name: qualified_name,
                                            params: params.clone(),
                                            return_type: return_type.clone(),
                                            taking_ms: *taking_ms,
                                            state_constraint: state_constraint
                                                .clone(),
                                            required_capabilities:
                                                required_capabilities.clone(),
                                            body: body.clone(),
                                        },
                                        s.span.clone(),
                                    );
                                    if sub_is_stdlib {
                                        ns_stmt = tag_stdlib(ns_stmt);
                                    }
                                    result.push(ns_stmt);
                                }
                            }
                            Statement::ForeignBlock {
                                lib_name,
                                abi,
                                routines,
                            } => {
                                let qualified_routines =
                                    routines
                                        .iter()
                                        .map(|r_spanned| {
                                            if let Statement::RoutineDef {
                                                name,
                                                params,
                                                return_type,
                                                taking_ms,
                                                state_constraint,
                                                required_capabilities,
                                                body,
                                            } = &r_spanned.stmt
                                            {
                                                let qualified_name = if !name
                                                    .starts_with(&format!("{}.", ns))
                                                {
                                                    format!("{}.{}", ns, name)
                                                } else {
                                                    name.clone()
                                                };
                                                SpannedStatement::new(
                                                    Statement::RoutineDef {
                                                        name: qualified_name,
                                                        params: params.clone(),
                                                        return_type: return_type
                                                            .clone(),
                                                        taking_ms: *taking_ms,
                                                        state_constraint:
                                                            state_constraint.clone(),
                                                        required_capabilities:
                                                            required_capabilities
                                                                .clone(),
                                                        body: body.clone(),
                                                    },
                                                    r_spanned.span.clone(),
                                                )
                                            } else {
                                                r_spanned.clone()
                                            }
                                        })
                                        .collect();
                                result.push(SpannedStatement::new(
                                    Statement::ForeignBlock {
                                        lib_name: lib_name.clone(),
                                        abi: abi.clone(),
                                        routines: qualified_routines,
                                    },
                                    s.span.clone(),
                                ));
                            }
                            Statement::TypeDecl {
                                name,
                                extends,
                                fields,
                                decay_after_ms,
                                auto_drop,
                                scoped_branch,
                            } => {
                                if !name.starts_with(&format!("{}.", ns)) {
                                    let qualified_name = format!("{}.{}", ns, name);
                                    result.push(SpannedStatement::new(
                                        Statement::TypeDecl {
                                            name: qualified_name,
                                            extends: extends.clone(),
                                            fields: fields.clone(),
                                            decay_after_ms: *decay_after_ms,
                                            auto_drop: auto_drop.clone(),
                                            scoped_branch: scoped_branch.clone(),
                                        },
                                        s.span.clone(),
                                    ));
                                }
                            }
                            Statement::EnumDecl { name, variants } => {
                                if !name.starts_with(&format!("{}.", ns)) {
                                    let qualified_name = format!("{}.{}", ns, name);
                                    result.push(SpannedStatement::new(
                                        Statement::EnumDecl {
                                            name: qualified_name,
                                            variants: variants.clone(),
                                        },
                                        s.span.clone(),
                                    ));
                                }
                            }
                            Statement::InterfaceDecl {
                                name,
                                extends,
                                methods,
                            } if !name.starts_with(&format!("{}.", ns)) => {
                                let qualified_name = format!("{}.{}", ns, name);
                                result.push(SpannedStatement::new(
                                    Statement::InterfaceDecl {
                                        name: qualified_name,
                                        extends: extends.clone(),
                                        methods: methods.clone(),
                                    },
                                    s.span.clone(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }
            Statement::FromImport { path, symbols } => {
                let (imported_prog, sub_base_dir, sub_is_stdlib) =
                    if let Some(embedded) = causm_stdlib::get_module(&path) {
                        (get_or_register_module(&path, embedded)?, None, true)
                    } else {
                        let target_path = if let Some(dir) = base_dir {
                            dir.join(&path)
                        } else {
                            PathBuf::from(&path)
                        };
                        let path_str = target_path.to_string_lossy().to_string();
                        if loaded_files.contains(&path_str) || !target_path.exists()
                        {
                            continue;
                        }
                        loaded_files.insert(path_str.clone());
                        let source = std::fs::read_to_string(&target_path)?;
                        let sub_base_dir =
                            target_path.parent().map(|p| p.to_path_buf());
                        (
                            get_or_register_module(&path_str, &source)?,
                            sub_base_dir,
                            false,
                        )
                    };

                let mut item_stmts = Vec::new();
                let mut sub_loaded_files = HashSet::new();
                for imp_tl in imported_prog.timelines {
                    let expanded = expand_spanned_statements(
                        imp_tl.statements,
                        sub_base_dir.as_deref(),
                        &mut sub_loaded_files,
                        sub_is_stdlib,
                    )?;
                    item_stmts.extend(flatten_container_statements(expanded));
                }

                let is_wildcard = symbols.iter().any(|(s, _)| s == "*");
                for s in item_stmts {
                    let s = if sub_is_stdlib { tag_stdlib(s) } else { s };
                    if is_wildcard {
                        result.push(s.clone());
                    } else {
                        for (sym_name, sym_alias) in &symbols {
                            match &s.stmt {
                                Statement::RoutineDef {
                                    name,
                                    params,
                                    return_type,
                                    taking_ms,
                                    state_constraint,
                                    required_capabilities,
                                    body,
                                } if name == sym_name
                                    || name
                                        .starts_with(&format!("{}.", sym_name)) =>
                                {
                                    let target_name = if let Some(alias) = sym_alias
                                    {
                                        if name == sym_name {
                                            alias.clone()
                                        } else {
                                            format!(
                                                "{}.{}",
                                                alias,
                                                &name[sym_name.len() + 1..]
                                            )
                                        }
                                    } else {
                                        name.clone()
                                    };
                                    let mut out_stmt = SpannedStatement::new(
                                        Statement::RoutineDef {
                                            name: target_name,
                                            params: params.clone(),
                                            return_type: return_type.clone(),
                                            taking_ms: *taking_ms,
                                            state_constraint: state_constraint
                                                .clone(),
                                            required_capabilities:
                                                required_capabilities.clone(),
                                            body: body.clone(),
                                        },
                                        s.span.clone(),
                                    );
                                    if sub_is_stdlib {
                                        out_stmt = tag_stdlib(out_stmt);
                                    }
                                    result.push(out_stmt);
                                }
                                Statement::TypeDecl {
                                    name,
                                    extends,
                                    fields,
                                    decay_after_ms,
                                    auto_drop,
                                    scoped_branch,
                                } if name == sym_name => {
                                    result.push(s.clone());
                                    if let Some(alias) = sym_alias {
                                        if alias != name {
                                            result.push(SpannedStatement::new(
                                                Statement::TypeDecl {
                                                    name: alias.clone(),
                                                    extends: extends.clone(),
                                                    fields: fields.clone(),
                                                    decay_after_ms: *decay_after_ms,
                                                    auto_drop: auto_drop.clone(),
                                                    scoped_branch: scoped_branch
                                                        .clone(),
                                                },
                                                s.span.clone(),
                                            ));
                                        }
                                    }
                                }
                                Statement::EnumDecl { name, variants }
                                    if name == sym_name =>
                                {
                                    result.push(s.clone());
                                    if let Some(alias) = sym_alias {
                                        if alias != name {
                                            result.push(SpannedStatement::new(
                                                Statement::EnumDecl {
                                                    name: alias.clone(),
                                                    variants: variants.clone(),
                                                },
                                                s.span.clone(),
                                            ));
                                        }
                                    }
                                }
                                Statement::InterfaceDecl {
                                    name,
                                    extends,
                                    methods,
                                } if name == sym_name => {
                                    result.push(s.clone());
                                    if let Some(alias) = sym_alias {
                                        if alias != name {
                                            result.push(SpannedStatement::new(
                                                Statement::InterfaceDecl {
                                                    name: alias.clone(),
                                                    extends: extends.clone(),
                                                    methods: methods.clone(),
                                                },
                                                s.span.clone(),
                                            ));
                                        }
                                    }
                                }
                                Statement::ForeignBlock { .. } => {
                                    result.push(s.clone());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Statement::Isolate(mut iso) => {
                iso.body = expand_spanned_statements(
                    iso.body,
                    base_dir,
                    loaded_files,
                    is_stdlib,
                )?;
                result.push(SpannedStatement::with_attributes(
                    Statement::Isolate(iso),
                    spanned.span.clone(),
                    spanned.attributes,
                ));
            }
            Statement::RelativisticBlock { time, body } => {
                let expanded_body = expand_spanned_statements(
                    body,
                    base_dir,
                    loaded_files,
                    is_stdlib,
                )?;
                result.push(SpannedStatement::new(
                    Statement::RelativisticBlock {
                        time,
                        body: expanded_body,
                    },
                    spanned.span,
                ));
            }
            _ => {
                let spanned = if is_stdlib {
                    tag_stdlib(spanned)
                } else {
                    spanned
                };
                result.push(spanned);
            }
        }
    }
    Ok(result)
}

pub fn parse_causm_with_imports(
    source: &str,
    base_dir: Option<&Path>,
) -> anyhow::Result<Program> {
    let mut program = parse_causm(source)?;
    let mut loaded_files = HashSet::new();

    for timeline in &mut program.timelines {
        let original_stmts = std::mem::take(&mut timeline.statements);
        timeline.statements = expand_spanned_statements(
            original_stmts,
            base_dir,
            &mut loaded_files,
            false,
        )?;
    }

    crate::macro_expand::expand_program(&mut program);
    crate::derive::expand_derives(&mut program);
    crate::hir::desugar_program(&mut program);
    Ok(program)
}

pub fn parse_causm_to_hir_with_imports(
    source: &str,
    base_dir: Option<&Path>,
) -> anyhow::Result<causm_core::HirProgram> {
    let program = parse_causm_with_imports(source, base_dir)?;
    Ok(crate::hir::lower_ast_to_hir(&program))
}

fn flatten_container_statements(
    stmts: Vec<SpannedStatement>,
) -> Vec<SpannedStatement> {
    let mut out = Vec::new();
    for s in stmts {
        match s.stmt {
            Statement::Isolate(iso) => {
                out.extend(flatten_container_statements(iso.body));
            }
            Statement::RelativisticBlock { body, .. } => {
                out.extend(flatten_container_statements(body));
            }
            _ => out.push(s),
        }
    }
    out
}
