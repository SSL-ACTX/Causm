use causm_core::{Program, Span};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CURRENT_PROTOCOL_VERSION: &str = "0.1.0";
pub const CURRENT_COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Note,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PluginDiagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    Success,
    Error(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PluginPhase {
    AstTransform,
    PostAnalysis,
    IrEmit,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct AnalysisArtifacts {
    pub verification_passed: bool,
    pub timeline_count: usize,
    pub total_estimated_cost: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PluginRequest {
    pub protocol_version: String,
    pub compiler_version: String,
    pub target_arch: String,
    pub target_os: String,
    pub file_path: String,
    pub phase: PluginPhase,
    pub ast: Program,
    pub analysis: Option<AnalysisArtifacts>,
    pub options: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PluginResponse {
    pub status: PluginStatus,
    pub modified_ast: Option<Program>,
    pub emitted_payload: Option<Vec<u8>>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl PluginRequest {
    pub fn new(file_path: impl Into<String>, ast: Program) -> Self {
        Self {
            protocol_version: CURRENT_PROTOCOL_VERSION.to_string(),
            compiler_version: CURRENT_COMPILER_VERSION.to_string(),
            target_arch: std::env::consts::ARCH.to_string(),
            target_os: std::env::consts::OS.to_string(),
            file_path: file_path.into(),
            phase: PluginPhase::AstTransform,
            ast,
            analysis: None,
            options: HashMap::new(),
        }
    }

    pub fn with_phase(mut self, phase: PluginPhase) -> Self {
        self.phase = phase;
        self
    }

    pub fn with_analysis(mut self, analysis: AnalysisArtifacts) -> Self {
        self.analysis = Some(analysis);
        self
    }

    pub fn with_target(
        mut self,
        target_arch: impl Into<String>,
        target_os: impl Into<String>,
    ) -> Self {
        self.target_arch = target_arch.into();
        self.target_os = target_os.into();
        self
    }

    pub fn with_options(mut self, options: HashMap<String, String>) -> Self {
        self.options = options;
        self
    }
}

impl PluginResponse {
    pub fn success(
        modified_ast: Option<Program>,
        diagnostics: Vec<PluginDiagnostic>,
    ) -> Self {
        Self {
            status: PluginStatus::Success,
            modified_ast,
            emitted_payload: None,
            diagnostics,
        }
    }

    pub fn emit_payload(
        payload: Vec<u8>,
        diagnostics: Vec<PluginDiagnostic>,
    ) -> Self {
        Self {
            status: PluginStatus::Success,
            modified_ast: None,
            emitted_payload: Some(payload),
            diagnostics,
        }
    }

    pub fn error(
        message: impl Into<String>,
        diagnostics: Vec<PluginDiagnostic>,
    ) -> Self {
        Self {
            status: PluginStatus::Error(message.into()),
            modified_ast: None,
            emitted_payload: None,
            diagnostics,
        }
    }
}
