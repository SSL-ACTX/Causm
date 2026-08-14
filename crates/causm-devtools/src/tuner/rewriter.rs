// crates/causm-devtools/src/tuner/rewriter.rs

pub fn patch_routine_contract(
    source: &str,
    routine_name: &str,
    tuned_ms: u64,
) -> String {
    let target_needle = format!("routine {}", routine_name);
    if let Some(pos) = source.find(&target_needle) {
        let after = &source[pos..];
        if let Some(q_pos) = after.find("taking ?") {
            let full_q_pos = pos + q_pos;
            let mut result = String::new();
            result.push_str(&source[..full_q_pos]);
            result.push_str(&format!("taking {}ms", tuned_ms));
            result.push_str(&source[full_q_pos + "taking ?".len()..]);
            return result;
        }
    }
    source.to_string()
}
