use causm_core::{Program, SpannedStatement, Statement};
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub mod expressions;
pub mod statements;

#[derive(Parser)]
#[grammar = "causm.pest"]
pub struct CausmParser;

pub fn parse_causm(source: &str) -> anyhow::Result<Program> {
    let mut pairs = CausmParser::parse(Rule::program, source)?;
    let mut timelines = Vec::new();
    let mut standalone_stmts = Vec::new();

    if let Some(program_pair) = pairs.next() {
        for pair in program_pair.into_inner() {
            match pair.as_rule() {
                Rule::timeline_block => {
                    timelines.push(statements::parse_timeline_block(pair));
                }
                Rule::statement => {
                    if let Some(s) = pair.into_inner().next() {
                        standalone_stmts.push(statements::parse_statement(s));
                    }
                }
                _ => {}
            }
        }
    }

    if !standalone_stmts.is_empty() {
        timelines.insert(
            0,
            causm_core::TimelineBlock {
                time: causm_core::TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: standalone_stmts,
            },
        );
    }

    Ok(Program { timelines })
}

fn expand_spanned_statements(
    stmts: Vec<SpannedStatement>,
    base_dir: Option<&Path>,
    loaded_files: &mut HashSet<String>,
) -> anyhow::Result<Vec<SpannedStatement>> {
    let mut result = Vec::new();
    for spanned in stmts {
        match spanned.stmt {
            Statement::Import { path, alias } => {
                let (imported_source, sub_base_dir) = if let Some(embedded) =
                    causm_stdlib::get_module(&path)
                {
                    (embedded.to_string(), None)
                } else {
                    let target_path = if let Some(dir) = base_dir {
                        dir.join(&path)
                    } else {
                        PathBuf::from(&path)
                    };
                    let path_str = target_path.to_string_lossy().to_string();
                    if loaded_files.contains(&path_str) || !target_path.exists() {
                        continue;
                    }
                    loaded_files.insert(path_str);
                    (
                        std::fs::read_to_string(&target_path)?,
                        target_path.parent().map(|p| p.to_path_buf()),
                    )
                };
                let imported_prog = parse_causm(&imported_source)?;
                for imp_tl in imported_prog.timelines {
                    let expanded = expand_spanned_statements(
                        imp_tl.statements,
                        sub_base_dir.as_deref(),
                        loaded_files,
                    )?;
                    for imp_spanned in expanded {
                        let item_stmts =
                            if let Statement::Isolate(iso) = imp_spanned.stmt {
                                iso.body
                            } else {
                                vec![imp_spanned]
                            };
                        for s in item_stmts {
                            result.push(s.clone());
                            if let Some(ref ns) = alias {
                                if let Statement::RoutineDef {
                                    name,
                                    params,
                                    return_type,
                                    taking_ms,
                                    state_constraint,
                                    body,
                                } = &s.stmt
                                {
                                    let qualified_name = format!("{}.{}", ns, name);
                                    result.push(SpannedStatement {
                                        stmt: Statement::RoutineDef {
                                            name: qualified_name,
                                            params: params.clone(),
                                            return_type: return_type.clone(),
                                            taking_ms: *taking_ms,
                                            state_constraint: state_constraint
                                                .clone(),
                                            body: body.clone(),
                                        },
                                        span: s.span.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Statement::FromImport { path, symbols } => {
                let (imported_source, sub_base_dir) = if let Some(embedded) =
                    causm_stdlib::get_module(&path)
                {
                    (embedded.to_string(), None)
                } else {
                    let target_path = if let Some(dir) = base_dir {
                        dir.join(&path)
                    } else {
                        PathBuf::from(&path)
                    };
                    let path_str = target_path.to_string_lossy().to_string();
                    if loaded_files.contains(&path_str) || !target_path.exists() {
                        continue;
                    }
                    loaded_files.insert(path_str);
                    (
                        std::fs::read_to_string(&target_path)?,
                        target_path.parent().map(|p| p.to_path_buf()),
                    )
                };
                let imported_prog = parse_causm(&imported_source)?;
                for imp_tl in imported_prog.timelines {
                    let expanded = expand_spanned_statements(
                        imp_tl.statements,
                        sub_base_dir.as_deref(),
                        loaded_files,
                    )?;
                    for imp_spanned in expanded {
                        let item_stmts =
                            if let Statement::Isolate(iso) = imp_spanned.stmt {
                                iso.body
                            } else {
                                vec![imp_spanned]
                            };
                        for s in item_stmts {
                            result.push(s.clone());
                            for (sym_name, sym_alias) in &symbols {
                                if let Some(alias_name) = sym_alias {
                                    if let Statement::RoutineDef {
                                        name,
                                        params,
                                        return_type,
                                        taking_ms,
                                        state_constraint,
                                        body,
                                    } = &s.stmt
                                    {
                                        if name == sym_name {
                                            result.push(SpannedStatement {
                                                stmt: Statement::RoutineDef {
                                                    name: alias_name.clone(),
                                                    params: params.clone(),
                                                    return_type: return_type.clone(),
                                                    taking_ms: *taking_ms,
                                                    state_constraint:
                                                        state_constraint.clone(),
                                                    body: body.clone(),
                                                },
                                                span: s.span.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Statement::Isolate(mut iso) => {
                iso.body =
                    expand_spanned_statements(iso.body, base_dir, loaded_files)?;
                result.push(SpannedStatement {
                    stmt: Statement::Isolate(iso),
                    span: spanned.span,
                });
            }
            _ => result.push(spanned),
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
        timeline.statements =
            expand_spanned_statements(original_stmts, base_dir, &mut loaded_files)?;
    }

    Ok(program)
}
