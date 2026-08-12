use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceLayer {
    Frontend,
    Analysis,
    Runtime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub timestamp_ms: u64,
    pub timeline_branch: String,
    pub layer: TraceLayer,
    pub span: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Tracer {
    events: Arc<Mutex<Vec<TraceEvent>>>,
    pub trace_terminal: bool,
}

impl Tracer {
    pub fn new(trace_terminal: bool) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            trace_terminal,
        }
    }

    pub fn emit(
        &self,
        timestamp_ms: u64,
        branch: &str,
        layer: TraceLayer,
        span: Option<&str>,
        message: &str,
    ) {
        let event = TraceEvent {
            timestamp_ms,
            timeline_branch: branch.to_string(),
            layer: layer.clone(),
            span: span.map(|s| s.to_string()),
            message: message.to_string(),
        };

        if self.trace_terminal {
            let layer_tag = match layer {
                TraceLayer::Frontend => "[FRONTEND]".magenta().bold(),
                TraceLayer::Analysis => "[ANALYSIS]".yellow().bold(),
                TraceLayer::Runtime => "[TVM]".cyan().bold(),
            };
            let span_str = span.map(|s| format!(" ({})", s)).unwrap_or_default();
            println!(
                "{} @{}ms [{}] {}{}: {}",
                layer_tag, timestamp_ms, branch, message, span_str, event.message
            );
        }

        if let Ok(mut lock) = self.events.lock() {
            lock.push(event);
        }
    }

    pub fn export_json(&self) -> String {
        if let Ok(lock) = self.events.lock() {
            serde_json::to_string_pretty(&*lock).unwrap_or_else(|_| "[]".to_string())
        } else {
            "[]".to_string()
        }
    }
}
