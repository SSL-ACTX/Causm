// crates/causm-devtools/src/telemetry/json.rs
use crate::Tracer;

pub fn export_trace_json(tracer: &Tracer) -> String {
    tracer.export_json()
}
