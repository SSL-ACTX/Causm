use causm_analysis::analyzer::{EntropicAnalyzer, SemanticErrorKind};
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn causm_oop_basic_method_call() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.get_score(peek self) -> int (taking 5ms) {
            let s = self.score
            yield s
        }

        let p: Player = struct { score = 42 }
        let s = p.get_score()
        let _s = s
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_oop_method_consume() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.retire(consume self) (taking 3ms) {
            let _self = self
        }

        let p: Player = struct { score = 42 }
        let _r = p.retire()
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::UseAfterConsume(name) => assert_eq!(name, "p"),
        _ => panic!("Expected UseAfterConsume, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_oop_method_type_mismatch() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.get_score(peek self) -> int (taking 5ms) {
            let s = self.score
            yield s
        }

        let p: Player = struct { score = 42 }
        let s = p.get_score(10)
        let _s = s
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::EntropyMismatch(_) => {}
        _ => panic!(
            "Expected EntropyMismatch/Argument count mismatch, got {:?}",
            err.kind
        ),
    }

    Ok(())
}

#[test]
fn causm_oop_constructor_call() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Player = struct { score: int }

        routine Player.new(clone score: int) -> Player (taking 4ms) {
            let p: Player = struct { score = score }
            yield p
        }

        let p: Player = call Player.new(42)
        let _p = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}
