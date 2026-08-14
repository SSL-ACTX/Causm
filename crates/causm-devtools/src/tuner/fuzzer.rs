use causm_frontend::lower::lower_program;
use causm_frontend::parser;
use causm_runtime::vm::Vm;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct FuzzConfig {
    pub iterations: usize,
    pub chaos_jitter_ms: u64,
    pub safety_margin_pct: f64,
    pub target: Option<String>,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            iterations: 1000,
            chaos_jitter_ms: 5,
            safety_margin_pct: 15.0,
            target: None,
        }
    }
}

pub struct FuzzResult {
    pub p99_wcet_ms: u64,
    pub sample_durations_ms: Vec<u64>,
    pub max_duration_ms: u64,
    pub min_duration_ms: u64,
}

pub fn fuzz_routine_wcet(
    source: &str,
    config: &FuzzConfig,
) -> Result<FuzzResult, String> {
    let program = parser::parse_causm(source).map_err(|e| e.to_string())?;

    // 1. Calculate static worst-case baseline from the analyzer
    let mut analyzer = causm_analysis::analyzer::EntropicAnalyzer::new();
    let _ = analyzer.analyze_program(&program);
    let static_cost = if let Some(target_name) = &config.target {
        analyzer
            .routines
            .get(target_name)
            .map(|r| r.taking_ms)
            .unwrap_or(0)
    } else {
        0
    };

    let ir = lower_program(&program);
    let mut durations = Vec::with_capacity(config.iterations);

    for _ in 0..config.iterations {
        let mut vm = Vm::new();
        causm_stdlib::register_all(&mut vm);
        // Suppress print / System.Log output during fuzzer sweeps
        vm.capability_handlers.insert(
            "System.Log".to_string(),
            Box::new(|_| Ok(causm_core::value::Payload::Null)),
        );
        vm.root_timeline.entropy_mode = causm_core::EntropyMode::Chaos;

        let start = Instant::now();
        vm.execute_program(&ir).map_err(|e| e.to_string())?;
        let wall_ms = start.elapsed().as_millis() as u64;
        let tvm_clock = vm.root_timeline.local_clock;

        let effective_duration = tvm_clock.max(wall_ms).max(static_cost);
        durations.push(effective_duration);
    }

    let p99 =
        super::statistics::calculate_p99_wcet(&durations, config.safety_margin_pct);
    let max = *durations.iter().max().unwrap_or(&0);
    let min = *durations.iter().min().unwrap_or(&0);

    Ok(FuzzResult {
        p99_wcet_ms: p99.max(static_cost),
        sample_durations_ms: durations,
        max_duration_ms: max,
        min_duration_ms: min,
    })
}
