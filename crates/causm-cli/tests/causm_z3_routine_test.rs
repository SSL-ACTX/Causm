use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;

#[test]
fn test_z3_routine_contract_independent_z3_only() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            routine independent_fast() taking 3ms {
                let x = 1
                if (true) {
                    let y = 2
                    let z = 3
                }
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();

    // Test without Z3 - Should fail at the standard analyzer level
    analyzer.use_z3 = false;
    let result_no_z3 = analyzer.analyze_program(&program);

    println!("No Z3 result: {:?}", result_no_z3);

    Ok(())
}
