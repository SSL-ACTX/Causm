use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_syntax_byte_string_literals_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let payload = b"ABCDE"
        let b0 = payload[0]
        let b4 = payload[4]
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let payload_reg = ir.symbols.get("payload").expect("payload symbol").0;
    let payload_val = vm.peek_reg("main", payload_reg)?;
    if let Payload::Array(elements) = payload_val {
        assert_eq!(elements.len(), 5);
        assert_eq!(elements[0], Payload::Integer(65));
        assert_eq!(elements[1], Payload::Integer(66));
        assert_eq!(elements[2], Payload::Integer(67));
        assert_eq!(elements[3], Payload::Integer(68));
        assert_eq!(elements[4], Payload::Integer(69));
    } else {
        panic!("expected array payload");
    }

    Ok(())
}

#[test]
fn test_syntax_hex_byte_string_literals_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let magic = hex"DE AD BE EF"
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let magic_reg = ir.symbols.get("magic").expect("magic symbol").0;
    let magic_val = vm.peek_reg("main", magic_reg)?;
    if let Payload::Array(elements) = magic_val {
        assert_eq!(elements.len(), 4);
        assert_eq!(elements[0], Payload::Integer(0xDE));
        assert_eq!(elements[1], Payload::Integer(0xAD));
        assert_eq!(elements[2], Payload::Integer(0xBE));
        assert_eq!(elements[3], Payload::Integer(0xEF));
    } else {
        panic!("expected array payload");
    }

    Ok(())
}

#[test]
fn test_syntax_hex_integer_literal_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let hex_val = 0xDEADBEEF
        let hex_lower = 0x10
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let hex_val_reg = ir.symbols.get("hex_val").expect("hex_val symbol").0;
    let val = vm.peek_reg("main", hex_val_reg)?;
    assert_eq!(val, Payload::Integer(0xDEADBEEF_u32 as i64));

    let hex_lower_reg = ir.symbols.get("hex_lower").expect("hex_lower symbol").0;
    let val16 = vm.peek_reg("main", hex_lower_reg)?;
    assert_eq!(val16, Payload::Integer(16));

    Ok(())
}

#[test]
fn test_syntax_direct_function_call_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        routine compute_sum(a: int, b: int) -> int taking _ {
            let res = a + b
            yield res
        }

        let total = compute_sum(100, 250)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let total_reg = ir.symbols.get("total").expect("total symbol").0;
    let total = vm.peek_reg("main", total_reg)?;
    assert_eq!(total, Payload::Integer(350));

    Ok(())
}

#[test]
fn test_syntax_concise_arrow_routine_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        pub routine multiply(a: int, b: int) -> int taking _ => a * b

        let result = multiply(7, 8)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let result_reg = ir.symbols.get("result").expect("result symbol").0;
    let result = vm.peek_reg("main", result_reg)?;
    assert_eq!(result, Payload::Integer(56));

    Ok(())
}

#[test]
fn test_syntax_struct_destructuring_assignment() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Pipe = struct { read_fd: int, write_fd: int }

        routine create_pipe() -> Pipe taking _ {
            struct { read_fd = 3, write_fd = 4 }
        }

        let { read_fd as rx, write_fd as tx } = create_pipe()
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let rx_reg = ir.symbols.get("rx").expect("rx symbol").0;
    let rx = vm.peek_reg("main", rx_reg)?;
    assert_eq!(rx, Payload::Integer(3));

    let tx_reg = ir.symbols.get("tx").expect("tx symbol").0;
    let tx = vm.peek_reg("main", tx_reg)?;
    assert_eq!(tx, Payload::Integer(4));

    Ok(())
}

#[test]
fn test_syntax_using_scoped_resource_lifecycle() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Resource = struct { id: int }

        routine acquire_resource(id: int) -> Resource taking _ {
            struct { id = id }
        }

        let mut final_sum = 0

        using res = acquire_resource(42) {
            let item_id = res.id
            final_sum = item_id + 8
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    // res should be consumed in analyzer branch state
    let main_branch = analyzer.branch_contexts.get("main").unwrap();
    assert!(main_branch.consumed.contains("res"));

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let final_sum_reg = ir.symbols.get("final_sum").expect("final_sum symbol").0;
    let final_sum = vm.peek_reg("main", final_sum_reg)?;
    assert_eq!(final_sum, Payload::Integer(50));

    Ok(())
}

#[test]
fn test_syntax_universal_method_call_namespaced() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        routine Math.compute_digest(val: int, factor: int) -> int taking _ => val * factor + 1

        let digest = Math.compute_digest(20, 3)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let digest_reg = ir.symbols.get("digest").expect("digest symbol").0;
    let digest = vm.peek_reg("main", digest_reg)?;
    assert_eq!(digest, Payload::Integer(61));

    Ok(())
}

#[test]
fn test_syntax_pipeline_operator_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        routine double_it(x: int) -> int taking _ => x * 2
        routine add_ten(x: int) -> int taking _ => x + 10

        let result = 15 |> double_it() |> add_ten()
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let result_reg = ir.symbols.get("result").expect("result symbol").0;
    let result = vm.peek_reg("main", result_reg)?;
    assert_eq!(result, Payload::Integer(40));

    Ok(())
}

#[test]
fn test_syntax_if_statement_omitted_reconcile_clause() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let mut x = 10
        if (x > 5) {
            x = 25
        }
        let final_val = x
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let final_val_reg = ir.symbols.get("final_val").expect("final_val symbol").0;
    let final_val = vm.peek_reg("main", final_val_reg)?;
    assert_eq!(final_val, Payload::Integer(25));

    Ok(())
}

#[test]
fn test_syntax_if_else_expression_evaluation() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let cond_a = true
        let cond_b = false

        let res_a = if (cond_a) { 100 } else { 200 }
        let res_b = if (cond_b) { 100 } else { 200 }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let res_a_reg = ir.symbols.get("res_a").expect("res_a symbol").0;
    let res_b_reg = ir.symbols.get("res_b").expect("res_b symbol").0;
    assert_eq!(vm.peek_reg("main", res_a_reg)?, Payload::Integer(100));
    assert_eq!(vm.peek_reg("main", res_b_reg)?, Payload::Integer(200));

    Ok(())
}

#[test]
fn test_syntax_nested_if_else_expression() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let score = 85
        let grade = if (score >= 90) { 1 } else if (score >= 80) { 2 } else { 3 }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let grade_reg = ir.symbols.get("grade").expect("grade symbol").0;
    assert_eq!(vm.peek_reg("main", grade_reg)?, Payload::Integer(2));

    Ok(())
}
