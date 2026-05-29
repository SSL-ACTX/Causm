use causm_core::{Expression, Program, SpannedStatement, Statement, TimeCoordinate};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reg(pub u32);

impl std::fmt::Display for Reg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrSelectCase {
    pub chan_id: String,
    pub dest: Reg,
    pub target: usize,
}

#[macro_export]
macro_rules! instructions {
    ($macro:ident) => {
        $macro! {
            // Arithmetic & Logic
            BinaryOp {
                dest: $crate::ir::Reg,
                op: causm_core::BinaryOperator,
                left: $crate::ir::Reg,
                right: $crate::ir::Reg
            },
            UnaryOp {
                dest: $crate::ir::Reg,
                op: causm_core::UnaryOperator,
                src: $crate::ir::Reg
            },

            // Data Movement
            LoadInt {
                dest: $crate::ir::Reg,
                value: i64
            },
            LoadFloat {
                dest: $crate::ir::Reg,
                value: u64
            },
            LoadBool {
                dest: $crate::ir::Reg,
                value: bool
            },
            LoadString {
                dest: $crate::ir::Reg,
                value: String
            },
            LoadNull {
                dest: $crate::ir::Reg
            },
            Move {
                dest: $crate::ir::Reg,
                src: $crate::ir::Reg
            },
            CMov {
                dest: $crate::ir::Reg,
                cond: $crate::ir::Reg,
                then_src: $crate::ir::Reg,
                else_src: $crate::ir::Reg
            },
            GetTsc {
                dest: $crate::ir::Reg
            },

            // Entropic Operations
            Consume {
                src: $crate::ir::Reg
            },
            ConsumeField {
                src: $crate::ir::Reg,
                field: String
            },
            ConsumeFieldDynamic {
                target: $crate::ir::Reg,
                index: $crate::ir::Reg
            },
            Clone {
                dest: $crate::ir::Reg,
                src: $crate::ir::Reg
            },

            // Control Flow
            Jump {
                target: usize
            },
            JumpIf {
                cond: $crate::ir::Reg,
                target: usize
            },
            JumpIfNot {
                cond: $crate::ir::Reg,
                target: usize
            },
            Call {
                routine: String,
                args: Vec<$crate::ir::Reg>,
                dest: $crate::ir::Reg
            },
            Return {
                src: Option<$crate::ir::Reg>
            },
            Break {
                target: usize
            },

            // Causm Temporal & Isolated Concurrency
            Isolate {
                name: String,
                manifest: causm_core::Manifest
            },
            EndIsolate,
            Lease {
                target_reg: $crate::ir::Reg,
                source_reg: $crate::ir::Reg,
                duration_ms: u64
            },
            EndLease {
                source_reg: $crate::ir::Reg,
                duration_ms: u64
            },
            Split {
                parent: String,
                branches: Vec<String>
            },
            Merge {
                branches: Vec<String>,
                target: String,
                resolution: causm_core::MergeResolution
            },
            Entangle {
                regs: Vec<$crate::ir::Reg>
            },
            Anchor {
                name: String
            },
            Rewind {
                target: String,
                anchor: String
            },
            Commit {
                vars: Vec<String>
            },
            Watchdog {
                target: String,
                timeout_ms: u64,
                recovery_jump: Option<usize>
            },
            Speculate {
                max_ms: u64,
                fallback_target: usize
            },
            Collapse,
            EndSpeculate {
                max_ms: u64,
                fallback_target: usize
            },
            SpeculationMode {
                mode: causm_core::SpeculationCommitMode
            },
            Select {
                max_ms: u64,
                cases: Vec<$crate::ir::IrSelectCase>,
                timeout_target: Option<usize>
            },
            MatchEntropy {
                target: $crate::ir::Reg,
                valid_target: Option<usize>,
                decayed_target: Option<usize>,
                pending_target: Option<usize>,
                consumed_target: Option<usize>
            },
            Await {
                target: $crate::ir::Reg
            },
            AwaitChan {
                target: String
            },
            Loop {
                max_ms: u64
            },
            EndLoop {
                max_ms: u64
            },
            Print {
                src: $crate::ir::Reg
            },
            Debug {
                src: $crate::ir::Reg
            },
            YieldPad,
            AssertTime {
                op: causm_core::BinaryOperator,
                limit_ms: u64
            },
            Slice {
                ms: u64
            },

            LoopTick,
            EndLoopTick,
            Capability {
                cap: causm_core::Capability
            },
            For {
                item_name: String,
                mode: causm_core::ForMode,
                source: $crate::ir::Reg,
                body: Vec<$crate::ir::Instruction>,
                pacing_ms: Option<u64>,
                max_ms: Option<u64>
            },
            SplitMap {
                item_name: String,
                mode: causm_core::ForMode,
                source: $crate::ir::Reg,
                body: Vec<$crate::ir::Instruction>,
                reconcile: Option<causm_core::MergeResolution>
            },
            Reset {
                target: String,
                anchor_name: String
            },
            FieldUpdate {
                target: $crate::ir::Reg,
                field: String,
                src: $crate::ir::Reg
            },
            IndexFieldUpdate {
                target: $crate::ir::Reg,
                index: $crate::ir::Reg,
                field: String,
                src: $crate::ir::Reg
            }
        }
    };
}

