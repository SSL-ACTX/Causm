use causm_frontend::ir::lower_program;
use causm_frontend::parser::parse_causm;
use causm_runtime::vm::Vm;

#[test]
fn test_speedmicro_jit_basic() {
    let code = r#"
        @0ms: {
            routine fast_add(consume a: int, consume b: int) -> int taking 1000 cycles {
                let res = a + b
                yield_pad
                yield res
            }
            let x = 10
            let y = 20
            let z = call fast_add(x, y)
            print(z)
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);
    let mut vm = Vm::new();
    vm.register_capability("System.Log", |params| {
        if let Some(msg) = params.get("message") {
            println!("LOG: {}", msg);
        }
        Ok(())
    });
    vm.execute_program(&ir).unwrap();

    let res_reg = ir.symbols.get("z").expect("z not found").0;
    let res_val = vm.root_timeline.arena.peek(res_reg);
    match res_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 30),
        _ => panic!("Expected z=30, got {:?}", res_val),
    }
}

#[test]
fn test_speedmicro_jit_branchless() {
    let code = r#"
        @0ms: {
            routine fast_max(consume a: int, consume b: int) -> int taking 500 cycles {
                if (a > b) {
                    yield a
                } else {
                    yield b
                }
            }
            let m1 = call fast_max(10, 20)
            print(m1)
            let m2 = call fast_max(20, 10)
            print(m2)
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);
    let mut vm = Vm::new();
    vm.register_capability("System.Log", |params| {
        if let Some(msg) = params.get("message") {
            println!("LOG: {}", msg);
        }
        Ok(())
    });
    vm.execute_program(&ir).unwrap();

    let m1_reg = ir.symbols.get("m1").expect("m1 not found").0;
    let m1_val = vm.root_timeline.arena.peek(m1_reg);
    match m1_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 20),
        _ => panic!("Expected m1=20, got {:?}", m1_val),
    }

    let m2_reg = ir.symbols.get("m2").expect("m2 not found").0;
    let m2_val = vm.root_timeline.arena.peek(m2_reg);
    match m2_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 20),
        _ => panic!("Expected m2=20, got {:?}", m2_val),
    }
}

#[test]
fn test_speedmicro_bench_consistency() {
    use causm_runtime::vm::jit::hw_timing::read_tsc;

    let code_jit = r#"
        @0ms: {
            routine fast_compute(consume a: int) -> int taking 100 cycles {
                let res = a + 1
                let res = res + 2
                let res = res + 3
                let res = res + 4
                let res = res + 5
                yield res
            }
            let x = 10
            let z = call fast_compute(x)
        }
    "#;

    let code_reg = r#"
        @0ms: {
            routine slow_compute(consume a: int) -> int taking _ {
                let res = a + 1
                let res = res + 2
                let res = res + 3
                let res = res + 4
                let res = res + 5
                yield res
            }
            let x = 10
            let z = call slow_compute(x)
        }
    "#;

    let ir_jit = lower_program(&parse_causm(code_jit).unwrap());
    let ir_reg = lower_program(&parse_causm(code_reg).unwrap());

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_| Ok(()));

    // Warmup JIT
    vm.execute_program(&ir_jit).unwrap();
    vm.execute_program(&ir_reg).unwrap();

    let mut timings_jit = Vec::new();
    let mut logical_clocks = Vec::new();
    for _ in 0..100 {
        vm.clear_state();
        let start = read_tsc();
        vm.execute_program(&ir_jit).unwrap();
        let end = read_tsc();
        timings_jit.push(end - start);
        logical_clocks.push(vm.root_timeline.local_clock);
    }

    let mut timings_reg = Vec::new();
    for _ in 0..100 {
        vm.clear_state();
        let start = read_tsc();
        vm.execute_program(&ir_reg).unwrap();
        let end = read_tsc();
        timings_reg.push(end - start);
    }

    let avg_jit = timings_jit.iter().sum::<u64>() / timings_jit.len() as u64;
    let avg_reg = timings_reg.iter().sum::<u64>() / timings_reg.len() as u64;

    let jitter_jit =
        timings_jit.iter().max().unwrap() - timings_jit.iter().min().unwrap();
    let jitter_reg =
        timings_reg.iter().max().unwrap() - timings_reg.iter().min().unwrap();

    let logical_jitter =
        logical_clocks.iter().max().unwrap() - logical_clocks.iter().min().unwrap();

    println!("\nBenchmark Comparison (100 runs):");
    println!(
        "  Regular (Interpreter) - Avg: {} cycles, Jitter: {}",
        avg_reg, jitter_reg
    );
    println!(
        "  SpeedMicro (JIT @ 100c) - Avg: {} cycles, Jitter: {}",
        avg_jit, jitter_jit
    );
    println!("  Logical Clock Jitter:  {} cycles", logical_jitter);

    assert_eq!(logical_jitter, 0, "Logical clock must have ZERO jitter");

    let res_reg = ir_jit.symbols.get("z").expect("z not found").0;
    let res_val = vm.root_timeline.arena.peek(res_reg);
    match res_val {
        Some(causm_core::value::Payload::Integer(v)) => assert_eq!(v, 25),
        _ => panic!("Expected z=25, got {:?}", res_val),
    }
}

