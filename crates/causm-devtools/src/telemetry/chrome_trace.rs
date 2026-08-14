// crates/causm-devtools/src/telemetry/chrome_trace.rs
use crate::TraceEvent;
use serde_json::json;

pub fn export_chrome_trace(events: &[TraceEvent]) -> String {
    let trace_events: Vec<_> = events
        .iter()
        .map(|e| {
            json!({
                "name": e.message,
                "cat": format!("{:?}", e.layer),
                "ph": "i",
                "ts": e.timestamp_ms * 1000,
                "pid": 1,
                "tid": e.timeline_branch,
                "args": {
                    "span": e.span
                }
            })
        })
        .collect();

    let output = json!({
        "traceEvents": trace_events,
        "displayTimeUnit": "ms"
    });
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}
