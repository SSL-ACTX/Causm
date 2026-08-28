use causm_plugin_sdk::prelude::*;
use std::collections::HashSet;

/// Multi-architecture Linux syscall number resolver.
fn resolve_syscall_name(number: i64, arch: &str) -> &'static str {
    match arch {
        "aarch64" | "arm64" => match number {
            63 => "sys_read",
            64 => "sys_write",
            93 => "sys_exit",
            172 => "sys_getpid",
            221 => "sys_execve",
            220 => "sys_clone",
            117 => "sys_ptrace",
            _ => "unknown_raw_syscall",
        },
        "riscv64" => match number {
            63 => "sys_read",
            64 => "sys_write",
            93 => "sys_exit",
            172 => "sys_getpid",
            221 => "sys_execve",
            220 => "sys_clone",
            117 => "sys_ptrace",
            _ => "unknown_raw_syscall",
        },
        "arm" => match number {
            3 => "sys_read",
            4 => "sys_write",
            1 => "sys_exit",
            20 => "sys_getpid",
            11 => "sys_execve",
            2 => "sys_fork",
            26 => "sys_ptrace",
            _ => "unknown_raw_syscall",
        },
        // Default to x86_64
        _ => match number {
            0 => "sys_read",
            1 => "sys_write",
            60 => "sys_exit",
            39 => "sys_getpid",
            59 => "sys_execve",
            57 => "sys_fork",
            101 => "sys_ptrace",
            _ => "unknown_raw_syscall",
        },
    }
}

#[causm_plugin(name = "seccomp_guard", version = "0.2.0")]
pub fn process_ast(
    mut program: Program,
    ctx: &PluginContext,
) -> Result<Program, PluginError> {
    let mut allowed_syscalls = HashSet::new();

    // 1. Read default allowlist from causm.toml options
    if let Some(opt_str) = ctx.get_option_string("allow") {
        for s in opt_str.split(',') {
            allowed_syscalls.insert(s.trim().to_string());
        }
    }

    let target_arch = ctx
        .get_option_string("target_arch")
        .unwrap_or(&ctx.target_arch);

    let mut diagnostics = Vec::new();

    // 2. Audit and Transform Isolates
    for tb in &mut program.timelines {
        for spanned_stmt in &mut tb.statements {
            if let Statement::Isolate(isolate) = &mut spanned_stmt.stmt {
                let mut isolate_custom_allowed = HashSet::new();
                let mut is_seccomp_enforced = false;

                // Check for @seccomp attribute on the isolate statement
                for attr in &spanned_stmt.attributes {
                    if let AttributeKind::Custom { name, args } = &attr.kind {
                        if name == "seccomp" {
                            is_seccomp_enforced = true;
                            for arg in args {
                                isolate_custom_allowed
                                    .insert(arg.trim_matches('"').to_string());
                            }
                        }
                    }
                }

                let local_allowed = if is_seccomp_enforced {
                    isolate_custom_allowed
                } else {
                    allowed_syscalls.clone()
                };

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
                                // 1. Direct Syscall Audit with Architecture Resolution
                                Expression::Syscall { target, .. } => {
                                    let syscall_name = match target {
                                        SyscallTarget::Symbol(s) => s.as_str(),
                                        SyscallTarget::Number(n) => {
                                            diagnostics.push(PluginDiagnostic {
                                                level: DiagnosticLevel::Warning,
                                                message: format!(
                                                    "Raw numeric syscall {} on arch '{}' should be specified symbolically as 'sys_{}' for portability",
                                                    n,
                                                    target_arch,
                                                    resolve_syscall_name(*n, target_arch).trim_start_matches("sys_")
                                                ),
                                                span: Some(inner.span.clone()),
                                            });
                                            resolve_syscall_name(*n, target_arch)
                                        }
                                    };

                                    if !local_allowed.is_empty() && !local_allowed.contains(syscall_name) {
                                        diagnostics.push(PluginDiagnostic {
                                            level: DiagnosticLevel::Error,
                                            message: format!(
                                                "Syscall '{}' violates @seccomp whitelist [{}] on target arch '{}'!",
                                                syscall_name,
                                                local_allowed.iter().cloned().collect::<Vec<_>>().join(", "),
                                                target_arch
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

                    // 3. Runtime Seccomp BPF Hook AST Injection:
                    // If no fatal errors occurred, inject an initialization call
                    // `__causm_seccomp_bpf_init(allowed_list)` at the entry of the isolate body.
                    if diagnostics.is_empty() {
                        let filter_payload = local_allowed
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(",");
                        let init_stmt = SpannedStatement {
                            attributes: Vec::new(),
                            stmt: Statement::Expression(Expression::Call {
                                routine: "__causm_seccomp_bpf_init".to_string(),
                                args: vec![Expression::Literal(filter_payload)],
                            }),
                            span: isolate
                                .body
                                .first()
                                .map(|s| s.span.clone())
                                .unwrap_or(causm_core::Span { start: 0, end: 0 }),
                        };
                        isolate.body.insert(0, init_stmt);
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
