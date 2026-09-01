use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::{Program, TimeCoordinate};

/// Stage 2 of the analysis pipeline: SSA Construction & Control Flow Graph.
///
/// Iterates every timeline block in the program, advances the virtual clock,
/// and runs `analyze_statement` on each statement in order. This corresponds
/// to the live-range / entropic state tracking pass that assigns SSA versions
/// to registers and checks consume/peek/lease invariants.
///
/// The formal SSA IR (basic blocks, phi nodes, dominance frontiers) is
/// constructed by `causm-ir`; this stage performs semantic validation in
/// parallel on the same source structure.
pub struct SsaStage;

impl SsaStage {
    pub fn run(
        analyzer: &mut EntropicAnalyzer,
        program: &Program,
    ) -> Result<(), SemanticError> {
        for block in &program.timelines {
            let old_branch = analyzer.current_branch.clone();
            let old_entropy_mode = analyzer.entropy_mode;
            if let Some(mode) = block.entropy_mode {
                analyzer.entropy_mode = mode;
            }

            match &block.time {
                TimeCoordinate::Branch(id) => {
                    if id != "main" && !analyzer.branch_contexts.contains_key(id) {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::InactiveTimeline(id.clone()),
                        ));
                    }
                    if analyzer.merged_branches.contains(id) {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::InactiveTimeline(id.clone()),
                        ));
                    }
                    analyzer.current_branch = id.clone();
                }
                TimeCoordinate::Global(t) => {
                    let state = analyzer
                        .branch_contexts
                        .get_mut(&analyzer.current_branch)
                        .unwrap();
                    if *t > state.accumulated_cost {
                        state.accumulated_cost = *t;
                    }
                }
                TimeCoordinate::Relative(t) => {
                    let state = analyzer
                        .branch_contexts
                        .get_mut(&analyzer.current_branch)
                        .unwrap();
                    state.accumulated_cost += *t;
                }
                TimeCoordinate::Periodic(interval_ms) => {
                    let block_cost = crate::statement::estimate_block_cost(
                        analyzer,
                        &block.statements,
                    );
                    if block_cost > *interval_ms {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::PeriodicDeadlineUnachievable(
                                block_cost,
                                *interval_ms,
                            ),
                        ));
                    }
                    let state = analyzer
                        .branch_contexts
                        .get_mut(&analyzer.current_branch)
                        .unwrap();
                    state.accumulated_cost += *interval_ms;
                }
            }

            for stmt in &block.statements {
                let old_stmt = analyzer.current_statement.clone();
                let old_span = analyzer.current_span.clone();
                analyzer.current_statement =
                    Some(analyzer.statement_snippet(stmt));
                analyzer.current_span = Some(stmt.span.clone());
                analyzer.analyze_statement(stmt)?;
                analyzer.current_statement = old_stmt;
                analyzer.current_span = old_span;
            }

            if matches!(&block.time, TimeCoordinate::Periodic(_)) {
                let state = analyzer
                    .branch_contexts
                    .get_mut(&analyzer.current_branch)
                    .unwrap();
                state.consumed.clear();
            }

            analyzer.current_branch = old_branch;
            analyzer.entropy_mode = old_entropy_mode;
        }

        Ok(())
    }
}
