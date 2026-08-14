use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_import_file_level_and_named_symbols() -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join("causm_test_imports");
    let _ = std::fs::create_dir_all(&temp_dir);

    let helper_path = temp_dir.join("helper.csm");
    std::fs::write(
        &helper_path,
        r#"
        @0ms: {
            isolate helper {
                let helper_val = 12345
            }
        }
        "#,
    )?;

    let main_path = temp_dir.join("main.csm");
    let main_source = r#"
    @0ms: {
        isolate main_iso {
            import "helper.csm" as Helper
            let final_val = 99
        }
    }
    "#;
    std::fs::write(&main_path, main_source)?;

    let program = parser::parse_causm_with_imports(main_source, Some(&temp_dir))?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let final_reg = ir.symbols.get("final_val").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(final_reg),
        Some(causm_core::value::Payload::Integer(99))
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_import_wildcard_symbol_import() -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join("causm_test_wildcard_imports");
    let _ = std::fs::create_dir_all(&temp_dir);

    let math_lib_path = temp_dir.join("math_lib.csm");
    std::fs::write(
        &math_lib_path,
        r#"
        @0ms: {
            isolate math_iso {
                pub routine compute_bonus(peek val: int) -> int taking 10ms {
                    let bonus = val + 500
                    yield bonus
                }
            }
        }
        "#,
    )?;

    let main_path = temp_dir.join("main_wildcard.csm");
    let main_source = r#"
    @0ms: {
        isolate main_iso {
            from "math_lib.csm" import *

            let base_val = 100
            let res = call compute_bonus(base_val)
        }
    }
    "#;
    std::fs::write(&main_path, main_source)?;

    let program = parser::parse_causm_with_imports(main_source, Some(&temp_dir))?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let res_reg = ir.symbols.get("res").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(res_reg),
        Some(causm_core::value::Payload::Integer(600))
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_import_std_time_monotonic_telemetry() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/time" as Time
        let ts = call Time.unix_timestamp()
        let start = call Time.now()
        let dur = call Time.from_millis(100)
        let total_nanos = dur.nanos_total
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

    let total_nanos_reg = ir.symbols.get("total_nanos").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(total_nanos_reg),
        Some(causm_core::value::Payload::Integer(100_000_000))
    );

    let ts_reg = ir.symbols.get("ts").unwrap().0;
    if let Some(causm_core::value::Payload::Integer(ts_val)) =
        vm.root_timeline.arena.peek(ts_reg)
    {
        assert!(ts_val > 1_700_000_000);
    } else {
        panic!("Expected valid UNIX timestamp from std/time");
    }

    Ok(())
}

#[test]
fn test_import_std_net_tcp_bind_and_sockaddr() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/net" as Net
        let fd = call Net.create_socket()
        let _reuse = call Net.set_reuseaddr(fd)
        let sa = call Net.make_sockaddr(19234, 127, 0, 0, 1)
        let sa_len = 16
        let _close = call Net.close_socket(fd)
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

    let sa_len_reg = ir.symbols.get("sa_len").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(sa_len_reg),
        Some(causm_core::value::Payload::Integer(16))
    );

    Ok(())
}

#[test]
fn test_import_std_net_socket_creation() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/net" as Net
        let saddr = call Net.addr("127.0.0.1", 9090)
        let port = saddr.port
        let fd = call Net.create_socket()
        let close_res = call Net.close_socket(fd)
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

    let port_reg = ir.symbols.get("port").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(port_reg),
        Some(causm_core::value::Payload::Integer(9090))
    );

    let close_res_reg = ir.symbols.get("close_res").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(close_res_reg),
        Some(causm_core::value::Payload::Integer(0))
    );

    Ok(())
}

#[test]
fn test_import_std_net_udp_datagram_transmission() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/net" as Net
        let server = call Net.udp_bind(19899)
        let s_fd = server.fd
        let client_fd = call Net.create_udp_socket()

        let payload = [85, 68, 80]
        let sent = call Net.udp_send_to(client_fd, payload, 3, 19899, 127, 0, 0, 1)

        let mut buf = [0, 0, 0]
        let recvd = call Net.udp_recv_from(s_fd, buf, 3)

        let _close_c = call Net.close_socket(client_fd)
        let _close_s = call Net.close_socket(s_fd)
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

    let sent_reg = ir.symbols.get("sent").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(sent_reg),
        Some(causm_core::value::Payload::Integer(3))
    );

    let recvd_reg = ir.symbols.get("recvd").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(recvd_reg),
        Some(causm_core::value::Payload::Integer(3))
    );

    let buf_reg = ir.symbols.get("buf").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(buf_reg),
        Some(causm_core::value::Payload::Array(vec![
            causm_core::value::Payload::Integer(85),
            causm_core::value::Payload::Integer(68),
            causm_core::value::Payload::Integer(80),
        ]))
    );

    Ok(())
}

#[test]
fn test_import_std_net_nonblocking_socket_configuration() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/net" as Net
        let fd = call Net.create_socket()
        let nonblock_res = call Net.set_nonblocking(fd)
        let block_res = call Net.set_blocking(fd)
        let _close = call Net.close_socket(fd)
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

    let nonblock_res_reg = ir.symbols.get("nonblock_res").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(nonblock_res_reg),
        Some(causm_core::value::Payload::Integer(0))
    );

    let block_res_reg = ir.symbols.get("block_res").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(block_res_reg),
        Some(causm_core::value::Payload::Integer(0))
    );

    Ok(())
}

#[test]
fn test_import_std_net_connect_ip_and_timeouts() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/net" as Net
        let listener = call Net.tcp_listener(19912)
        let l_fd = listener.fd

        let stream = call Net.tcp_stream_connect_ip("127.0.0.1", 19912)
        let s_fd = stream.fd

        let rcv_to_res = call Net.set_recv_timeout(s_fd, 500)
        let snd_to_res = call Net.set_send_timeout(s_fd, 500)

        let client_fd = call Net.tcp_accept(l_fd)

        let payload = [65, 66, 67]
        let sent = call Net.tcp_send_all(s_fd, payload, 3)

        let mut buf = [0, 0, 0]
        let recvd = call Net.tcp_recv_exact(client_fd, buf, 3)

        let _c1 = call Net.close_socket(client_fd)
        let _c2 = call Net.close_socket(s_fd)
        let _c3 = call Net.close_socket(l_fd)
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

    let rcv_to_reg = ir.symbols.get("rcv_to_res").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(rcv_to_reg),
        Some(causm_core::value::Payload::Integer(0))
    );

    let snd_to_reg = ir.symbols.get("snd_to_res").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(snd_to_reg),
        Some(causm_core::value::Payload::Integer(0))
    );

    let sent_reg = ir.symbols.get("sent").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(sent_reg),
        Some(causm_core::value::Payload::Integer(3))
    );

    let recvd_reg = ir.symbols.get("recvd").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(recvd_reg),
        Some(causm_core::value::Payload::Integer(3))
    );

    let buf_reg = ir.symbols.get("buf").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(buf_reg),
        Some(causm_core::value::Payload::Array(vec![
            causm_core::value::Payload::Integer(65),
            causm_core::value::Payload::Integer(66),
            causm_core::value::Payload::Integer(67),
        ]))
    );

    Ok(())
}
