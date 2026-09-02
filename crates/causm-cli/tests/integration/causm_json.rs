use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_json_encode_primitives() -> anyhow::Result<()> {
    let source = r#"
    import "std/json" as Json

    @0ms: {
        let n_val = Json.num_val(42)
        let s_val = Json.str_val("hello world")
        let b_val = Json.bool_val(true)
        let null_val = Json.null_val()

        let n_str = Json.stringify(n_val)
        let s_str = Json.stringify(s_val)
        let b_str = Json.stringify(b_val)
        let null_str = Json.stringify(null_val)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let n_str_reg = ir.symbols.get("n_str").expect("n_str not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(n_str_reg),
        Some(Payload::String("42".to_string()))
    );

    let s_str_reg = ir.symbols.get("s_str").expect("s_str not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(s_str_reg),
        Some(Payload::String("\"hello world\"".to_string()))
    );

    let b_str_reg = ir.symbols.get("b_str").expect("b_str not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(b_str_reg),
        Some(Payload::String("true".to_string()))
    );

    let null_str_reg = ir.symbols.get("null_str").expect("null_str not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(null_str_reg),
        Some(Payload::String("null".to_string()))
    );

    Ok(())
}

#[test]
fn test_json_decode_and_query_object() -> anyhow::Result<()> {
    let source = r#"
    import "std/json" as Json

    @0ms: {
        let input = "{\"name\": \"Alice\", \"age\": 30, \"active\": true}"
        let parsed = Json.parse(input)

        let name_val = Json.get_string(parsed, "name", "")
        let age_val = Json.get_int(parsed, "age", 0)
        let active_val = Json.get_bool(parsed, "active", false)
        let has_name = Json.has_key(parsed, "name")
        let has_missing = Json.has_key(parsed, "missing")
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let name_reg = ir.symbols.get("name_val").expect("name_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(name_reg),
        Some(Payload::String("Alice".to_string()))
    );

    let age_reg = ir.symbols.get("age_val").expect("age_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(age_reg),
        Some(Payload::Integer(30))
    );

    let active_reg = ir
        .symbols
        .get("active_val")
        .expect("active_val not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(active_reg),
        Some(Payload::Bool(true))
    );

    let has_name_reg = ir.symbols.get("has_name").expect("has_name not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_name_reg),
        Some(Payload::Bool(true))
    );

    let has_missing_reg = ir
        .symbols
        .get("has_missing")
        .expect("has_missing not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_missing_reg),
        Some(Payload::Bool(false))
    );

    Ok(())
}

#[test]
fn test_json_decode_and_query_array() -> anyhow::Result<()> {
    let source = r#"
    import "std/json" as Json

    @0ms: {
        let input = "[10, 20, 30]"
        let parsed = Json.parse(input)
        let count = Json.array_len(parsed)

        let first = Json.array_get(parsed, 0)
        let first_num = Json.as_number(first)

        let third = Json.array_get(parsed, 2)
        let third_num = Json.as_number(third)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let count_reg = ir.symbols.get("count").expect("count not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(count_reg),
        Some(Payload::Integer(3))
    );

    let first_num_reg = ir.symbols.get("first_num").expect("first_num not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(first_num_reg),
        Some(Payload::Integer(10))
    );

    let third_num_reg = ir.symbols.get("third_num").expect("third_num not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(third_num_reg),
        Some(Payload::Integer(30))
    );

    Ok(())
}

#[test]
fn test_json_encode_object_and_array() -> anyhow::Result<()> {
    let source = r#"
    import "std/json" as Json

    @0ms: {
        let mut members = []
        members = Json.push_member(members, "title", Json.str_val("Causm"))
        members = Json.push_member(members, "version", Json.num_val(1))
        let obj = Json.obj_val(members)
        let json_str = Json.stringify(obj)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let json_str_reg = ir.symbols.get("json_str").expect("json_str not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(json_str_reg),
        Some(Payload::String(
            "{\"title\":\"Causm\",\"version\":1}".to_string()
        ))
    );

    Ok(())
}

#[test]
fn test_json_http_payload_integration() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/json" as Json
        import "std/http" as Http

        // 1. Build JSON request payload
        let mut members = []
        members = Json.push_member(members, "action", Json.str_val("sync"))
        members = Json.push_member(members, "target_id", Json.num_val(99))
        let req_obj = Json.obj_val(members)
        let body = Json.stringify(req_obj)

        // 2. Format HTTP POST request
        let http_req = Http.format_post("cluster.causm.internal", "/api/v1/sync", body)

        // 3. Parse JSON response
        let resp_json = "{\"status\": \"ok\", \"code\": 200, \"active\": true}"
        let parsed = Json.parse(resp_json)
        let status = Json.get_string(parsed, "status", "err")
        let code = Json.get_int(parsed, "code", 0)
        let is_active = Json.get_bool(parsed, "active", false)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let status_reg = ir.symbols.get("status").expect("status not found").0;
    let code_reg = ir.symbols.get("code").expect("code not found").0;
    let active_reg = ir.symbols.get("is_active").expect("is_active not found").0;
    let http_req_reg = ir.symbols.get("http_req").expect("http_req not found").0;

    assert_eq!(
        vm.root_timeline.arena.peek(status_reg),
        Some(Payload::String("ok".to_string()))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(code_reg),
        Some(Payload::Integer(200))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(active_reg),
        Some(Payload::Bool(true))
    );
    let http_payload = vm
        .root_timeline
        .arena
        .peek(http_req_reg)
        .unwrap()
        .to_string();
    assert!(http_payload.contains("POST /api/v1/sync HTTP/1.1"));
    assert!(http_payload.contains("Host: cluster.causm.internal"));
    assert!(http_payload.contains("{\"action\":\"sync\",\"target_id\":99}"));

    Ok(())
}

#[test]
fn test_json_enum_variant_pattern_matching() -> anyhow::Result<()> {
    let source = r#"
    import "std/json" as Json

    @0ms: {
        let input = "{\"score\": 42, \"flag\": true, \"title\": \"causm\"}"
        let parsed = Json.parse(input)

        let score_val = Json.get(parsed, "score")
        let score_extracted = match score_val {
            JsonValue::Number(n) => n * 10,
            _ => 0
        }

        let flag_val = Json.get(parsed, "flag")
        let flag_extracted = match flag_val {
            JsonValue::Bool(b) => b,
            _ => false
        }

        let title_val = Json.get(parsed, "title")
        let mut title_extracted = ""
        if let JsonValue::String(s) = title_val {
            title_extracted = s
        }
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let score_reg = ir
        .symbols
        .get("score_extracted")
        .expect("score_extracted not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(score_reg),
        Some(Payload::Integer(420))
    );

    let flag_reg = ir
        .symbols
        .get("flag_extracted")
        .expect("flag_extracted not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(flag_reg),
        Some(Payload::Bool(true))
    );

    let title_reg = ir
        .symbols
        .get("title_extracted")
        .expect("title_extracted not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(title_reg),
        Some(Payload::String("causm".to_string()))
    );

    Ok(())
}

#[test]
fn test_json_harsh_edge_cases_and_surrogates() -> anyhow::Result<()> {
    let source = r#"
    import "std/json" as Json

    @0ms: {
        let raw_json = "{\"emoji\": \"\\uD83D\\uDE80\", \"escaped\": \"Line1\\nLine2\\t\\\"Quote\\\"\\\\Slash\", \"empty_arr\": [], \"empty_obj\": {}, \"nested\": [[1, 2], {\"k\": \"v\"}]}"
        let parsed = Json.parse(raw_json)

        let emoji_str = Json.get_string(parsed, "emoji", "")
        let escaped_str = Json.get_string(parsed, "escaped", "")
        let re_encoded = Json.stringify(parsed)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let emoji_reg = ir.symbols.get("emoji_str").expect("emoji_str not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(emoji_reg),
        Some(Payload::String("🚀".to_string()))
    );

    let escaped_reg = ir
        .symbols
        .get("escaped_str")
        .expect("escaped_str not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(escaped_reg),
        Some(Payload::String(
            "Line1\nLine2\t\"Quote\"\\Slash".to_string()
        ))
    );

    let re_encoded_reg = ir
        .symbols
        .get("re_encoded")
        .expect("re_encoded not found")
        .0;
    if let Some(Payload::String(s)) = vm.root_timeline.arena.peek(re_encoded_reg) {
        assert!(s.contains("🚀"));
        assert!(s.contains("Line1\\nLine2"));
    } else {
        panic!("Expected re-encoded JSON string");
    }

    Ok(())
}
