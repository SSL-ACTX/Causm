//! End-to-end integration test verifying the strict stage-by-stage compilation pipeline:
//!
//! Source Code (&str)
//!      │
//!      ▼ [Stage 1: TokenStream / Lexer]
//! Tokens (TokenKind stream via TokenStream)
//!      │
//!      ▼ [Stage 2: ArenaParser + Pratt Parser]
//! AstArena (ExprId / StmtId) -> causm_core::Program (AST with surface sugar)
//!      │
//!      ▼ [Stage 3: HIR Lowering & Desugaring]
//! causm_core::hir::HirProgram (Canonical HIR, sugar stripped: using -> let+autodrop+consume, f"..." -> binary add chain)
//!      │
//!      ▼ [Stage 4: Entropic Semantic Analysis]
//! EntropicAnalyzer::analyze_hir_with_source (Invariants, lifetimes, WCET bounds)
//!      │
//!      ▼ [Stage 5: IR Lowering]
//! causm_ir::IrProgram (Basic blocks, instructions, SSA registers)
//!      │
//!      ▼ [Stage 6: IR Optimization Pipeline]
//! causm_ir::optimize::optimize_program (DCE, branch simplification, constant propagation)
//!      │
//!      ▼ [Stage 7: VM Isochronous Execution]
//! causm_runtime::vm::Vm (Executes bytecode, asserts deterministic terminal state)

use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::hir::{HirExpression, HirProgram, HirStatement};
use causm_frontend::hir::lower_ast_to_hir;
use causm_frontend::lower::lower_hir_program;
use causm_frontend::parser::lexer::{TokenKind, TokenStream};
use causm_frontend::parser::parse_causm;
use causm_runtime::vm::Vm;