macro_rules! define_instruction_enum {
    ($($name:ident $({ $($field:ident: $type:ty),* })?),*) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Instruction {
            $($name $({ $($field: $type),* })?),*
        }
    };
}

instructions!(define_instruction_enum);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrRoutine {
    pub params: Vec<(causm_core::ParamMode, String, causm_core::types::Type)>,
    pub return_type: causm_core::types::Type,
    pub taking_ms: Option<u64>,
    pub taking_cycles: Option<u64>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrBlock {
    pub time: TimeCoordinate,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub blocks: Vec<IrBlock>,
    pub routines: HashMap<String, IrRoutine>,
    pub symbols: HashMap<String, Reg>,
    pub type_decay_limits: HashMap<String, u64>,
}

impl std::fmt::Display for IrProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, routine) in &self.routines {
            if let Some(ms) = routine.taking_ms {
                writeln!(f, "routine {} taking {}ms:", name, ms)?;
            } else if let Some(cycles) = routine.taking_cycles {
                writeln!(f, "routine {} taking {}cycles:", name, cycles)?;
            } else {
                writeln!(f, "routine {} taking _:", name)?;
            }
            for (i, instr) in routine.instructions.iter().enumerate() {
                writeln!(f, "  {:>3}: {:?}", i, instr)?;
            }
        }
        for block in &self.blocks {
            writeln!(f, "@{}:", block.time)?;
            for (i, instr) in block.instructions.iter().enumerate() {
                writeln!(f, "  {:>3}: {:?}", i, instr)?;
            }
        }
        Ok(())
    }
}

struct LoweringContext {
    next_reg: u32,
    symbols: HashMap<String, Reg>,
    instructions: Vec<Instruction>,
    routines: HashMap<String, IrRoutine>,
    type_decay_limits: HashMap<String, u64>,
    loop_stack: Vec<Vec<usize>>, // Indices of Break instructions to be resolved
}

impl LoweringContext {
    fn new() -> Self {
        Self {
            next_reg: 0,
            symbols: HashMap::new(),
            instructions: Vec::new(),
            routines: HashMap::new(),
            type_decay_limits: HashMap::new(),
            loop_stack: Vec::new(),
        }
    }

    fn alloc_reg(&mut self) -> Reg {
        let r = Reg(self.next_reg);
        self.next_reg += 1;
        r
    }

    fn get_reg(&mut self, name: &str) -> Reg {
        if let Some(r) = self.symbols.get(name) {
            *r
        } else {
            let r = self.alloc_reg();
            self.symbols.insert(name.to_string(), r);
            r
        }
    }

