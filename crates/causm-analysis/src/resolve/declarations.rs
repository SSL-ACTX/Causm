use crate::analyzer::{EntropicAnalyzer, RoutineInfo};
use causm_core::{Program, SpannedStatement, Statement};

/// Pre-register all top-level declarations (types, interfaces, routines, state
/// constants) into the analyzer's symbol tables before semantic analysis begins.
///
/// This is a forward-scan pass: it does not perform full type-checking but
/// ensures every name is visible to all later expression analysis, regardless
/// of textual declaration order.
pub fn pre_register_program_declarations(
    analyzer: &mut EntropicAnalyzer,
    program: &Program,
) {
    fn visit_stmts(analyzer: &mut EntropicAnalyzer, stmts: &[SpannedStatement]) {
        for stmt in stmts {
            match &stmt.stmt {
                Statement::TypeDecl {
                    name,
                    extends,
                    fields,
                    decay_after_ms,
                    auto_drop,
                    scoped_branch,
                } => {
                    let _ = analyzer.TypeDecl(
                        name,
                        extends,
                        fields,
                        decay_after_ms,
                        auto_drop,
                        scoped_branch,
                    );
                    if let Some(dot_idx) = name.rfind('.') {
                        let bare_name = &name[dot_idx + 1..];
                        let _ = analyzer.TypeDecl(
                            bare_name,
                            extends,
                            fields,
                            decay_after_ms,
                            auto_drop,
                            scoped_branch,
                        );
                    }
                }
                Statement::InterfaceDecl {
                    name,
                    extends,
                    methods,
                } => {
                    let _ = analyzer.InterfaceDecl(name, extends, methods);
                }
                Statement::RoutineDef {
                    name,
                    params,
                    return_type,
                    taking_ms,
                    state_constraint,
                    required_capabilities,
                    ..
                } => {
                    let preliminary_params = params
                        .iter()
                        .map(|p| {
                            let mut param_type = p
                                .typ
                                .as_ref()
                                .map(causm_core::types::Type::from_typename)
                                .unwrap_or(causm_core::types::Type::Unknown);
                            if p.name == "self" && p.typ.is_none() {
                                if let Some(dot_idx) = name.find('.') {
                                    let struct_name = &name[..dot_idx];
                                    param_type = causm_core::types::Type::Custom(
                                        struct_name.to_string(),
                                    );
                                }
                            }
                            (p.mode.clone(), p.name.clone(), param_type)
                        })
                        .collect();
                    let r_info = RoutineInfo {
                        params: preliminary_params,
                        return_type: return_type
                            .as_ref()
                            .map(causm_core::types::Type::from_typename)
                            .unwrap_or(causm_core::types::Type::Unknown),
                        taking_ms: taking_ms.unwrap_or(0),
                        state_constraint: state_constraint.clone(),
                        required_capabilities: required_capabilities.clone(),
                    };
                    analyzer.routines.insert(name.clone(), r_info.clone());
                    // Strip generic angle brackets for monomorphised lookup.
                    let base_name = if let Some(angle_idx) = name.find('<') {
                        if let Some(dot_idx) = name.find('.') {
                            let struct_part = &name[..angle_idx];
                            let method_part = &name[dot_idx..];
                            format!("{}{}", struct_part, method_part)
                        } else {
                            name.clone()
                        }
                    } else {
                        name.clone()
                    };
                    if base_name != *name {
                        analyzer.routines.insert(base_name, r_info);
                    }
                }
                Statement::Isolate(iso) => {
                    visit_stmts(analyzer, &iso.body);
                }
                Statement::StateDecl {
                    target,
                    var_type,
                    expr,
                } => {
                    let typ = if let Some(explicit) = var_type {
                        causm_core::types::Type::from_typename(explicit)
                    } else if let Ok(inferred) =
                        crate::expression::infer_expression_type(analyzer, expr)
                    {
                        inferred
                    } else {
                        causm_core::types::Type::Unknown
                    };
                    let branch = analyzer.branch_contexts.get_mut("main").unwrap();
                    branch.mutables.insert(target.clone());
                    branch.types.insert(target.clone(), typ);
                    branch.produced.insert(target.clone());
                }
                Statement::RelativisticBlock { body, .. } => {
                    visit_stmts(analyzer, body);
                }
                _ => {}
            }
        }
    }

    for block in &program.timelines {
        visit_stmts(analyzer, &block.statements);
    }
}

/// Thin public alias kept for backward compatibility with call-sites inside
/// `analyzer/core.rs` that still use `self.pre_register_program_declarations`.
pub fn run_resolve_stage(analyzer: &mut EntropicAnalyzer, program: &Program) {
    pre_register_program_declarations(analyzer, program);
}
