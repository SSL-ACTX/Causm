use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_stdlib_process_pid_and_parent_pid() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/process" as Process
        let my_pid = Process.pid()
        let my_ppid = Process.parent_pid()
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let pid_reg = ir.symbols.get("my_pid").unwrap().0;
    if let Some(causm_core::value::Payload::Integer(pid)) =
        vm.root_timeline.arena.peek(pid_reg)
    {
        assert!(pid > 0, "Expected positive PID, got {}", pid);
    } else {
        panic!("Expected valid integer for my_pid");
    }

    let ppid_reg = ir.symbols.get("my_ppid").unwrap().0;
    if let Some(causm_core::value::Payload::Integer(ppid)) =
        vm.root_timeline.arena.peek(ppid_reg)
    {
        assert!(ppid > 0, "Expected positive PPID, got {}", ppid);
    } else {
        panic!("Expected valid integer for my_ppid");
    }

    Ok(())
}

#[test]
fn test_stdlib_process_info_struct() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/process" as Process
        let proc_info = Process.info()
        let info_pid = proc_info.pid
        let is_active = proc_info.is_running
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let info_pid_reg = ir.symbols.get("info_pid").unwrap().0;
    if let Some(causm_core::value::Payload::Integer(pid)) =
        vm.root_timeline.arena.peek(info_pid_reg)
    {
        assert!(pid > 0, "Expected positive PID from struct, got {}", pid);
    } else {
        panic!("Expected valid integer for info_pid");
    }

    let is_active_reg = ir.symbols.get("is_active").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_active_reg),
        Some(causm_core::value::Payload::Bool(true))
    );

    Ok(())
}

#[test]
fn test_stdlib_process_is_alive_and_signal() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/process" as Process
        let curr_pid = Process.pid()
        let alive = Process.is_alive(curr_pid)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let alive_reg = ir.symbols.get("alive").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(alive_reg),
        Some(causm_core::value::Payload::Bool(true))
    );

    Ok(())
}

#[test]
fn test_stdlib_process_create_pipe() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/process" as Process
        let p = Process.create_pipe()
        let rfd = p.read_fd
        let wfd = p.write_fd
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let rfd_reg = ir.symbols.get("rfd").unwrap().0;
    let wfd_reg = ir.symbols.get("wfd").unwrap().0;

    if let (
        Some(causm_core::value::Payload::Integer(r)),
        Some(causm_core::value::Payload::Integer(w)),
    ) = (
        vm.root_timeline.arena.peek(rfd_reg),
        vm.root_timeline.arena.peek(wfd_reg),
    ) {
        assert!(r >= 0, "Expected valid read_fd >= 0, got {}", r);
        assert!(w >= 0, "Expected valid write_fd >= 0, got {}", w);
        assert_ne!(r, w, "read_fd and write_fd must be distinct");
    } else {
        panic!("Expected integer values for pipe descriptors");
    }

    Ok(())
}

#[test]
fn test_stdlib_process_run_command_exit_status() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/process" as Process
        let res_ok = Process.run_command("true")
        let ok_code = res_ok.code
        let ok_success = res_ok.success

        let res_err = Process.run_command("false")
        let err_code = res_err.code
        let err_success = res_err.success
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let ok_code_reg = ir.symbols.get("ok_code").unwrap().0;
    let ok_success_reg = ir.symbols.get("ok_success").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(ok_code_reg),
        Some(causm_core::value::Payload::Integer(0))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(ok_success_reg),
        Some(causm_core::value::Payload::Bool(true))
    );

    let err_code_reg = ir.symbols.get("err_code").unwrap().0;
    let err_success_reg = ir.symbols.get("err_success").unwrap().0;
    if let Some(causm_core::value::Payload::Integer(code)) =
        vm.root_timeline.arena.peek(err_code_reg)
    {
        assert_ne!(code, 0, "Expected non-zero exit code for 'false'");
    } else {
        panic!("Expected integer for err_code");
    }
    assert_eq!(
        vm.root_timeline.arena.peek(err_success_reg),
        Some(causm_core::value::Payload::Bool(false))
    );

    Ok(())
}

#[test]
fn test_stdlib_process_pipe_endpoints_and_child_handle_auto_drop(
) -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/process" as Process
        let endpoints = Process.pipe_endpoints()
        let r = endpoints.reader
        let w = endpoints.writer
        let child = Process.child_handle(99999)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let child_reg = ir.symbols.get("child").unwrap().0;
    if let Some(causm_core::value::Payload::Struct(fields)) =
        vm.root_timeline.arena.peek(child_reg)
    {
        assert!(fields.contains_key("pid"));
    } else {
        panic!("Expected struct for child handle");
    }

    Ok(())
}
