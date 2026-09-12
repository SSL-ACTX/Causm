use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::{hir, lower, parser};
use causm_runtime::vm::Vm;

#[test]
fn test_jit_execution_arithmetic_routine_call() {
    let source = r#"
@0ms: {
    routine add_and_double(a: int, b: int) -> int taking 5ms {
        let sum = a + b
        yield sum * 2
    }

    let res = add_and_double(15, 25)
    debug(res)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_jit_arithmetic")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm = Vm::new();
    vm.jit_enabled = true;
    vm.execute_program(&ir).expect("Execution failed");

    let res_reg = vm.symbols.get("res").expect("Symbol 'res' not found");
    let val = vm.root_timeline.arena.peek(res_reg.0).expect("Register missing");
    assert_eq!(val, Payload::Integer(80));
}

#[test]
fn test_jit_execution_branching_and_phi_merge() {
    let source = r#"
@0ms: {
    routine branch_calc(flag: bool) -> int taking 5ms {
        if (flag) {
            yield 500
        } else {
            yield 100
        } reconcile auto
    }

    let v1 = branch_calc(true)
    let v2 = branch_calc(false)
    debug(v1)
    debug(v2)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_jit_branch")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm = Vm::new();
    vm.jit_enabled = true;
    vm.execute_program(&ir).expect("Execution failed");

    let v1_reg = vm.symbols.get("v1").expect("Symbol 'v1' not found");
    let v2_reg = vm.symbols.get("v2").expect("Symbol 'v2' not found");
    let val1 = vm.root_timeline.arena.peek(v1_reg.0).expect("Register v1 missing");
    let val2 = vm.root_timeline.arena.peek(v2_reg.0).expect("Register v2 missing");
    assert_eq!(val1, Payload::Integer(500));
    assert_eq!(val2, Payload::Integer(100));
}

#[test]
fn test_jit_execution_loop_counter_accumulator() {
    let source = r#"
@0ms: {
    routine loop_accum(n: int) -> int taking 30ms {
        let i = 0
        let sum = 0
        while (i < n) taking 20ms {
            let sum = sum + i
            let i = i + 1
        }
        yield sum
    }

    let total = loop_accum(10)
    debug(total)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_jit_loop")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm = Vm::new();
    vm.jit_enabled = true;
    vm.execute_program(&ir).expect("Execution failed");

    let total_reg = vm.symbols.get("total").expect("Symbol 'total' not found");
    let val = vm.root_timeline.arena.peek(total_reg.0).expect("Register 'total' missing");
    assert_eq!(val, Payload::Integer(45));
}
