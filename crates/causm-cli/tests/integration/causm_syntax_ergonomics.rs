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

#[test]
fn test_branchless_conditional_select_vm_execution() -> anyhow::Result<()> {
    let mut vm = Vm::new();
    // Verify ConditionalSelect instruction works directly in TVM
    let mut instructions = Vec::new();
    // R0 = true, R1 = 42, R2 = 99, R3 = ConditionalSelect(R0, R1, R2)
    instructions.push(causm_ir::Instruction::LoadBool {
        dest: causm_ir::Reg(0),
        value: true,
    });
    instructions.push(causm_ir::Instruction::LoadInt {
        dest: causm_ir::Reg(1),
        value: 42,
    });
    instructions.push(causm_ir::Instruction::LoadInt {
        dest: causm_ir::Reg(2),
        value: 99,
    });
    instructions.push(causm_ir::Instruction::ConditionalSelect {
        dest: causm_ir::Reg(3),
        cond: causm_ir::Reg(0),
        true_val: causm_ir::Reg(1),
        false_val: causm_ir::Reg(2),
    });

    let block = causm_ir::IrBlock {
        time: causm_core::TimeCoordinate::Global(0),
        entropy_mode: None,
        instructions,
        spans: Vec::new(),
    };

    let program = causm_ir::IrProgram {
        blocks: vec![block],
        routines: std::collections::HashMap::new(),
        symbols: std::collections::HashMap::new(),
        type_decay_limits: std::collections::HashMap::new(),
        auto_drop_specs: std::collections::HashMap::new(),
        struct_extends: std::collections::HashMap::new(),
        decay_handlers: std::collections::HashMap::new(),
    };

    vm.execute_program(&program)?;
    assert_eq!(vm.peek_reg("main", 3)?, Payload::Integer(42));

    Ok(())
}

#[test]
fn test_branchless_conditional_select_emitted_from_source_syntax(
) -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let flag = true
        let chosen = if (flag) { 42 } else { 99 }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);

    // Verify emitted IR block contains ConditionalSelect and NO Jump instructions
    assert!(!ir.blocks.is_empty(), "Expected blocks in IrProgram");
    let block = &ir.blocks[0];
    let has_cond_select = block.instructions.iter().any(|instr| {
        matches!(instr, causm_ir::Instruction::ConditionalSelect { .. })
    });
    let has_jump = block.instructions.iter().any(|instr| {
        matches!(
            instr,
            causm_ir::Instruction::Jump { .. }
                | causm_ir::Instruction::JumpIf { .. }
                | causm_ir::Instruction::JumpIfNot { .. }
        )
    });

    assert!(
        has_cond_select,
        "Frontend MUST emit ConditionalSelect for pure scalar if-else"
    );
    assert!(
        !has_jump,
        "Frontend MUST NOT emit Jump instructions for branchless if-else"
    );

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let chosen_reg = ir.symbols.get("chosen").expect("chosen symbol").0;
    assert_eq!(vm.peek_reg("main", chosen_reg)?, Payload::Integer(42));

    Ok(())
}

#[test]
fn test_syntax_collection_and_string_primitives_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let mut list = [10, 20]
        list = push(list, 30)
        let last = pop(list)

        let sliced = array_slice(list, 0, 1)

        let ascii_bytes = [72, 101, 108, 108, 111]
        let greeting = string_from_bytes(ascii_bytes)
        let first_ch = char_at(greeting, 0)
        let sub = str_slice(greeting, 0, 4)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let list_reg = ir.symbols.get("list").expect("list symbol").0;
    let last_reg = ir.symbols.get("last").expect("last symbol").0;
    let sliced_reg = ir.symbols.get("sliced").expect("sliced symbol").0;
    let greeting_reg = ir.symbols.get("greeting").expect("greeting symbol").0;
    let first_ch_reg = ir.symbols.get("first_ch").expect("first_ch symbol").0;
    let sub_reg = ir.symbols.get("sub").expect("sub symbol").0;

    assert_eq!(
        vm.peek_reg("main", list_reg)?,
        Payload::Array(vec![
            Payload::Integer(10),
            Payload::Integer(20),
            Payload::Integer(30)
        ])
    );
    assert_eq!(vm.peek_reg("main", last_reg)?, Payload::Integer(30));
    assert_eq!(
        vm.peek_reg("main", sliced_reg)?,
        Payload::Array(vec![Payload::Integer(10)])
    );
    assert_eq!(
        vm.peek_reg("main", greeting_reg)?,
        Payload::String("Hello".to_string())
    );
    assert_eq!(vm.peek_reg("main", first_ch_reg)?, Payload::Integer(72));
    assert_eq!(
        vm.peek_reg("main", sub_reg)?,
        Payload::String("Hell".to_string())
    );

    Ok(())
}

#[test]
fn test_syntax_compound_assignment_operators() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let mut x = 10
        x += 5
        x -= 3
        x *= 4
        x /= 2
        x %= 7
        let mut b = 1
        b <<= 3
        b >>= 1
        b |= 0x04
        b &= 0x06
        b ^= 0x02
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let x_reg = ir.symbols.get("x").expect("x symbol").0;
    let b_reg = ir.symbols.get("b").expect("b symbol").0;

    // x calculation:
    // 10 + 5 = 15
    // 15 - 3 = 12
    // 12 * 4 = 48
    // 48 / 2 = 24
    // 24 % 7 = 3
    assert_eq!(vm.peek_reg("main", x_reg)?, Payload::Integer(3));

    // b calculation:
    // 1 << 3 = 8
    // 8 >> 1 = 4
    // 4 | 4 = 4
    // 4 & 6 = 4
    // 4 ^ 2 = 6
    assert_eq!(vm.peek_reg("main", b_reg)?, Payload::Integer(6));

    Ok(())
}

#[test]
fn test_syntax_bitwise_operators_and_shift() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let a = 0x0F & 0x07
        let b = 0x08 | 0x02
        let c = 0x0F ^ 0x03
        let d = 1 << 4
        let e = 64 >> 2
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let a_reg = ir.symbols.get("a").expect("a symbol").0;
    let b_reg = ir.symbols.get("b").expect("b symbol").0;
    let c_reg = ir.symbols.get("c").expect("c symbol").0;
    let d_reg = ir.symbols.get("d").expect("d symbol").0;
    let e_reg = ir.symbols.get("e").expect("e symbol").0;

    assert_eq!(vm.peek_reg("main", a_reg)?, Payload::Integer(0x07));
    assert_eq!(vm.peek_reg("main", b_reg)?, Payload::Integer(0x0A));
    assert_eq!(vm.peek_reg("main", c_reg)?, Payload::Integer(0x0C));
    assert_eq!(vm.peek_reg("main", d_reg)?, Payload::Integer(16));
    assert_eq!(vm.peek_reg("main", e_reg)?, Payload::Integer(16));

    Ok(())
}

#[test]
fn test_syntax_bitwise_not_operator() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let raw = 0
        let inv = ~raw
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let inv_reg = ir.symbols.get("inv").expect("inv symbol").0;
    assert_eq!(vm.peek_reg("main", inv_reg)?, Payload::Integer(-1));

    Ok(())
}

#[test]
fn test_syntax_null_coalescing_operator() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let a = null ?? 8080
        let b = 42 ?? 8080
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let a_reg = ir.symbols.get("a").expect("a symbol").0;
    let b_reg = ir.symbols.get("b").expect("b symbol").0;

    assert_eq!(vm.peek_reg("main", a_reg)?, Payload::Integer(8080));
    assert_eq!(vm.peek_reg("main", b_reg)?, Payload::Integer(42));

    Ok(())
}