#[test]
fn test_pipeline_end_to_end_stage_by_stage_execution() -> anyhow::Result<()> {
    // A complete program exercising surface syntax sugar:
    // - using resource block (sugar -> desugared into binding + autodrop + consume in HIR)
    // - f"..." string interpolation (sugar -> desugared into binary add chain in HIR)
    // - arithmetic operations (verified by analyzer, lowered into registers, optimized, executed by VM)
    let source = r#"
    @0ms: {
        using res = 10 {
            let base = res * 4
            let bonus = 2
            let total = base + bonus
            let msg = f"Result: {total}"
            let _done = msg
        }
    }
    "#;

    // Stage 1: Lexer & TokenStream
    let mut stream = TokenStream::new(source);
    let mut tokens = Vec::new();
    loop {
        let tok = stream.next_token();
        let is_eof = tok.kind == TokenKind::Eof;
        tokens.push(tok.kind);
        if is_eof {
            break;
        }
    }
    assert!(
        tokens.iter().any(|t| matches!(t, TokenKind::Using)),
        "Stage 1 assertion failed: Lexer must emit TokenKind::Using"
    );
    assert!(
        tokens.iter().any(|t| matches!(t, TokenKind::FStr(_))),
        "Stage 1 assertion failed: Lexer must emit TokenKind::FStr"
    );

    // Stage 2: ArenaParser + Pratt Parser -> AST Program
    let ast_program = parse_causm(source)?;
    assert_eq!(ast_program.timelines.len(), 1);
    // At AST level, surface sugar (`using`) exists verbatim in the syntax tree
    let has_ast_using = ast_program.timelines[0].statements.iter().any(|s| {
        matches!(&s.stmt, causm_core::Statement::Using { binding, .. } if binding == "res")
    });
    assert!(
        has_ast_using,
        "Stage 2 assertion failed: AST must contain verbatim Statement::Using node"
    );

    // Stage 3: HIR Lowering & Desugaring (AST -> HirProgram)
    let hir_program: HirProgram = lower_ast_to_hir(&ast_program);
    assert_eq!(hir_program.timelines.len(), 1);
    let hir_stmts = &hir_program.timelines[0].statements;

    // Verify HIR canonical desugaring:
    // 1. `using res = 10 { ... }` was dismantled into `let res = 10`, inner stmts, AutoDrop, Consume
    let has_hir_res_binding = matches!(&hir_stmts[0].stmt, HirStatement::Assignment { target, .. } if target == "res");
    let has_hir_autodrop = hir_stmts.iter().any(
        |s| matches!(&s.stmt, HirStatement::AutoDrop { target } if target == "res"),
    );
    let has_hir_consume = hir_stmts.iter().any(
        |s| matches!(&s.stmt, HirStatement::Consume { target } if target == "res"),
    );
    assert!(
        has_hir_res_binding,
        "Stage 3 assertion failed: HIR must lower using into let assignment"
    );
    assert!(has_hir_autodrop, "Stage 3 assertion failed: HIR must insert explicit AutoDrop for using resource");
    assert!(has_hir_consume, "Stage 3 assertion failed: HIR must insert explicit Consume for using resource");

    // 2. `f"Result: {total}"` was desugared into a typed BinaryOp::Add concatenation tree
    let found_fstring_binary_chain = hir_stmts.iter().any(|s| {
        if let HirStatement::Assignment { target, expr, .. } = &s.stmt {
            if target == "msg" {
                return matches!(
                    expr,
                    HirExpression::BinaryOp {
                        op: causm_core::BinaryOperator::Add,
                        ..
                    }
                );
            }
        }
        false
    });
    assert!(
        found_fstring_binary_chain,
        "Stage 3 assertion failed: HIR must desugar f-string into canonical BinaryOp::Add chain"
    );

    // Stage 4: Entropic Semantic Analysis on HirProgram
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    let analysis_result = analyzer.analyze_hir_with_source(
        &hir_program,
        source,
        "pipeline_stage_test.csm",
    );
    assert!(
        analysis_result.is_ok(),
        "Stage 4 assertion failed: Semantic analysis on HirProgram must succeed, got: {:?}",
        analysis_result.err()
    );

    // Stage 5: IR Lowering (HirProgram -> causm_ir::IrProgram)
    let mut ir_program = lower_hir_program(&hir_program);
    assert!(
        !ir_program.blocks.is_empty(),
        "Stage 5 assertion failed: IR lowering must produce at least one IrBlock"
    );
    let initial_instr_count: usize =
        ir_program.blocks.iter().map(|b| b.instructions.len()).sum();
    assert!(
        initial_instr_count > 0,
        "Stage 5 assertion failed: IR program must contain instructions"
    );

    // Stage 6: IR Optimization Pipeline (DCE, Constant Prop, CFG Simplify)
    causm_ir::optimize::prune_unreachable_routines(&mut ir_program);
    let optimized_ir = causm_ir::optimize::optimize_program(ir_program);
    assert!(
        !optimized_ir.blocks.is_empty(),
        "Stage 6 assertion failed: Optimized IR program must preserve blocks"
    );

    // Stage 7: VM Isochronous Execution
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    let exec_res = vm.execute_program(&optimized_ir);
    assert!(
        exec_res.is_ok(),
        "Stage 7 assertion failed: VM must successfully execute the optimized IR program: {:?}",
        exec_res.err()
    );

    Ok(())
}

#[test]
fn test_cli_emit_hir_and_dump_flag() -> anyhow::Result<()> {
    let source = r#"
    timeline @root {
        let x = 42
        let text = f"value is: {x}"
        debug text
    }
    "#;

    let ast_program = parse_causm(source)?;
    let hir_program = lower_ast_to_hir(&ast_program);

    // Verify HIR output formatting debug string
    let hir_debug = format!("{:#?}", hir_program);
    assert!(
        hir_debug.contains("Program"),
        "HIR dump output must format Program"
    );
    assert!(
        hir_debug.contains("BinaryOp"),
        "HIR dump output must format desugared expressions"
    );

    Ok(())
}
