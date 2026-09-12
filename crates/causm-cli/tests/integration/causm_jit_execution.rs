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
    let val = vm
        .root_timeline
        .arena
        .peek(res_reg.0)
        .expect("Register missing");
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
    let val1 = vm
        .root_timeline
        .arena
        .peek(v1_reg.0)
        .expect("Register v1 missing");
    let val2 = vm
        .root_timeline
        .arena
        .peek(v2_reg.0)
        .expect("Register v2 missing");
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
    let val = vm
        .root_timeline
        .arena
        .peek(total_reg.0)
        .expect("Register 'total' missing");
    assert_eq!(val, Payload::Integer(45));
}

#[test]
fn test_syntax_speedmicro_taking_cycles_contract() {
    let source = r#"
@0ms: {
    routine compute_cycles(a: int, b: int) -> int taking 5000 cycles {
        yield a + b
    }

    let out = compute_cycles(20, 30)
    debug(out)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_cycles_contract")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm = Vm::new();
    vm.jit_enabled = true;
    vm.execute_program(&ir).expect("Execution failed");

    let out_reg = vm.symbols.get("out").expect("Symbol 'out' not found");
    let val = vm
        .root_timeline
        .arena
        .peek(out_reg.0)
        .expect("Register 'out' missing");
    assert_eq!(val, Payload::Integer(50));
    assert!(
        vm.root_timeline.local_clock >= 5000,
        "local_clock should reflect cycles contract"
    );
}

#[test]
fn test_syntax_speedmicro_yield_pad_instruction() {
    let source = r#"
@0ms: {
    routine spin_routine(x: int) -> int taking 1000 cycles {
        let res = x * 3
        yield_pad
        yield res
    }

    let final_val = spin_routine(7)
    debug(final_val)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_yield_pad")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm = Vm::new();
    vm.jit_enabled = true;
    vm.execute_program(&ir).expect("Execution failed");

    let final_reg = vm
        .symbols
        .get("final_val")
        .expect("Symbol 'final_val' not found");
    let val = vm
        .root_timeline
        .arena
        .peek(final_reg.0)
        .expect("Register missing");
    assert_eq!(val, Payload::Integer(21));
}

#[test]
fn test_jit_speedmicro_branchless_execution() {
    let source = r#"
@0ms: {
    routine branchless_max(a: int, b: int) -> int taking 500 cycles {
        if (a > b) {
            yield a
        } else {
            yield b
        } reconcile auto
    }

    let m1 = branchless_max(10, 20)
    let m2 = branchless_max(20, 10)
    debug(m1)
    debug(m2)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_branchless_max")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm = Vm::new();
    vm.jit_enabled = true;
    vm.execute_program(&ir).expect("Execution failed");

    let m1_reg = vm.symbols.get("m1").expect("Symbol 'm1' not found");
    let m2_reg = vm.symbols.get("m2").expect("Symbol 'm2' not found");
    assert_eq!(
        vm.root_timeline.arena.peek(m1_reg.0).unwrap(),
        Payload::Integer(20)
    );
    assert_eq!(
        vm.root_timeline.arena.peek(m2_reg.0).unwrap(),
        Payload::Integer(20)
    );
}

#[test]
fn test_temporal_speedmicro_isochronous_cycle_padding() {
    let source = r#"
@0ms: {
    routine isochronous_pad() taking 10000 cycles {
        yield 100
    }

    let z = isochronous_pad()
    debug(z)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_isochronous_pad")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm = Vm::new();
    vm.jit_enabled = true;
    vm.execute_program(&ir).expect("Execution failed");

    let z_reg = vm.symbols.get("z").expect("Symbol 'z' not found");
    assert_eq!(
        vm.root_timeline.arena.peek(z_reg.0).unwrap(),
        Payload::Integer(100)
    );
    assert!(vm.root_timeline.local_clock >= 10000);
}

#[test]
fn test_temporal_speedmicro_elastic_determinism_jitter_freeze() {
    causm_jit::hft::reset_total_lost_cycles();

    // 1. Evaluate within-budget execution
    let expected_cycles = 10_000;
    let normal_elapsed = 12_000;
    let status_normal = causm_jit::hft::evaluate_elastic_determinism(
        normal_elapsed,
        expected_cycles,
        causm_jit::hft::DEFAULT_ELASTIC_JITTER_THRESHOLD_CYCLES,
    );
    assert_eq!(status_normal, causm_jit::hft::JitterStatus::WithinBudget);
    assert_eq!(causm_jit::hft::get_total_lost_cycles(), 0);

    // 2. Simulate external OS context switch / hardware interrupt exceeding threshold
    let jitter_elapsed = 300_000;
    let status_jitter = causm_jit::hft::evaluate_elastic_determinism(
        jitter_elapsed,
        expected_cycles,
        causm_jit::hft::DEFAULT_ELASTIC_JITTER_THRESHOLD_CYCLES,
    );
    match status_jitter {
        causm_jit::hft::JitterStatus::ElasticJitterDetected {
            elapsed_cycles,
            expected_cycles: exp,
            lost_to_void,
        } => {
            assert_eq!(elapsed_cycles, 300_000);
            assert_eq!(exp, 10_000);
            assert_eq!(lost_to_void, 290_000);
        }
        _ => panic!("Expected ElasticJitterDetected"),
    }
    assert_eq!(causm_jit::hft::get_total_lost_cycles(), 290_000);

    // 3. Verify Vm temporal_freeze shifts timeline baselines while preserving causal invariants
    let mut vm = Vm::new();
    let initial_global = vm.global_clock;
    vm.temporal_freeze(6_000_000);
    assert_eq!(vm.global_clock, initial_global + 2);

    // 4. End-to-end program execution under JIT with cycle contract
    let source = r#"
@0ms: {
    routine elastic_compute(val: int) -> int taking 500 cycles {
        yield val * 10
    }

    let out = elastic_compute(42)
    debug(out)
}
"#;

    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, "test_elastic_compute")
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    ir = causm_ir::optimize::optimize_program(ir);

    let mut vm2 = Vm::new();
    vm2.jit_enabled = true;
    vm2.execute_program(&ir).expect("Execution failed");

    let out_reg = vm2.symbols.get("out").expect("Symbol 'out' not found");
    assert_eq!(
        vm2.root_timeline.arena.peek(out_reg.0).unwrap(),
        Payload::Integer(420)
    );
    assert!(vm2.root_timeline.local_clock >= 500);
}

