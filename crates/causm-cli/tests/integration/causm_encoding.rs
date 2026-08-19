use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_encoding_utf8_encode_decode() -> anyhow::Result<()> {
    let source = r#"
    import "std/encoding/utf8" as Utf8

    @0ms: {
        let text = "Hello Causm"
        let bytes = Utf8.encode(text)
        let n = len(bytes)
        let roundtrip = Utf8.decode(bytes)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let n_reg = ir.symbols.get("n").expect("n not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(n_reg),
        Some(Payload::Integer(11))
    );

    let roundtrip_reg = ir.symbols.get("roundtrip").expect("roundtrip not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(roundtrip_reg),
        Some(Payload::String("Hello Causm".to_string()))
    );

    Ok(())
}

#[test]
fn test_encoding_binary_endianness_pack_unpack() -> anyhow::Result<()> {
    let source = r#"
    import "std/encoding/binary" as Binary

    @0ms: {
        let port_be = Binary.write_u16_be(8080)
        let port_val = Binary.read_u16_be(port_be)

        let u32_le = Binary.write_u32_le(305419896)
        let u32_val = Binary.read_u32_le(u32_le)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let port_val_reg = ir.symbols.get("port_val").expect("port_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(port_val_reg),
        Some(Payload::Integer(8080))
    );

    let u32_val_reg = ir.symbols.get("u32_val").expect("u32_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(u32_val_reg),
        Some(Payload::Integer(305419896))
    );

    Ok(())
}

#[test]
fn test_encoding_base64_chunk_transform() -> anyhow::Result<()> {
    let source = r#"
    import "std/encoding/base64" as Base64

    @0ms: {
        // "Man" -> [77, 97, 110] -> Base64 chunk "TWFu" -> [84, 87, 70, 117]
        let chunk = Base64.encode_chunk(77, 97, 110)
        let decoded = Base64.decode_chunk(84, 87, 70, 117)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let chunk_reg = ir.symbols.get("chunk").expect("chunk not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(chunk_reg),
        Some(Payload::Array(vec![
            Payload::Integer(84),
            Payload::Integer(87),
            Payload::Integer(70),
            Payload::Integer(117),
        ]))
    );

    let decoded_reg = ir.symbols.get("decoded").expect("decoded not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(decoded_reg),
        Some(Payload::Array(vec![
            Payload::Integer(77),
            Payload::Integer(97),
            Payload::Integer(110),
        ]))
    );

    Ok(())
}

#[test]
fn test_syntax_for_in_step_wildcard() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let arr = [10, 20, 30]
        let total = 0
        for x in arr step _ {
            let total = total + x
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let total_reg = ir.symbols.get("total").expect("total not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(total_reg),
        Some(Payload::Integer(60))
    );

    Ok(())
}

#[test]
fn test_syntax_len_intrinsic_array_and_string() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let bytes = str_bytes("Causm")
        let arr_len = len(bytes)
        let str_len = len("Causm")
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let arr_len_reg = ir.symbols.get("arr_len").expect("arr_len not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(arr_len_reg),
        Some(Payload::Integer(5))
    );

    let str_len_reg = ir.symbols.get("str_len").expect("str_len not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(str_len_reg),
        Some(Payload::Integer(5))
    );

    Ok(())
}

#[test]
fn test_stdlib_auto_drop_resource_lifecycle() -> anyhow::Result<()> {
    let source = r#"
    import "std/net" as Net

    @0ms: {
        routine consume_listener(consume l) taking _ {
            let done = true
        }
        let listener = Net.TcpListener.bind(19899)
        call consume_listener(listener)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let listener_reg = ir.symbols.get("listener").expect("listener not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(listener_reg),
        None,
        "Listener should be consumed and auto-dropped"
    );

    Ok(())
}
