use causm_frontend::ir::lower_program;
use causm_frontend::parser::parse_causm;
use causm_runtime::vm::Vm;

#[test]
fn test_jit_coverage_binary_ops() {
    let code = r#"
        @0ms: {
            routine coverage_math(consume a: int, consume b: int) -> int taking 100 cycles {
                let r_add = a + b
                let r_sub = a - b
                let r_mul = a * b
                let r_div = a / b
                let r_rem = a % b
                
                let res = 0
                if (r_add == 30) {
                    res = 1
                }
                if (r_sub != 10) {
                    res = 0
                }
                if (a > b) {
                    res = res + 10
                }
                if (b < a) {
                    res = res + 100
                }
                if (a >= 20) {
                    res = res + 1000
                }
                if (b <= 10) {
                    res = res + 10000
                }

                yield res
            }
            let x = 20
            let y = 10
            let z = call coverage_math(x, y)
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir).unwrap();

    let res_reg = ir.symbols.get("z").expect("z not found").0;
    let res_val = vm.root_timeline.arena.peek(res_reg);
    // res should be: 1 (r_add == 30) + 10 (a > b) + 100 (b < a) + 1000 (a >= 20) + 10000 (b <= 10) = 11111
    match res_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 11111),
        _ => panic!("Expected z=11111, got {:?}", res_val),
    }
}

#[test]
fn test_jit_coverage_unary_ops() {
    let code = r#"
        @0ms: {
            routine coverage_unary(consume a: int) -> int taking 100 cycles {
                let b = -a
                let c = !(a > 0)
                let res = 0
                if (b == -10) {
                    res = 1
                }
                if (c == 0) {
                    res = res + 2
                }
                yield res
            }
            let x = 10
            let z = call coverage_unary(x)
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir).unwrap();

    let res_reg = ir.symbols.get("z").expect("z not found").0;
    let res_val = vm.root_timeline.arena.peek(res_reg);
    match res_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 3),
        _ => panic!("Expected z=3, got {:?}", res_val),
    }
}

#[test]
fn test_jit_branchless_optimization() {
    let code = r#"
        @0ms: {
            routine fast_ternary(consume cond: int, consume a: int, consume b: int) -> int taking 100 cycles {
                let res = 0
                if (cond > 0) {
                    res = a
                } else {
                    res = b
                }
                yield res
            }
            let z1 = call fast_ternary(1, 10, 20)
            let z2 = call fast_ternary(0, 10, 20)
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);

    // Verify that CMov is actually emitted in the IR for this routine
    let routine = ir.routines.get("fast_ternary").unwrap();
    let has_cmov = routine
        .instructions
        .iter()
        .any(|i| matches!(i, causm_frontend::ir::Instruction::CMov { .. }));
    assert!(
        has_cmov,
        "CMov instruction should have been emitted for simple ternary-like if"
    );

    let mut vm = Vm::new();
    vm.execute_program(&ir).unwrap();

    let z1_reg = ir.symbols.get("z1").expect("z1 not found").0;
    let z2_reg = ir.symbols.get("z2").expect("z2 not found").0;

    match vm.root_timeline.arena.peek(z1_reg) {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 10),
        _ => panic!("Expected z1=10"),
    }
    match vm.root_timeline.arena.peek(z2_reg) {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 20),
        _ => panic!("Expected z2=20"),
    }
}

#[test]
fn test_jit_consume_interaction() {
    let code = r#"
        @0ms: {
            routine use_param(consume x: int) -> int taking 100 cycles {
                let val = x
                yield val
            }
            let z = call use_param(42)
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir).unwrap();

    let z_reg = ir.symbols.get("z").expect("z not found").0;
    match vm.root_timeline.arena.peek(z_reg) {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 42),
        _ => panic!(
            "Expected z=42, got {:?}",
            vm.root_timeline.arena.peek(z_reg)
        ),
    }
}
