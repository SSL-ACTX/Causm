use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_std_core_option_methods() -> anyhow::Result<()> {
    let source = r#"
    import "std/core" as Core

    @0ms: {
        let opt_some = Option::Some(42)
        let opt_none = Option::None

        let some_check = opt_some.is_some()
        let some_none_check = opt_some.is_none()
        let some_val = opt_some.unwrap_or(0)

        let none_check = opt_none.is_some()
        let none_none_check = opt_none.is_none()
        let none_val = opt_none.unwrap_or(99)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let some_check_reg = ir
        .symbols
        .get("some_check")
        .expect("some_check not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(some_check_reg),
        Some(Payload::Bool(true))
    );

    let some_none_check_reg = ir
        .symbols
        .get("some_none_check")
        .expect("some_none_check not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(some_none_check_reg),
        Some(Payload::Bool(false))
    );

    let some_val_reg = ir.symbols.get("some_val").expect("some_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(some_val_reg),
        Some(Payload::Integer(42))
    );

    let none_check_reg = ir
        .symbols
        .get("none_check")
        .expect("none_check not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(none_check_reg),
        Some(Payload::Bool(false))
    );

    let none_none_check_reg = ir
        .symbols
        .get("none_none_check")
        .expect("none_none_check not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(none_none_check_reg),
        Some(Payload::Bool(true))
    );

    let none_val_reg = ir.symbols.get("none_val").expect("none_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(none_val_reg),
        Some(Payload::Integer(99))
    );

    Ok(())
}

#[test]
fn test_std_core_result_methods() -> anyhow::Result<()> {
    let source = r#"
    import "std/core" as Core

    @0ms: {
        let res_ok = Result::Ok("payload_success")
        let res_err = Result::Err(500)

        let ok_check = res_ok.is_ok()
        let ok_err_check = res_ok.is_err()
        let ok_val = res_ok.unwrap_or("default_val")

        let err_check = res_err.is_ok()
        let err_err_check = res_err.is_err()
        let err_val = res_err.unwrap_or(0)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let ok_check_reg = ir.symbols.get("ok_check").expect("ok_check not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(ok_check_reg),
        Some(Payload::Bool(true))
    );

    let ok_err_check_reg = ir
        .symbols
        .get("ok_err_check")
        .expect("ok_err_check not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(ok_err_check_reg),
        Some(Payload::Bool(false))
    );

    let ok_val_reg = ir.symbols.get("ok_val").expect("ok_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(ok_val_reg),
        Some(Payload::String("payload_success".to_string()))
    );

    let err_check_reg = ir.symbols.get("err_check").expect("err_check not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(err_check_reg),
        Some(Payload::Bool(false))
    );

    let err_err_check_reg = ir
        .symbols
        .get("err_err_check")
        .expect("err_err_check not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(err_err_check_reg),
        Some(Payload::Bool(true))
    );

    let err_val_reg = ir.symbols.get("err_val").expect("err_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(err_val_reg),
        Some(Payload::Integer(0))
    );

    Ok(())
}

#[test]
fn test_json_value_method_calls() -> anyhow::Result<()> {
    let source = r#"
    import "std/json" as Json

    @0ms: {
        let input = "{\"name\":\"Causm\",\"count\":10}"
        let parsed = Json.parse(input)

        let is_obj = parsed.is_object()
        let name_node = parsed.get("name")
        let is_str = name_node.is_string()
        let name_val = name_node.as_string()

        let count_node = parsed.get("count")
        let count_val = count_node.as_number()
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let is_obj_reg = ir.symbols.get("is_obj").expect("is_obj not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_obj_reg),
        Some(Payload::Bool(true))
    );

    let is_str_reg = ir.symbols.get("is_str").expect("is_str not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_str_reg),
        Some(Payload::Bool(true))
    );

    let name_val_reg = ir.symbols.get("name_val").expect("name_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(name_val_reg),
        Some(Payload::String("Causm".to_string()))
    );

    let count_val_reg = ir.symbols.get("count_val").expect("count_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(count_val_reg),
        Some(Payload::Integer(10))
    );

    Ok(())
}

#[test]
fn test_collection_try_peek_option() -> anyhow::Result<()> {
    let source = r#"
    import "std/collection" as Collection
    import "std/core" as Core

    @0ms: {
        let mut st = Collection.Stack.new(10)
        let empty_opt = st.try_peek()
        let is_empty_none = empty_opt.is_none()

        st = st.push(100)
        let pushed_opt = st.try_peek()
        let is_pushed_some = pushed_opt.is_some()
        let val = pushed_opt.unwrap_or(0)

        let mut q = Collection.Queue.new(10)
        let q_empty_opt = q.try_peek()
        let is_q_empty_none = q_empty_opt.is_none()

        q = q.push(200)
        let q_pushed_opt = q.try_peek()
        let q_val = q_pushed_opt.unwrap_or(0)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let is_empty_none_reg = ir
        .symbols
        .get("is_empty_none")
        .expect("is_empty_none not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_empty_none_reg),
        Some(Payload::Bool(true))
    );

    let is_pushed_some_reg = ir
        .symbols
        .get("is_pushed_some")
        .expect("is_pushed_some not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_pushed_some_reg),
        Some(Payload::Bool(true))
    );

    let val_reg = ir.symbols.get("val").expect("val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(val_reg),
        Some(Payload::Integer(100))
    );

    let is_q_empty_none_reg = ir
        .symbols
        .get("is_q_empty_none")
        .expect("is_q_empty_none not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_q_empty_none_reg),
        Some(Payload::Bool(true))
    );

    let q_val_reg = ir.symbols.get("q_val").expect("q_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(q_val_reg),
        Some(Payload::Integer(200))
    );

    Ok(())
}

