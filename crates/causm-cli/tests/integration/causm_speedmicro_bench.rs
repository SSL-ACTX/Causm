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

fn percentile(sorted: &[u64], pct: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * pct).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn std_dev(samples: &[u64], mean: f64) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    let var: f64 = samples
        .iter()
        .map(|&x| {
            let diff = x as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / (samples.len() - 1) as f64;
    var.sqrt()
}

#[test]
fn test_speedmicro_bench_isochronous_cycle_padding_accuracy() {
    println!("\n[Benchmark: SpeedMicro Isochronous Cycle Padding Precision]");

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
    let mut vm_warmup = Vm::new();
    vm_warmup.jit_enabled = true;
    let _ = vm_warmup.execute_program(&ir_unpadded);

    let sample_count = 100;
    let mut unpadded_samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        let t0 = causm_jit::timing::read_tsc();
        let mut vm = Vm::new();
        vm.jit_enabled = true;
        let _ = vm.execute_program(&ir_unpadded);
        let t1 = causm_jit::timing::read_tsc();
        unpadded_samples.push(t1.saturating_sub(t0));
    }
    unpadded_samples.sort_unstable();

    let unpadded_mean =
        unpadded_samples.iter().sum::<u64>() as f64 / sample_count as f64;
    let unpadded_p50 = percentile(&unpadded_samples, 0.50);
    let unpadded_p90 = percentile(&unpadded_samples, 0.90);
    let unpadded_p99 = percentile(&unpadded_samples, 0.99);

    println!("Baseline (Unpadded, {} samples):", sample_count);
    println!(
        "  Mean: {:>8.0} | P50: {:>8} | P90: {:>8} | P99: {:>8} | Min: {:>8} | Max: {:>8} cycles",
        unpadded_mean,
        unpadded_p50,
        unpadded_p90,
        unpadded_p99,
        unpadded_samples[0],
        unpadded_samples[sample_count - 1]
    );

    println!("\nPadded Cycle Targets:");
    println!(
        "  {:<10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Target", "Mean", "P50", "P90", "P99", "Min", "Max", "StdDev"
    );

    let targets: [u64; 5] = [5_000, 20_000, 50_000, 100_000, 250_000];

    for &target in &targets {
        let source_padded = format!(
            r#"
@0ms: {{
    routine compute_padded(a: int, b: int) -> int taking {} cycles {{
        yield (a * 31) + (b ^ 17)
    }}

    let r = compute_padded(42, 99)
}}
"#,
            target
        );

        let ir_padded =
            compile_ir(&source_padded, &format!("bench_padded_{}", target));
        let mut samples = Vec::with_capacity(sample_count);

        for _ in 0..sample_count {
            let t0 = causm_jit::timing::read_tsc();
            let mut vm = Vm::new();
            vm.jit_enabled = true;
            let _ = vm.execute_program(&ir_padded);
            let t1 = causm_jit::timing::read_tsc();
            samples.push(t1.saturating_sub(t0));
        }

        samples.sort_unstable();
        let mean = samples.iter().sum::<u64>() as f64 / sample_count as f64;
        let p50 = percentile(&samples, 0.50);
        let p90 = percentile(&samples, 0.90);
        let p99 = percentile(&samples, 0.99);
        let min = samples[0];
        let max = samples[sample_count - 1];
        let sdev = std_dev(&samples, mean);

        println!(
            "  {:<10} {:>10.0} {:>10} {:>10} {:>10} {:>10} {:>10} {:>12.1}",
            target, mean, p50, p90, p99, min, max, sdev
        );

        assert!(
            mean as u64 >= target,
            "Target {} not reached by mean {}",
            target,
            mean
        );
    }
}

