use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_http_format_get_and_post() -> anyhow::Result<()> {
    let source = r#"
    import "std/http" as Http

    @0ms: {
        let req_get = Http.format_get("localhost", "/status")
        let req_post = Http.format_post("localhost", "/api/data", "hello=1")
        let code_200 = Http.parse_status_code("HTTP/1.1 200 OK\r\n")
        let text_200 = Http.parse_status_text(200)
        let code_404 = Http.parse_status_code("HTTP/1.1 404 Not Found\r\n")
        let text_404 = Http.parse_status_text(404)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let req_get_reg = ir.symbols.get("req_get").expect("req_get not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(req_get_reg),
        Some(Payload::String(
            "GET /status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
                .to_string()
        ))
    );

    let req_post_reg = ir.symbols.get("req_post").expect("req_post not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(req_post_reg),
        Some(Payload::String("POST /api/data HTTP/1.1\r\nHost: localhost\r\nContent-Length: 7\r\nConnection: close\r\n\r\nhello=1".to_string()))
    );

    let code_200_reg = ir.symbols.get("code_200").expect("code_200 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(code_200_reg),
        Some(Payload::Integer(200))
    );

    let text_200_reg = ir.symbols.get("text_200").expect("text_200 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(text_200_reg),
        Some(Payload::String("OK".to_string()))
    );

    let code_404_reg = ir.symbols.get("code_404").expect("code_404 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(code_404_reg),
        Some(Payload::Integer(404))
    );

    let text_404_reg = ir.symbols.get("text_404").expect("text_404 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(text_404_reg),
        Some(Payload::String("Not Found".to_string()))
    );

    Ok(())
}

#[test]
fn test_http_client_get_request_mock_server() -> anyhow::Result<()> {
    let source = r#"
    import "std/net" as Net
    import "std/http" as Http
    import "std/encoding/utf8" as Utf8

    @0ms: {
        using server = Net.TcpListener.bind(19890) {
            using client = Net.TcpStream.connect("127.0.0.1", 19890) {
                let conn = server.accept()
                let req_bytes = Utf8.encode("GET /ping HTTP/1.1\r\nHost: localhost\r\n\r\n")
                let _s = client.send(req_bytes, len(req_bytes))

                let mut s_buf = [0; 64]
                let _r = conn.recv(s_buf, 64)

                let res_bytes = Utf8.encode("HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nPONG")
                let _s2 = conn.send(res_bytes, len(res_bytes))

                let mut c_buf = [0; 64]
                let c_recvd = client.recv(c_buf, 64)
                let c_str = Utf8.decode(c_buf[0..c_recvd])
                let status_code = Http.parse_status_code(c_str)
                let status_text = Http.parse_status_text(status_code)
            }
        }
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let status_code_reg = ir
        .symbols
        .get("status_code")
        .expect("status_code not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(status_code_reg),
        Some(Payload::Integer(200))
    );

    let status_text_reg = ir
        .symbols
        .get("status_text")
        .expect("status_text not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(status_text_reg),
        Some(Payload::String("OK".to_string()))
    );

    Ok(())
}
