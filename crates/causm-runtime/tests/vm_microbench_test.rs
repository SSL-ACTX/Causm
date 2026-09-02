use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::lower;
use causm_frontend::parser;
use causm_runtime::vm::Vm;
use std::time::Instant;

fn setup_vm_and_program(source: &str) -> (Vm, causm_ir::IrProgram) {
    let program = parser::parse_causm(source).expect("AST parsing failed");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer
        .analyze_program(&program)
        .expect("Semantic analysis failed");

    let mut ir_program = lower::lower_program(&program);
    causm_ir::optimize::prune_unreachable_routines(&mut ir_program);
    ir_program = causm_ir::optimize::optimize_program(ir_program);

    let mut vm = Vm::new();
    vm.debug_mode = false;
    vm.root_timeline.max_cycles_watchdog = 50_000_000;
    vm.root_timeline.arena.capacity = 100 * 1024 * 1024;
    (vm, ir_program)
}

#[ignore]
#[test]
fn test_perf_tight_loop_100k_iterations() {
    let source = r#"
    @0ms: {
        let sum = 0
        for i in 0..100000 step _ {
            sum = sum + i
        }
    }
    "#;

    let (mut vm, ir_program) = setup_vm_and_program(source);
    vm.debug_mode = false;

    let exec_start = Instant::now();
    vm.execute_program(&ir_program)
        .expect("VM execution failed");
    let exec_duration = exec_start.elapsed();

    let total_nanos = exec_duration.as_nanos();
    let nanos_per_iter = total_nanos / 100_000;

    println!("\n=======================================================");
    println!(" [PERF] 100,000 Loop Iterations Benchmark");
    println!(" - Total VM Execution Time: {:?}", exec_duration);
    println!(
        " - Cycles Executed: {}",
        vm.root_timeline.total_executed_cycles
    );
    println!(" - Average Cost per Loop Step: {} ns/iter", nanos_per_iter);
    println!("=======================================================");
}

#[ignore]
#[test]
fn test_perf_recursive_fibonacci_n20() {
    let source = r#"
    @0ms: {
        routine fib(n: i64) -> i64 taking 50ms {
            if n <= 1 {
                return n
            }
            return fib(n - 1) + fib(n - 2)
        }

        let res = fib(20)
    }
    "#;

    let (mut vm, ir_program) = setup_vm_and_program(source);

    let start = Instant::now();
    vm.execute_program(&ir_program)
        .expect("VM execution failed");
    let duration = start.elapsed();

    // fib(20) results in 21,891 function calls
    let total_calls = 21_891;
    let ns_per_call = duration.as_nanos() / total_calls;

    println!("\n=======================================================");
    println!(" [PERF] Recursive Call Stack Benchmark (fib(20))");
    println!(" - Total Recursive Frame Invocations: {}", total_calls);
    println!(" - Total Execution Time: {:?}", duration);
    println!(" - Cost per Call Frame Dispatch: {} ns/call", ns_per_call);
    println!("=======================================================");
}

#[ignore]
#[test]
fn test_perf_epoch_arena_allocation_and_reclamation() {
    let source = r#"
    @0ms: {
        for i in 0..20000 step _ {
            let buffer = "telemetry_frame_epoch_buffer"
            let flag = len(buffer) > 5
        }
    }
    "#;

    let (mut vm, ir_program) = setup_vm_and_program(source);

    let start = Instant::now();
    vm.execute_program(&ir_program)
        .expect("VM execution failed");
    let duration = start.elapsed();

    let total_nanos = duration.as_nanos();
    let ns_per_alloc = total_nanos / 20_000;

    println!("\n=======================================================");
    println!(" [PERF] Epoch Memory Arena Lifecycle (20,000 alloc cycles)");
    println!(" - Total Execution Time: {:?}", duration);
    println!(
        " - Allocation & Entropic Transition Rate: {} ns/cycle",
        ns_per_alloc
    );
    println!("=======================================================");
}
