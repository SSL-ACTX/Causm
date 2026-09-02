use causm_analysis::analyzer::EntropicAnalyzer;
use causm_analysis::oxiz::OxiZBackend;
use causm_analysis::solver::{extract_facts, EntropicDiagnostic, RelationalInvariantSolver};
use causm_analysis::ssa::LiveRangeTable;
use causm_frontend::parser;

#[test]
fn test_entropius_relational_fact_extraction_and_ssa_points() {
    let source = r#"
        @main: {
            let a = 100
            let b = 200
            let c = a + b
            yield a
            let d = c
        }
    "#;

    let program = parser::parse_causm(source).expect("Program should parse");
    let facts = extract_facts(&program, source, "<test>");

    assert!(facts.var_origins.contains_key("a"));
    assert!(facts.var_origins.contains_key("b"));
    assert!(facts.var_origins.contains_key("c"));
    assert!(facts.var_consumes.contains_key("a"));

    let liveness = LiveRangeTable::compute_from_facts(&facts);
    assert!(liveness.ranges.contains_key("a"));
    assert!(liveness.ranges["a"].is_consumed);

    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    assert!(solver.solve_invariants(&facts).is_ok());
}

#[test]
fn test_entropius_invariant_1_use_after_consume_relational_detection() {
    let source = r#"
        @main: {
            let data = 42
            yield data
            let fail = data + 1
        }
    "#;

    let program = parser::parse_causm(source).expect("Program should parse");
    let facts = extract_facts(&program, source, "<test>");

    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let result = solver.solve_invariants(&facts);

    assert!(result.is_err());
    let err_str = format!("{}", result.unwrap_err());
    assert!(err_str.contains("data"));
}

#[test]
fn test_entropius_fine_grained_diagnostic_and_smt_formula_emission() {
    let source = r#"
        @main: {
            let buffer = 1024
            yield buffer
            let leak = buffer * 2
        }
    "#;

    let program = parser::parse_causm(source).expect("Program should parse");
    let facts = extract_facts(&program, source, "<test>");

    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diagnostics = solver.collect_diagnostics(&facts);

    assert_eq!(diagnostics.len(), 1);
    match &diagnostics[0] {
        EntropicDiagnostic::UseAfterConsume {
            var,
            origin_point,
            consume_point,
            access_point,
            smt_formula,
        } => {
            assert_eq!(var, "buffer");
            assert!(origin_point.is_some());
            assert!(consume_point < access_point);
            assert!(smt_formula.contains("IllegalConsumeAccess(buffer"));
            assert!(smt_formula.contains("LinearConsume(buffer"));
            assert!(smt_formula.contains("UNSAT proof"));

            let rendered = diagnostics[0].format_diagnostic(true);
            assert!(rendered.contains("error[E0001]"));
            assert!(rendered.contains("buffer"));
            assert!(rendered.contains("consumed"));
            assert!(rendered.contains("SMT"));
        }
        _ => panic!("Expected UseAfterConsume diagnostic"),
    }
}

#[test]
fn test_entropius_lease_safety_relational_detection() {
    let source = r#"
        @main: {
            let resource = 99
            lease borrowed = resource for 10ms {
                let x = borrowed
            }
        }
    "#;

    let program = parser::parse_causm(source).expect("Program should parse");
    let facts = extract_facts(&program, source, "<test>");

    assert!(facts.active_leases.contains_key("resource"));
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    assert!(solver.solve_invariants(&facts).is_ok());
}
