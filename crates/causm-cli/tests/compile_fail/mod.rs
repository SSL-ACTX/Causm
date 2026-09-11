pub mod budget_failures;
pub mod capability_failures;
pub mod entropy_failures;
pub mod relational_failures;
pub mod syntax_failures;
pub mod temporal_failures;
pub mod type_failures;

use causm_analysis::analyzer::{EntropicAnalyzer, SemanticError};
use causm_frontend::parser;

pub fn compile_program(source: &str) -> Result<(), SemanticError> {
    let program = parser::parse_causm(source).expect("Source program must parse");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.source = Some(source.to_string());
    analyzer.filename = Some("<compile_fail_test>".to_string());
    analyzer.analyze_program(&program)
}

pub fn compile_program_with_egc(source: &str) -> Result<(), SemanticError> {
    let program = parser::parse_causm(source).expect("Source program must parse");
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.source = Some(source.to_string());
    analyzer.filename = Some("<compile_fail_test>".to_string());
    analyzer.enforce_egc = true;
    analyzer.analyze_program(&program)
}
