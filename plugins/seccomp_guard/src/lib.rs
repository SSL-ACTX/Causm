use causm_plugin_sdk::prelude::*;
use std::collections::HashSet;

#[causm_plugin(name = "seccomp_guard", version = "0.0.1")]
pub fn process_ast(
    program: Program,
    ctx: &PluginContext,
) -> Result<Program, PluginError> {
    let mut allowed_syscalls = HashSet::new();

    // 1. Read default allowlist from causm.toml options
    if let Some(opt_str) = ctx.get_option_string("allow") {
        for s in opt_str.split(',') {
            allowed_syscalls.insert(s.trim().to_string());
        }
    }

    let mut diagnostics = Vec::new();

    // 2. Audit Isolates and Statements
    for tb in &program.timelines {
        for spanned_stmt in &tb.statements {
            if let Statement::Isolate(isolate) = &spanned_stmt.stmt {
                let mut local_allowed = allowed_syscalls.clone();
                let mut is_seccomp_enforced = false;

                // Check for @seccomp attribute on the isolate statement
                for attr in &spanned_stmt.attributes {
                    if let AttributeKind::Custom { name, args } = &attr.kind {
                        if name == "seccomp" {
                            is_seccomp_enforced = true;
                            for arg in args {
                                local_allowed
                                    .insert(arg.trim_matches('"').to_string());
                            }
                        }
                    }
                }

                if is_seccomp_enforced || !allowed_syscalls.is_empty() {
                    // Check for dangerous operations within sandboxed isolate:
                    for inner in &isolate.body {
                        // Audit Foreign C/Rust blocks inside seccomp isolate
                        if let Statement::ForeignBlock { lib_name, .. } = &inner.stmt
                        {
                            diagnostics.push(PluginDiagnostic {
                                level: DiagnosticLevel::Error,
                                message: format!(
                                    "Foreign FFI library '{}' is forbidden inside @seccomp sandbox isolate '{}'",
                                    lib_name,
                                    isolate.name.as_deref().unwrap_or("anonymous")
                                ),
                                span: Some(inner.span.clone()),
                            });
                        }

                        // Audit all child expressions
                        inner.stmt.for_each_child_expr(&mut |expr| {
                            match expr {
                                // 1. Direct Syscall Audit
                                Expression::Syscall { target, .. } => {
                                    let syscall_name = match target {
                                        SyscallTarget::Symbol(s) => s.as_str(),
                                        SyscallTarget::Number(n) => {
                                            match n {
                                                0 => "sys_read",
                                                1 => "sys_write",
                                                60 => "sys_exit",
                                                39 => "sys_getpid",
                                                59 => "sys_execve",
                                                57 => "sys_fork",
                                                101 => "sys_ptrace",
                                                _ => "unknown_raw_syscall",
                                            }
                                        }
                                    };

                                    if !local_allowed.is_empty() && !local_allowed.contains(syscall_name) {
                                        diagnostics.push(PluginDiagnostic {
                                            level: DiagnosticLevel::Error,
                                            message: format!(
                                                "Syscall '{}' violates @seccomp whitelist [{}]!",
                                                syscall_name,
                                                local_allowed.iter().cloned().collect::<Vec<_>>().join(", ")
                                            ),
                                            span: Some(inner.span.clone()),
                                        });
                                    }
                                }

                                // 2. Audit Dangerous Direct Calls (exec/spawn/raw memory)
                                Expression::Call { routine, .. }
                                    if routine.starts_with("System.Process.exec")
                                        || routine.starts_with("System.Process.spawn")
                                        || routine.starts_with("System.Mem.raw_write")
                                        || routine.starts_with("System.Net.bind_raw_socket") =>
                                {
                                    diagnostics.push(PluginDiagnostic {
                                        level: DiagnosticLevel::Error,
                                        message: format!(
                                            "Dangerous security routine '{}' is blocked by @seccomp sandbox!",
                                            routine
                                        ),
                                        span: Some(inner.span.clone()),
                                    });
                                }

                                _ => {}
                            }
                        });
                    }
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
