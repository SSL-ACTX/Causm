use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::*;
use causm_frontend::parser;
use causm_plugin_sdk::PluginContext;
use causm_plugin_sdk::PluginError;
use causm_plugins::*;
use std::collections::HashSet;

// 1. Rust SDK showcase implementation of Seccomp Guard
fn seccomp_guard_plugin(
    program: Program,
    ctx: &PluginContext,
) -> Result<Program, PluginError> {
    let mut allowed_syscalls = HashSet::new();
    if let Some(opt_str) = ctx.get_option_string("allow") {
        for s in opt_str.split(',') {
            allowed_syscalls.insert(s.trim().to_string());
        }
    }

    let mut diagnostics = Vec::new();

    for tb in &program.timelines {
        for spanned_stmt in &tb.statements {
            if let Statement::Isolate(isolate) = &spanned_stmt.stmt {
                // Check if isolate has @seccomp attribute or options
                let mut local_allowed = allowed_syscalls.clone();
                for attr in &spanned_stmt.attributes {
                    if let AttributeKind::Custom { name, args } = &attr.kind {
                        if name == "seccomp" {
                            for arg in args {
                                local_allowed
                                    .insert(arg.trim_matches('"').to_string());
                            }
                        }
                    }
                }

                // Check inner statements for syscall statements or expressions
                for inner in &isolate.body {
                    inner.stmt.for_each_child_expr(&mut |expr| {
                        if let Expression::Syscall { target, .. } = expr {
                            let name = match target {
                                SyscallTarget::Symbol(s) => s.as_str(),
                                SyscallTarget::Number(_) => "unknown_num",
                            };
                            if !local_allowed.is_empty()
                                && !local_allowed.contains(name)
                            {
                                diagnostics.push(PluginDiagnostic {
                                    level: DiagnosticLevel::Error,
                                    message: format!(
                                        "Syscall '{}' violates @seccomp policy!",
                                        name
                                    ),
                                    span: Some(inner.span.clone()),
                                });
                            }
                        }
                    });
                }
            }
        }
    }

    if !diagnostics.is_empty() {
        Err(PluginError::Diagnostics(diagnostics))
    } else {
        Ok(program)
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_plugin_seccomp_guard_showcase_permitted() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        @seccomp("sys_read", "sys_write", "sys_exit")
        isolate telemetry_worker {
            require System.Syscall
            let log_msg = "Telemetry packet verified.\n"
            let written = syscall("sys_write", 1, log_msg, 28) taking 2ms
        }
    }
    "#;

    let program = parser::parse_causm(source)?;

    // Register seccomp guard in-process
    let req = PluginRequest::new("telemetry.csm", program.clone());
    let resp = match seccomp_guard_plugin(
        req.ast.clone(),
        &PluginContext {
            file_path: req.file_path,
            options: req.options,
        },
    ) {
        Ok(ast) => PluginResponse::success(Some(ast), vec![]),
        Err(PluginError::Diagnostics(d)) => {
            PluginResponse::error("Seccomp violation", d)
        }
        Err(PluginError::Message(m)) => PluginResponse::error(m, vec![]),
    };

    assert_eq!(resp.status, PluginStatus::Success);
    assert!(resp.diagnostics.is_empty());

    // Verified AST passed to causm-analysis
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    let modified = resp.modified_ast.unwrap();
    let analysis_res = analyzer.analyze_program(&modified);
    if let Err(ref e) = analysis_res {
        eprintln!("Analysis error: {:?}", e);
    }
    assert!(analysis_res.is_ok());

    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn test_plugin_seccomp_guard_showcase_rejected() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        @seccomp("sys_read", "sys_write", "sys_exit")
        isolate telemetry_worker {
            let pid = syscall("sys_getpid") taking 1ms
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let req = PluginRequest::new("telemetry_bad.csm", program);
    let resp = match seccomp_guard_plugin(
        req.ast.clone(),
        &PluginContext {
            file_path: req.file_path,
            options: req.options,
        },
    ) {
        Ok(ast) => PluginResponse::success(Some(ast), vec![]),
        Err(PluginError::Diagnostics(d)) => {
            PluginResponse::error("Seccomp violation", d)
        }
        Err(PluginError::Message(m)) => PluginResponse::error(m, vec![]),
    };

    assert!(matches!(resp.status, PluginStatus::Error(_)));
    assert_eq!(resp.diagnostics.len(), 1);
    assert_eq!(resp.diagnostics[0].level, DiagnosticLevel::Error);
    assert!(resp.diagnostics[0]
        .message
        .contains("Syscall 'sys_getpid' violates @seccomp policy!"));

    Ok(())
}
