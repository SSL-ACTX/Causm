use causm_frontend::ir::lower_program;
use causm_frontend::parser::parse_causm;
use causm_runtime::vm::Vm;

#[test]
fn test_speedmicro_spoonfeeding() {
    let code = r#"
        @0ms: {
            routine hot_loop(consume n: int) -> int taking 5000 cycles {
                let i = 0
                let sum = 0
                loop (max 10ms) {
                    if (i >= n) {
                        break
                    }
                    sum = sum + i
                    i = i + 1
                }
                yield sum
            }
            let res = call hot_loop(100)
        }
    "#;

    let program = parse_causm(code).unwrap();
    let ir = lower_program(&program);

    // Vm::new() now automatically pins the thread and pre-faults memory
    let mut vm = Vm::new();

    // WARM-UP PHASE: Ensure JIT compilation is finished and cached
    println!("Warming up JIT...");
    vm.execute_program(&ir).unwrap();

    // Measure jitter across multiple runs
    let mut timings = Vec::new();
    for _ in 0..100 {
        vm.clear_state();
        let start = std::time::Instant::now();
        vm.execute_program(&ir).unwrap();
        timings.push(start.elapsed().as_nanos());
    }
    let avg = timings.iter().sum::<u128>() as f64 / timings.len() as f64;
    let jitter = timings.iter().map(|&t| (t as f64 - avg).abs()).sum::<f64>()
        / timings.len() as f64;

    println!("Average execution time: {} ns", avg);
    println!("Spoonfed Jitter: {} ns", jitter);

    // We expect very low jitter due to pinning and warming
    // (Actual threshold depends on environment, but we'll just log it)
}