    fn push(&mut self, instr: Instruction) {
        self.instructions.push(instr);
    }
}

pub fn lower_program(program: &Program) -> IrProgram {
    let mut blocks = Vec::new();
    let mut ctx = LoweringContext::new();

    for tb in &program.timelines {
        let start_idx = ctx.instructions.len();
        for stmt in &tb.statements {
            lower_statement(&mut ctx, &stmt.stmt);
        }
        let block_instrs = ctx.instructions.split_off(start_idx);
        blocks.push(IrBlock {
            time: tb.time.clone(),
            instructions: block_instrs,
        });
    }

    IrProgram {
        blocks,
        routines: ctx.routines,
        symbols: ctx.symbols,
        type_decay_limits: ctx.type_decay_limits,
    }
}

fn is_simple_expression(expr: &Expression) -> bool {
    match expr {
        Expression::Integer(_)
        | Expression::Float(_)
        | Expression::Boolean(_)
        | Expression::Literal(_)
        | Expression::Null
        | Expression::Identifier(_) => true,
        Expression::BinaryOp { left, op: _, right } => {
            is_simple_expression(left) && is_simple_expression(right)
        }
        Expression::UnaryOp { op: _, expr } => is_simple_expression(expr),
        Expression::FieldAccess { target, field: _ } => is_simple_expression(target),
        _ => false,
    }
}

#[allow(dead_code)]
fn is_simple_statement(stmt: &Statement) -> bool {
    match stmt {
        Statement::Assignment { expr, .. } => is_simple_expression(expr),
        Statement::Expression(expr) => is_simple_expression(expr),
        Statement::FieldUpdate {
            target,
            field: _,
            value,
        } => is_simple_expression(target) && is_simple_expression(value),
        _ => false,
    }
}

#[allow(dead_code)]
fn is_simple_block(block: &[SpannedStatement]) -> bool {
    block.iter().all(|s| is_simple_statement(&s.stmt))
}

