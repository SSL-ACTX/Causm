// crates/causm-devtools/src/tuner/rewriter.rs

pub fn patch_routine_contract(
    source: &str,
    routine_name: &str,
    tuned_ms: u64,
) -> String {
    let target_needle = format!("routine {}", routine_name);
    if let Some(pos) = source.find(&target_needle) {
        let after = &source[pos..];
        // Look for existing 'taking <contract>' before the routine body '{'
        if let Some(brace_pos) = after.find('{') {
            let header = &after[..brace_pos];
            if let Some(taking_idx) = header.find("taking ") {
                let contract_start = pos + taking_idx;
                let contract_rest = &source[contract_start..];
                // Find end of taking token (space, where, or {)
                let token_end = contract_rest
                    .find(|c: char| c == '{' || c == '\n' || c == '\r' || c == 'w')
                    .unwrap_or(header.len());
                let old_contract = contract_rest[..token_end].trim_end();

                let mut result = String::new();
                result.push_str(&source[..contract_start]);
                result.push_str(&format!("taking {}ms", tuned_ms));
                result.push_str(&source[contract_start + old_contract.len()..]);
                return result;
            }
        }
    }
    source.to_string()
}