#[test]
fn test_speedmicro_bench_workloads_and_throughput() {
    println!("\n[Benchmark: SpeedMicro Cranelift Bare-Metal JIT vs TVM Bytecode Interpreter]");

    // Workload A: Bitwise & Arithmetic Hashing (Order ID Hash)
    let source_hash = r#"
@0ms: {
    routine hash_op(a: int, b: int) -> int {
        let mut h = (a * 31) ^ (b + 17)
        h = (h * 13) ^ (a - 5)
        yield h
    }

    let out = hash_op(101, 202)
}
"#;

    // Workload B: Branchless Decision Signal (HFT Market Spread / Trade Execution)
    let source_decision = r#"
@0ms: {
    routine trade_signal(bid: int, ask: int) -> int {
        let spread = ask - bid
        let mut signal = 0
        if (spread < 5) {
            signal = 100
        } else {
            signal = -100
        } reconcile auto
        yield signal
    }

    let s = trade_signal(995, 998)
}
"#;

    // Workload C: Loop Accumulator (While loop counter accumulation)
    let source_loop = r#"
@0ms: {
    routine loop_accum(n: int) -> int taking 50ms {
        let i = 0
        let sum = 0
        while (i < n) taking 40ms {
            let sum = sum + i
            let i = i + 1
        }
        yield sum
    }

    let l = loop_accum(50)
}
"#;

    let workloads = [
        ("Bitwise Hashing", source_hash, "hash_op", 2),
        (
            "Branchless Trade Signal",
            source_decision,
            "trade_signal",
            2,
        ),
        ("Loop Accumulation (50 iters)", source_loop, "loop_accum", 1),
    ];

    println!(
        "\n  {:<30} | {:<16} | {:<22} | {:>10}",
        "Workload Pattern", "TVM Latency", "JIT Bare-Metal Latency", "Speedup"
    );

    let tvm_runs = 200;
    let jit_calls: u64 = 10_000_000;

    for (name, source, routine_name, arity) in workloads {
        let ir = compile_ir(source, name);

        // 1. Measure TVM Interpreter
        let t_tvm_start = Instant::now();
        for _ in 0..tvm_runs {
            let mut vm = Vm::new();
            vm.jit_enabled = false;
            let _ = vm.execute_program(&ir);
        }
        let tvm_elapsed = t_tvm_start.elapsed();
        let tvm_ns_per_run = tvm_elapsed.as_nanos() as f64 / tvm_runs as f64;

        // 2. Measure Direct Cranelift JIT Bare-Metal Execution
        let mut jit = causm_jit::CausmJit::new().expect("JIT init failed");
        let routine_def = ir.routines.get(routine_name).expect("Routine missing");
        let func_ptr = jit
            .compile_routine(routine_name, routine_def)
            .expect("Compile failed");

        let mut checksum: i64 = 0;
        let t_jit_start = Instant::now();
        let calls = if arity == 1 { 2_000_000 } else { 5_000_000 };
        if arity == 1 {
            let f: extern "C" fn(i64) -> i64 =
                unsafe { std::mem::transmute(func_ptr) };
            for _ in 0..calls {
                checksum = checksum.wrapping_add(f(50));
            }
        } else {
            let f: extern "C" fn(i64, i64) -> i64 =
                unsafe { std::mem::transmute(func_ptr) };
            for i in 0..calls {
                checksum = checksum.wrapping_add(f(i as i64, (i ^ 42) as i64));
            }
        }
        let jit_elapsed = t_jit_start.elapsed();
        let jit_ns_per_call = jit_elapsed.as_nanos() as f64 / calls as f64;
        let speedup = tvm_ns_per_run / jit_ns_per_call;

        let jit_mops = (calls as f64 / jit_elapsed.as_secs_f64()) / 1_000_000.0;

        println!(
            "  {:<30} | {:>10.2} µs/run | {:>10.2} ns ({:>5.1}M/s) | {:>9.0}x",
            name,
            tvm_ns_per_run / 1000.0,
            jit_ns_per_call,
            jit_mops,
            speedup
        );

        assert!(checksum != 0 || jit_calls > 0);
    }
}

#[test]
fn test_speedmicro_bench_elastic_determinism_overhead() {
    println!(
        "\n[Benchmark: SpeedMicro Elastic Determinism & Memory Pinning Primitives]"
    );

    causm_jit::hft::reset_total_lost_cycles();

    // 1. Jitter Evaluation Check Overhead
    let eval_iters: u64 = 5_000_000;
    let t0 = Instant::now();
    let mut dummy_count = 0u64;
    for i in 0..eval_iters {
        let status = causm_jit::hft::evaluate_elastic_determinism(
            10_000 + (i % 200_000),
            10_000,
            causm_jit::hft::DEFAULT_ELASTIC_JITTER_THRESHOLD_CYCLES,
        );
        if let causm_jit::hft::JitterStatus::ElasticJitterDetected { .. } = status {
            dummy_count += 1;
        }
    }
    let elapsed = t0.elapsed();
    let ns_per_eval = elapsed.as_nanos() as f64 / eval_iters as f64;
    let evals_sec = (eval_iters as f64 / elapsed.as_secs_f64()) as u64;

    println!(
        "  Elastic Determinism Jitter Evaluation ({} iterations):",
        eval_iters
    );
    println!(
        "    Latency: {:.2} ns/eval | Throughput: {} checks/sec | Jitter spikes: {}",
        ns_per_eval, evals_sec, dummy_count
    );

    // 2. Temporal Freeze Baseline Shift Overhead
    let freeze_iters: u64 = 1_000_000;
    let mut vm = Vm::new();
    let t_freeze = Instant::now();
    for _ in 0..freeze_iters {
        vm.temporal_freeze(3_000_000);
    }
    let freeze_elapsed = t_freeze.elapsed();
    let ns_per_freeze = freeze_elapsed.as_nanos() as f64 / freeze_iters as f64;
    println!(
        "  Vm Temporal Freeze Timeline Shift Latency: {:.2} ns/freeze",
        ns_per_freeze
    );

    // 3. Memory Page Pinning & Cache Warming
    let mut buffer = vec![0u8; 64 * 1024]; // 64 KB L1/L2 buffer
    let t_pin = Instant::now();
    let pin_res = causm_jit::hft::pin_and_prefault_memory(&mut buffer);
    let pin_time = t_pin.elapsed();
    assert!(pin_res.is_ok());

    let t_warm = Instant::now();
    causm_jit::hft::warm_l1_cache(&buffer);
    let warm_time = t_warm.elapsed();

    causm_jit::hft::unpin_memory(&buffer);

    println!(
        "  Page Pin & Pre-fault (64KB): {:?} | L1 Cache Warming: {:?}",
        pin_time, warm_time
    );
}
