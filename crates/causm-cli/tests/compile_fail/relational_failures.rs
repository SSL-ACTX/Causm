use causm_analysis::analyzer::EntropicAnalyzer;
use causm_analysis::oxiz::OxiZBackend;
use causm_analysis::solver::diagnostics::EntropicDiagnostic;
use causm_analysis::solver::facts::extract_facts;
use causm_analysis::solver::relational::RelationalInvariantSolver;
use causm_frontend::parser;

#[test]
fn test_relational_invariant_1_use_after_consume_relational() {
    let source = r#"
        @0ms: {
            let msg = "payload"
            yield msg
            let second = msg
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::UseAfterConsume { var, .. } if var == "msg")));
}

#[test]
fn test_relational_invariant_2_use_after_decay_relational() {
    let source = r#"
        @0ms: {
            let @decayed(10ms) item = 42
        }
        @50ms: {
            let read = item
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::TemporalUseAfterDecay { var, .. } if var == "item")));
}

#[test]
fn test_relational_invariant_3_field_decay_conflict_relational() {
    let source = r#"
        @0ms: {
            let s = struct { a = "A", b = "B" }
            yield s.a
            let y = s
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::CompoundStructFieldDecay { var, field, .. } if var == "s" && field == "a")));
}

#[test]
fn test_relational_invariant_4_lease_conflict_relational() {
    let source = r#"
        @0ms: {
            let state = 10
            lease view = state for 50ms {
                yield state
            }
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::LeaseConflict { source_var, .. } if source_var == "state")));
}

#[test]
fn test_relational_invariant_5_causal_paradox_relational() {
    let source = r#"
        @0ms: {
            anchor Checkpoint
            yield 1
            rewind_to Checkpoint
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::CausalParadox { anchor_name, .. } if anchor_name == "Checkpoint")));
}

#[test]
fn test_relational_invariant_6_entanglement_decay_relational() {
    let source = r#"
        @0ms: {
            let v1 = 10
            let v2 = 20
            entangle(v1, v2)
            yield v1
            let v3 = v2
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::EntanglementConflict { var, .. } if var == "v2")));
}

#[test]
fn test_relational_invariant_7_double_consume_conflict_relational() {
    let source = r#"
        @0ms: {
            let asset = 42
            yield asset
            yield asset
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::DoubleConsumeConflict { var, .. } if var == "asset")));
}

#[test]
fn test_relational_invariant_8_cross_branch_collision_relational() {
    let source = r#"
        @main: {
            let secret = 123
            split main into [left, right]
        }
        @left: {
            yield secret
        }
        @right: {
            let peek = secret + 1
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::CrossBranchCollision { var, .. } if var == "secret")));
}

#[test]
fn test_relational_invariant_9_speculative_leak_relational() {
    let source = r#"
        @main: {
            speculate (max 50ms) {
                let uncommitted = 999
            }
            let leaked = uncommitted
        }
    "#;
    let program = parser::parse_causm(source).unwrap();
    let facts = extract_facts(&program, source, "<test>");
    let analyzer = EntropicAnalyzer::new();
    let mut solver = RelationalInvariantSolver::<OxiZBackend>::new(&analyzer);
    let diags = solver.collect_diagnostics(&facts);
    assert!(diags.iter().any(|d| matches!(d, EntropicDiagnostic::SpeculativeLeak { var, .. } if var == "uncommitted")));
}
