use super::*;
use causm_analysis::analyzer::SemanticErrorKind;

#[test]
fn test_capability_missing_log_in_isolate_failure() {
    let source = r#"
        @0ms: {
            isolate sandbox {
                enable cpu(10)
                print("unauthorized print")
            }
        }
    "#;
    let res = compile_program(source);
    assert!(
        res.is_err(),
        "Expected compile failure for missing System.Log capability in isolate"
    );
    let err = res.unwrap_err();
    match *err.kind {
        SemanticErrorKind::MissingCapability(ref cap) => {
            assert_eq!(cap, "System.Log");
        }
        _ => panic!("Expected MissingCapability, got: {:?}", err.kind),
    }
}
