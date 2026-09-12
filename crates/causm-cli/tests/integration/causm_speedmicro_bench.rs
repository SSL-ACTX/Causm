use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::{hir, lower, parser};
use causm_runtime::vm::Vm;
use std::time::Instant;

fn compile_ir(source: &str, name: &str) -> causm_ir::IrProgram {
    let program = parser::parse_causm(source).expect("Parse failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program_with_source(&program, source, name)
        .expect("Semantic analysis failed");

    let hir_program = hir::lower_ast_to_hir(&program);
    let mut ir = lower::lower_hir_program(&hir_program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir);
    causm_ir::optimize::optimize_program(ir)
}

#[test]
fn test_speedmicro_bench_isochronous_cycle_padding_accuracy() {
    println!("\n================================================================================");
    println!(" [BENCHMARK] SpeedMicro Isochronous Cycle Padding Precision");
    println!("================================================================================");

    // 1. Unpadded baseline routine
    let source_unpadded = r#"
@0ms: {
    routine compute_raw(a: int, b: int) -> int {
        yield (a * 31) + (b ^ 17)
    }

    let r = compute_raw(42, 99)
}
"#;
    let ir_unpadded = compile_ir(source_unpadded, "bench_unpadded");
    let mut vm_unpadded = Vm::new();
    vm_unpadded.jit_enabled = true;

    // Warm up
    let _ = vm_unpadded.execute_program(&ir_unpadded);

    let mut unpadded_tsc_deltas = Vec::new();
    for _ in 0..50 {
        let t0 = causm_jit::timing::read_tsc();
        let mut vm = Vm::new();
        vm.jit_enabled = true;
        let _ = vm.execute_program(&ir_unpadded);
        let t1 = causm_jit::timing::read_tsc();
        unpadded_tsc_deltas.push(t1.saturating_sub(t0));
    }
    let unpadded_avg = unpadded_tsc_deltas.iter().sum::<u64>() / unpadded_tsc_deltas.len() as u64;
    let unpadded_min = *unpadded_tsc_deltas.iter().min().unwrap();
    let unpadded_max = *unpadded_tsc_deltas.iter().max().unwrap();
    println!(" Baseline (Unpadded):");
    println!("   Avg Cycles: {:>8} | Min: {:>8} | Max: {:>8}", unpadded_avg, unpadded_min, unpadded_max);

    // 2. Padded routines with different cycle targets
    let targets: [u64; 3] = [30_000, 60_000, 120_000];

    for target in targets {
        let source_padded = format!(r#"
@0ms: {{
    routine compute_padded(a: int, b: int) -> int taking {} cycles {{
        yield (a * 31) + (b ^ 17)
    }}

    let r = compute_padded(42, 99)
}}
"#, target);

        let ir_padded = compile_ir(&source_padded, &format!("bench_padded_{}", target));
        let mut padded_tsc_deltas = Vec::new();

        // Run iterations to measure actual TSC deltas
        for _ in 0..30 {
            let t0 = causm_jit::timing::read_tsc();
            let mut vm = Vm::new();
            vm.jit_enabled = true;
            let _ = vm.execute_program(&ir_padded);
            let t1 = causm_jit::timing::read_tsc();
            padded_tsc_deltas.push(t1.saturating_sub(t0));
        }

        let avg = padded_tsc_deltas.iter().sum::<u64>() / padded_tsc_deltas.len() as u64;
        let min = *padded_tsc_deltas.iter().min().unwrap();
        let max = *padded_tsc_deltas.iter().max().unwrap();
        let diff = avg.saturating_sub(target);
        let error_pct = (diff as f64 / target as f64) * 100.0;

        println!(" Target: {:>6} cycles | Measured Avg: {:>8} | Min: {:>8} | Max: {:>8} | Delta: {:>6} ({:.2}%)",
            target, avg, min, max, diff, error_pct
        );

        // Verification: The measured elapsed cycles must be >= target cycles (guaranteed padded)
        assert!(avg >= target, "Padded execution ({}) must be at least target cycles ({})", avg, target);
        assert!(min >= target / 2, "Padding spin must be active");
    }

    println!("================================================================================\n");
}

#[test]
fn test_speedmicro_bench_raw_jit_speed_and_throughput() {
    println!("\n================================================================================");
    println!(" [BENCHMARK] SpeedMicro Bare-Metal JIT vs Bytecode Interpreter Throughput");
    println!("================================================================================");

    // Test program: iterative math routine
    let source = r#"
@0ms: {
    routine compute_hash(n: int) -> int {
        let mut acc = n
        if (acc > 50) {
            acc = acc * 3 + 1
        } else {
            acc = acc * 2 + 7
        } reconcile auto
        yield acc
    }

    let out = compute_hash(42)
}
"#;

    let ir = compile_ir(source, "bench_throughput");

    // Measure TVM Interpreter (jit_enabled = false)
    let iters = 500;
    let tvm_start = Instant::now();
    for _ in 0..iters {
        let mut vm = Vm::new();
        vm.jit_enabled = false;
        let _ = vm.execute_program(&ir);
    }
    let tvm_duration = tvm_start.elapsed();
    let tvm_ns_per_op = tvm_duration.as_nanos() as f64 / iters as f64;
    let tvm_ops_sec = (iters as f64 / tvm_duration.as_secs_f64()) as u64;

    // Measure Cranelift JIT compile + execute per program run
    let jit_start = Instant::now();
    for _ in 0..iters {
        let mut vm = Vm::new();
        vm.jit_enabled = true;
        let _ = vm.execute_program(&ir);
    }
    let jit_duration = jit_start.elapsed();
    let jit_ns_per_op = jit_duration.as_nanos() as f64 / iters as f64;
    let jit_ops_sec = (iters as f64 / jit_duration.as_secs_f64()) as u64;

    // Measure direct Cranelift native function pointer invocation (zero VM overhead)
    let mut jit_engine = causm_jit::CausmJit::new().expect("JIT engine initialization failed");
    let routine = ir.routines.get("compute_hash").expect("Routine not found");
    let func_ptr = jit_engine.compile_routine("compute_hash", routine).expect("Compilation failed");
    let native_func: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(func_ptr) };

    let native_iters: u64 = 10_000_000;
    let native_start = Instant::now();
    let mut black_box = 0i64;
    for i in 0..native_iters {
        black_box = black_box.wrapping_add(native_func(i as i64));
    }
    let native_duration = native_start.elapsed();
    let native_ns_per_call = native_duration.as_nanos() as f64 / native_iters as f64;
    let native_calls_sec = (native_iters as f64 / native_duration.as_secs_f64()) as u64;

    println!(" 1. Bytecode TVM Interpreter (Fresh VM instance per run):");
    println!("    Total Time ({} runs): {:?}", iters, tvm_duration);
    println!("    Throughput: {:>12} runs/sec | Latency: {:.2} µs/run", tvm_ops_sec, tvm_ns_per_op / 1000.0);

    println!("\n 2. Cranelift JIT Compile + VM Runtime Dispatch:");
    println!("    Total Time ({} runs): {:?}", iters, jit_duration);
    println!("    Throughput: {:>12} runs/sec | Latency: {:.2} µs/run", jit_ops_sec, jit_ns_per_op / 1000.0);

    println!("\n 3. Direct Cranelift JIT Machine Code Execution (Bare-Metal):");
    println!("    Total Time ({} calls): {:?}", native_iters, native_duration);
    println!("    Throughput: {:>12} calls/sec | Latency: {:.2} ns/call", native_calls_sec, native_ns_per_call);
    println!("    Verification Output: {}", black_box);

    println!("================================================================================\n");

    assert!(native_ns_per_call < 50.0, "Native machine code call latency should be < 50 ns");
}