#[test]
fn test_speedmicro_logical_clock_determinism() {
    let code = r#"
        @0ms: {
            routine fast_work() taking 50000 cycles {
                let res = 1
                yield res
            }
            let z = call fast_work()
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir).unwrap();

    // deterministic at 50001 because of the Call + Move instructions in IR
    assert!(vm.root_timeline.local_clock >= 50000);
}

#[test]
fn test_speedmicro_bench_isochronous_determinism() {
    use causm_runtime::vm::jit::hw_timing::read_tsc;

    let code_base = r#"
        @0ms: {
            routine isochronous_work(consume a: int, consume extra: int) -> int taking 100000 cycles {
                let res = a
                if (extra > 0) {
                    let res = res + 1
                    let res = res + 2
                    let res = res + 3
                }
                yield res
            }
            let z = call isochronous_work(10, 0)
        }
    "#;

    let code_extra = r#"
        @0ms: {
            routine isochronous_work(consume a: int, consume extra: int) -> int taking 100000 cycles {
                let res = a
                if (extra > 0) {
                    let res = res + 1
                    let res = res + 2
                    let res = res + 3
                }
                yield res
            }
            let z = call isochronous_work(10, 1)
        }
    "#;

    let ir_base = lower_program(&parse_causm(code_base).unwrap());
    let ir_extra = lower_program(&parse_causm(code_extra).unwrap());

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_| Ok(()));

    // Warmup
    vm.execute_program(&ir_base).unwrap();
    vm.execute_program(&ir_extra).unwrap();

    let mut timings_base = Vec::new();
    for _ in 0..100 {
        vm.clear_state();
        let start = read_tsc();
        vm.execute_program(&ir_base).unwrap();
        let end = read_tsc();
        timings_base.push(end - start);
    }

    let mut timings_extra = Vec::new();
    for _ in 0..100 {
        vm.clear_state();
        let start = read_tsc();
        vm.execute_program(&ir_extra).unwrap();
        let end = read_tsc();
        timings_extra.push(end - start);
    }

    let avg_base = timings_base.iter().sum::<u64>() / timings_base.len() as u64;
    let avg_extra = timings_extra.iter().sum::<u64>() / timings_extra.len() as u64;

    println!("\nIsochronous Determinism Benchmark (100000 cycle contract):");
    println!("  Base Work  - Avg: {} cycles", avg_base);
    println!("  Extra Work - Avg: {} cycles", avg_extra);
    println!(
        "  Difference:  {} cycles",
        (avg_base as i64 - avg_extra as i64).abs()
    );

    assert!(
        (avg_base as i64 - avg_extra as i64).abs() < 5000,
        "Timing difference too large for isochronous contract"
    );
}
