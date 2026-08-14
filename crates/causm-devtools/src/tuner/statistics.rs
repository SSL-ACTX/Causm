// crates/causm-devtools/src/tuner/statistics.rs

pub fn calculate_p99_wcet(durations_ms: &[u64], safety_margin_pct: f64) -> u64 {
    if durations_ms.is_empty() {
        return 0;
    }
    let mut sorted = durations_ms.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() as f64) * 0.999).floor() as usize;
    let clamped_idx = idx.min(sorted.len() - 1);
    let p99 = sorted[clamped_idx] as f64;
    (p99 * (1.0 + safety_margin_pct / 100.0)).ceil() as u64
}
