use super::*;
use causm_analysis::analyzer::SemanticErrorKind;

#[test]
fn test_budget_while_loop_zero_max_budget_failure() {
    let source = r#"
        @0ms: {
            let mut x = 0
            while (x < 10) (max 0ms) {
                x = x + 1
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected failure on while loop with 0ms budget"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::InvalidLoopBudget => {}
        _ => panic!("Expected InvalidLoopBudget, got: {:?}", err.kind),
    }
}

#[test]
fn test_budget_tick_loop_without_slice_failure() {
    let source = r#"
        @0ms: {
            loop {
                break
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected failure on tick loop without slice declaration"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::TickLoopWithoutSlice => {}
        _ => panic!("Expected TickLoopWithoutSlice, got: {:?}", err.kind),
    }
}

#[test]
fn test_budget_tick_loop_budget_exceeded_failure() {
    let source = r#"
        @0ms: {
            slice 1ms
            loop {
                let a = 1
                let b = 2
                let c = 3
                break
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected failure when tick loop body exceeds slice"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::TickLoopBudgetExceeded(cost, slice) => {
            assert!(
                cost > slice,
                "Body cost {} should exceed slice {}",
                cost,
                slice
            );
        }
        _ => panic!("Expected TickLoopBudgetExceeded, got: {:?}", err.kind),
    }
}

#[test]
fn test_budget_tick_loop_missing_break_failure() {
    let source = r#"
        @0ms: {
            slice 50ms
            loop {
                let x = 1
            }
        }
    "#;
    let res = compile_program(source);
    assert!(res.is_err(), "Expected failure when tick loop has no break");
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::TickLoopNeedsBreak => {}
        _ => panic!("Expected TickLoopNeedsBreak, got: {:?}", err.kind),
    }
}