fn lower_statement(ctx: &mut LoweringContext, stmt: &Statement) {
    match stmt {
        Statement::RoutineDef {
            name,
            params,
            return_type,
            taking_ms,
            taking_cycles,
            body,
        } => {
            let mut sub_ctx = LoweringContext::new();
            for p in params {
                sub_ctx
                    .symbols
                    .insert(p.name.clone(), Reg(sub_ctx.next_reg));
                sub_ctx.next_reg += 1;
            }
            for s in body {
                lower_statement(&mut sub_ctx, &s.stmt);
            }
            let routine = IrRoutine {
                params: params
                    .iter()
                    .map(|p| {
                        (
                            p.mode.clone(),
                            p.name.clone(),
                            p.typ
                                .as_ref()
                                .map(causm_core::types::Type::from_typename)
                                .unwrap_or(causm_core::types::Type::Unknown),
                        )
                    })
                    .collect(),
                return_type: return_type
                    .as_ref()
                    .map(causm_core::types::Type::from_typename)
                    .unwrap_or(causm_core::types::Type::Unknown),
                taking_ms: *taking_ms,
                taking_cycles: *taking_cycles,
                instructions: sub_ctx.instructions,
            };
            ctx.routines.insert(name.clone(), routine);
        }
        Statement::YieldPad => {
            ctx.push(Instruction::YieldPad);
        }
        Statement::Yield(name) => {
            let src = ctx.get_reg(name);
            // By convention, Move src to R0 for return
            ctx.push(Instruction::Move { dest: Reg(0), src });
        }
        Statement::Await(target) => {
            let reg = ctx.get_reg(target);
            ctx.push(Instruction::Await { target: reg });
        }
        Statement::AwaitChan(target) => {
            ctx.push(Instruction::AwaitChan {
                target: target.clone(),
            });
        }
        Statement::Slice { milliseconds } => {
            ctx.push(Instruction::Slice { ms: *milliseconds });
        }
        Statement::Break => {
            let idx = ctx.instructions.len();
            ctx.push(Instruction::Break { target: 0 }); // Placeholder
            if let Some(breaks) = ctx.loop_stack.last_mut() {
                breaks.push(idx);
            }
        }
        Statement::LoopTick { body } => {
            ctx.loop_stack.push(Vec::new());
            ctx.push(Instruction::LoopTick);
            for s in body {
                lower_statement(ctx, &s.stmt);
            }
            ctx.push(Instruction::EndLoopTick);
            let breaks = ctx.loop_stack.pop().unwrap_or_default();
            let end_idx = ctx.instructions.len();
            for b_idx in breaks {
                if let Instruction::Break { ref mut target } =
                    ctx.instructions[b_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::Lease {
            binding,
            source,
            duration_ms,
            body,
        } => {
            let source_reg = ctx.get_reg(source);
            let target_reg = ctx.get_reg(binding);

            ctx.push(Instruction::Lease {
                target_reg,
                source_reg,
                duration_ms: *duration_ms,
            });

            for s in body {
                lower_statement(ctx, &s.stmt);
            }

            ctx.push(Instruction::EndLease {
                source_reg,
                duration_ms: *duration_ms,
            });
        }
        Statement::Loop { max_ms, body } => {
            ctx.loop_stack.push(Vec::new());
            let start_pc = ctx.instructions.len();
            ctx.push(Instruction::Loop { max_ms: *max_ms });
            for s in body {
                lower_statement(ctx, &s.stmt);
            }
            ctx.push(Instruction::EndLoop { max_ms: *max_ms });
            ctx.push(Instruction::Jump { target: start_pc });

            let breaks = ctx.loop_stack.pop().unwrap_or_default();
            let end_idx = ctx.instructions.len();
            for b_idx in breaks {
                if let Instruction::Break { ref mut target } =
                    ctx.instructions[b_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            // SpeedMicro: Constant-Time Lowering (Branchless)
            // If both branches are single simple assignments to the same variable, use CMov.
            if let Some(eb) = else_branch {
                if then_branch.len() == 1 && eb.len() == 1 {
                    if let (
                        Statement::Assignment {
                            target: t1,
                            expr: e1,
                            ..
                        },
                        Statement::Assignment {
                            target: t2,
                            expr: e2,
                            ..
                        },
                    ) = (&then_branch[0].stmt, &eb[0].stmt)
                    {
                        if t1 == t2
                            && is_simple_expression(e1)
                            && is_simple_expression(e2)
                        {
                            let cond_reg = lower_expression(ctx, condition);
                            let t_reg = lower_expression(ctx, e1);
                            let e_reg = lower_expression(ctx, e2);
                            let dest = ctx.get_reg(t1);
                            ctx.push(Instruction::CMov {
                                dest,
                                cond: cond_reg,
                                then_src: t_reg,
                                else_src: e_reg,
                            });
                            return;
                        }
                    }
                }
            }

            let cond_reg = lower_expression(ctx, condition);

            let jump_to_else_idx = ctx.instructions.len();
            ctx.push(Instruction::JumpIfNot {
                cond: cond_reg,
                target: 0,
            }); // Placeholder

            for s in then_branch {
                lower_statement(ctx, &s.stmt);
            }

            if let Some(eb) = else_branch {
                let jump_to_end_idx = ctx.instructions.len();
                ctx.push(Instruction::Jump { target: 0 }); // Placeholder

                let else_start_idx = ctx.instructions.len();
                if let Instruction::JumpIfNot { ref mut target, .. } =
                    ctx.instructions[jump_to_else_idx]
                {
                    *target = else_start_idx;
                }

                for s in eb {
                    lower_statement(ctx, &s.stmt);
                }

                let end_idx = ctx.instructions.len();
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_to_end_idx]
                {
                    *target = end_idx;
                }
            } else {
                let end_idx = ctx.instructions.len();
                if let Instruction::JumpIfNot { ref mut target, .. } =
                    ctx.instructions[jump_to_else_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::Watchdog {
            target,
            timeout_ms,
            recovery,
        } => {
            let recovery_start = ctx.instructions.len() + 1; // Approx
            let _ = recovery_start; // Placeholder
            ctx.push(Instruction::Watchdog {
                target: target.clone(),
                timeout_ms: *timeout_ms,
                recovery_jump: None,
            });
            for s in recovery {
                lower_statement(ctx, &s.stmt);
            }
        }
        Statement::Speculate {
            max_ms,
            body,
            fallback,
        } => {
            let spec_idx = ctx.instructions.len();
            ctx.push(Instruction::Speculate {
                max_ms: *max_ms,
                fallback_target: 0,
            });

            for s in body {
                lower_statement(ctx, &s.stmt);
            }

            ctx.push(Instruction::EndSpeculate {
                max_ms: *max_ms,
                fallback_target: 0, // Not strictly used by EndSpeculate but matches signature
            });

            if let Some(fb) = fallback {
                let end_idx_placeholder = ctx.instructions.len();
                ctx.push(Instruction::Jump { target: 0 });

                let fb_start = ctx.instructions.len();
                if let Instruction::Speculate {
                    ref mut fallback_target,
                    ..
                } = ctx.instructions[spec_idx]
                {
                    *fallback_target = fb_start;
                }

                for s in fb {
                    lower_statement(ctx, &s.stmt);
                }

                let end_idx = ctx.instructions.len();
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[end_idx_placeholder]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::Collapse => {
            ctx.push(Instruction::Collapse);
        }
        Statement::SpeculationMode(mode) => {
            ctx.push(Instruction::SpeculationMode { mode: *mode });
        }
        Statement::Select {
            max_ms,
            cases,
            timeout,
            ..
        } => {
            let select_idx = ctx.instructions.len();
            ctx.push(Instruction::Select {
                max_ms: *max_ms,
                cases: Vec::new(),
                timeout_target: None,
            });

            let mut branch_jumps = Vec::new();
            for case in cases {
                let dest = ctx.get_reg(&case.binding);
                let start = ctx.instructions.len();

                let chan_id = match &case.source {
                    Expression::Identifier(id) => id.clone(),
                    _ => "unknown".to_string(),
                };

                if let Instruction::Select { ref mut cases, .. } =
                    ctx.instructions[select_idx]
                {
                    cases.push(IrSelectCase {
                        chan_id,
                        dest,
                        target: start,
                    });
                }

                for s in &case.body {
                    lower_statement(ctx, &s.stmt);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            if let Some(fb) = timeout {
                let start = ctx.instructions.len();
                if let Instruction::Select {
                    ref mut timeout_target,
                    ..
                } = ctx.instructions[select_idx]
                {
                    *timeout_target = Some(start);
                }
                for s in fb {
                    lower_statement(ctx, &s.stmt);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            let end_idx = ctx.instructions.len();
            for jump_idx in branch_jumps {
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::MatchEntropy {
            target,
            valid_branch,
            decayed_branch,
            pending_branch,
            consumed_branch,
        } => {
            let target_reg = lower_expression(ctx, target);
            let match_idx = ctx.instructions.len();
            ctx.push(Instruction::MatchEntropy {
                target: target_reg,
                valid_target: None,
                decayed_target: None,
                pending_target: None,
                consumed_target: None,
            });

            let mut branch_jumps = Vec::new();

            if let Some((binding, body)) = valid_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut valid_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *valid_target = Some(start);
                }
                let dest = ctx.get_reg(binding);
                ctx.push(Instruction::Move {
                    dest,
                    src: target_reg,
                });
                for s in body {
                    lower_statement(ctx, &s.stmt);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            if let Some((binding, body)) = decayed_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut decayed_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *decayed_target = Some(start);
                }
                let dest = ctx.get_reg(binding);
                ctx.push(Instruction::Move {
                    dest,
                    src: target_reg,
                });
                for s in body {
                    lower_statement(ctx, &s.stmt);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            if let Some(body) = pending_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut pending_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *pending_target = Some(start);
                }
                for s in body {
                    lower_statement(ctx, &s.stmt);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            if let Some(body) = consumed_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut consumed_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *consumed_target = Some(start);
                }
                for s in body {
                    lower_statement(ctx, &s.stmt);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            let end_idx = ctx.instructions.len();
            for jump_idx in branch_jumps {
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::Assignment { target, expr, .. } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.get_reg(target);
            ctx.push(Instruction::Move { dest, src });

            // Consuming move by default in Causm
            match expr {
                Expression::Identifier(_) => {
                    ctx.push(Instruction::Consume { src });
                }
                Expression::IndexAccess {
                    target: inner_target,
                    index,
                } => {
                    let graph_reg = lower_expression(ctx, inner_target);
                    let index_reg = lower_expression(ctx, index);
                    // We need to know the field name/index at runtime.
                    // For now, let's assume it's a string index.
                    // Instruction::ConsumeIndex { target, index }
                    ctx.push(Instruction::ConsumeFieldDynamic {
                        target: graph_reg,
                        index: index_reg,
                    });
                }
                _ => {}
            }
        }
        Statement::Print(expr) => {
            let src = lower_expression(ctx, expr);
            ctx.push(Instruction::Print { src });
        }
        Statement::Debug(expr) => {
            let src = lower_expression(ctx, expr);
            ctx.push(Instruction::Debug { src });
        }
        Statement::Isolate(block) => {
            let name = block.name.clone().unwrap_or_else(|| "<anon>".to_string());
            ctx.push(Instruction::Isolate {
                name,
                manifest: block.manifest.clone(),
            });
            for s in &block.body {
                lower_statement(ctx, &s.stmt);
            }
            ctx.push(Instruction::EndIsolate);
        }
        Statement::Capability(cap) => {
            ctx.push(Instruction::Capability { cap: cap.clone() });
        }
        Statement::For {
            item_name,
            mode,
            source,
            body,
            pacing_ms,
            max_ms,
        } => {
            let source_reg = ctx.get_reg(source);
            let mut sub_ctx = LoweringContext::new();
            sub_ctx.symbols.insert(item_name.clone(), Reg(0));
            sub_ctx.next_reg = 1;
            for s in body {
                lower_statement(&mut sub_ctx, &s.stmt);
            }
            ctx.push(Instruction::For {
                item_name: item_name.clone(),
                mode: mode.clone(),
                source: source_reg,
                body: sub_ctx.instructions,
                pacing_ms: *pacing_ms,
                max_ms: *max_ms,
            });
        }
        Statement::SplitMap {
            item_name,
            mode,
            source,
            body,
            reconcile,
            ..
        } => {
            let source_reg = ctx.get_reg(source);
            let mut sub_ctx = LoweringContext::new();
            sub_ctx.symbols.insert(item_name.clone(), Reg(0));
            sub_ctx.next_reg = 1;
            for s in body {
                lower_statement(&mut sub_ctx, &s.stmt);
            }
            ctx.push(Instruction::SplitMap {
                item_name: item_name.clone(),
                mode: mode.clone(),
                source: source_reg,
                body: sub_ctx.instructions,
                reconcile: reconcile.clone(),
            });
        }
        Statement::Split { parent, branches } => {
            ctx.push(Instruction::Split {
                parent: parent.clone(),
                branches: branches.clone(),
            });
        }
        Statement::Merge {
            branches,
            target,
            resolutions,
        } => {
            ctx.push(Instruction::Merge {
                branches: branches.clone(),
                target: target.clone(),
                resolution: resolutions.clone(),
            });
        }
        Statement::Anchor(name) => {
            ctx.push(Instruction::Anchor { name: name.clone() });
        }
        Statement::Rewind(target) => {
            ctx.push(Instruction::Rewind {
                target: "main".to_string(),
                anchor: target.clone(),
            });
        }
        Statement::Reset {
            target,
            anchor_name,
        } => {
            ctx.push(Instruction::Reset {
                target: target.clone(),
                anchor_name: anchor_name.clone(),
            });
        }
        Statement::Commit(body) => {
            let mut vars = Vec::new();
            for s in body {
                if let Statement::Assignment { target, .. } = &s.stmt {
                    vars.push(target.clone());
                }
                lower_statement(ctx, &s.stmt);
            }
            ctx.push(Instruction::Commit { vars });
        }
        Statement::Entangle { variables } => {
            let regs = variables.iter().map(|v| ctx.get_reg(v)).collect();
            ctx.push(Instruction::Entangle { regs });
        }
        Statement::FieldUpdate {
            target,
            field,
            value,
        } => match target {
            Expression::IndexAccess {
                target: inner_target,
                index,
            } => {
                let graph_reg = lower_expression(ctx, inner_target);
                let index_reg = lower_expression(ctx, index);
                let src_reg = lower_expression(ctx, value);
                ctx.push(Instruction::IndexFieldUpdate {
                    target: graph_reg,
                    index: index_reg,
                    field: field.clone(),
                    src: src_reg,
                });
            }
            _ => {
                let target_reg = lower_expression(ctx, target);
                let src_reg = lower_expression(ctx, value);
                ctx.push(Instruction::FieldUpdate {
                    target: target_reg,
                    field: field.clone(),
                    src: src_reg,
                });
            }
        },
        Statement::Expression(expr) => {
            lower_expression(ctx, expr);
        }
        Statement::AssertTime {
            operator, limit_ms, ..
        } => {
            ctx.push(Instruction::AssertTime {
                op: *operator,
                limit_ms: *limit_ms,
            });
        }
        Statement::TypeDecl {
            name,
            decay_after_ms: Some(limit),
            ..
        } => {
            ctx.type_decay_limits.insert(name.clone(), *limit);
        }
        Statement::TypeDecl { .. } => {}
        _ => {
            // Other statements can be added as needed
        }
    }
}

fn lower_expression(ctx: &mut LoweringContext, expr: &Expression) -> Reg {
    match expr {
        Expression::Integer(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::LoadInt { dest, value: *v });
            dest
        }
        Expression::Float(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::LoadFloat { dest, value: *v });
            dest
        }
        Expression::Boolean(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::LoadBool { dest, value: *v });
            dest
        }
        Expression::Literal(s) => {
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::LoadString {
                dest,
                value: s.clone(),
            });
            dest
        }
        Expression::Null => {
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::LoadNull { dest });
            dest
        }
        Expression::Identifier(name) => ctx.get_reg(name),
        Expression::BinaryOp { left, op, right } => {
            let l = lower_expression(ctx, left);
            let r = lower_expression(ctx, right);
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::BinaryOp {
                dest,
                op: *op,
                left: l,
                right: r,
            });
            dest
        }
        Expression::UnaryOp { op, expr } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::UnaryOp { dest, op: *op, src });
            dest
        }

        Expression::Call { routine, args } => {
            let mut arg_regs = Vec::new();
            for arg in args {
                arg_regs.push(lower_expression(ctx, arg));
            }
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::Call {
                routine: routine.clone(),
                args: arg_regs,
                dest,
            });
            dest
        }

        Expression::FieldAccess { target, field } => {
            let src = lower_expression(ctx, target);
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::ConsumeField {
                src,
                field: field.clone(),
            });
            // We reuse next_reg for the result
            dest
        }
        Expression::CloneOp(name) => {
            let src = ctx.get_reg(name);
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::Clone { dest, src });
            dest
        }
        Expression::IndexAccess { target, index } => {
            let t = lower_expression(ctx, target);
            let i = lower_expression(ctx, index);
            let dest = ctx.alloc_reg();
            ctx.push(Instruction::ConsumeFieldDynamic {
                target: t,
                index: i,
            });
            dest
        }
        _ => Reg(0), // TODO: other expressions
    }
}
