pub mod analyzer;
pub mod expression;
pub mod resolve;
pub mod ssa;
pub mod statement;
pub mod statements;

pub use analyzer::{
    BranchState, EntropicAnalyzer, RoutineInfo, SemanticError, SemanticErrorKind,
};
pub use expression::cost::estimate_expression_cost;
pub use expression::inference::infer_expression_type;
pub use resolve::{resolve_method_call, MethodTargetResolution, ResolveStage};
pub use ssa::{LiveRange, LiveRangeTable, SsaStage};
pub use statement::{estimate_block_cost, estimate_statement_cost};

pub use analyzer::core::{get_default_pipeline_runner, set_default_pipeline_runner};
