// crates/causm-devtools/src/telemetry/speedscope.rs
use crate::TraceEvent;
use serde_json::json;

pub fn export_speedscope_json(events: &[TraceEvent]) -> String {
    let output = json!({
        "$schema": "https://www.speedscope.app/file-format-schema.json",
        "shared": {
            "frames": events.iter().map(|e| json!({"name": e.message})).collect::<Vec<_>>()
        },
        "profiles": [{
            "type": "evented",
            "name": "causm-speedscope-profile",
            "unit": "milliseconds",
            "startValue": 0,
            "endValue": events.last().map(|e| e.timestamp_ms).unwrap_or(0),
            "events": []
        }]
    });
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}
