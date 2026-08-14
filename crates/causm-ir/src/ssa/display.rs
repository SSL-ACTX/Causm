use super::types::*;
use crate::cfg::BlockId;
use std::collections::HashMap;

impl std::fmt::Display for SsaInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsaInstruction::BinaryOp {
                dest,
                op,
                left,
                right,
            } => {
                write!(f, "{} = {} {:?} {}", dest, left, op, right)
            }
            SsaInstruction::UnaryOp { dest, op, src } => {
                write!(f, "{} = {:?} {}", dest, op, src)
            }
            SsaInstruction::LoadInt { dest, value }
            | SsaInstruction::ConstInt { dest, value } => {
                write!(f, "{} = {}", dest, value)
            }
            SsaInstruction::LoadFloat { dest, value }
            | SsaInstruction::ConstFloat { dest, value } => {
                write!(f, "{} = {}", dest, value)
            }
            SsaInstruction::LoadBool { dest, value }
            | SsaInstruction::ConstBool { dest, value } => {
                write!(f, "{} = {}", dest, value)
            }
            SsaInstruction::LoadString { dest, value }
            | SsaInstruction::ConstString { dest, value } => {
                write!(f, "{} = {:?}", dest, value)
            }
            SsaInstruction::LoadNull { dest }
            | SsaInstruction::ConstNull { dest } => {
                write!(f, "{} = null", dest)
            }
            SsaInstruction::Move { dest, src } => {
                write!(f, "{} = {}", dest, src)
            }
            SsaInstruction::Consume { src } => {
                write!(f, "Consume {}", src)
            }
            SsaInstruction::ConsumeField { src, field } => {
                write!(f, "ConsumeField {}.{}", src, field)
            }
            SsaInstruction::ConsumeFieldDynamic { target, index } => {
                write!(f, "ConsumeFieldDynamic {}[{}]", target, index)
            }
            SsaInstruction::Clone { dest, src } => {
                write!(f, "{} = Clone {}", dest, src)
            }
            SsaInstruction::Call {
                routine,
                args,
                dest,
            } => {
                let args_str: Vec<String> =
                    args.iter().map(|a| a.to_string()).collect();
                write!(f, "{} = Call {}({})", dest, routine, args_str.join(", "))
            }
            SsaInstruction::DynamicCall {
                method,
                args,
                dest,
                budget,
            } => {
                let args_str: Vec<String> =
                    args.iter().map(|a| a.to_string()).collect();
                let budget_str = budget
                    .map(|b| format!(" [budget: {}ms]", b))
                    .unwrap_or_default();
                write!(
                    f,
                    "{} = DynamicCall {}({}){}",
                    dest,
                    method,
                    args_str.join(", "),
                    budget_str
                )
            }
            SsaInstruction::TypeAssert {
                dest,
                src,
                type_name,
            } => {
                write!(f, "{} = {} as {}", dest, src, type_name)
            }
            SsaInstruction::TypeCast {
                dest,
                src,
                target_type,
            } => {
                write!(f, "{} = {} as {:?}", dest, src, target_type)
            }
            SsaInstruction::AssertState { src, state } => {
                write!(f, "AssertState {} is {}", src, state)
            }
            SsaInstruction::TryTypeAssert {
                dest,
                src,
                type_name,
                success,
            } => {
                write!(
                    f,
                    "{}, {} = TryTypeAssert {} as {}",
                    dest, success, src, type_name
                )
            }
            SsaInstruction::Print { src } => {
                write!(f, "Print {}", src)
            }
            SsaInstruction::Debug { src } => {
                write!(f, "Debug {}", src)
            }
            SsaInstruction::Slice { ms } => {
                write!(f, "Slice {}ms", ms)
            }
            SsaInstruction::Isolate { name, manifest } => {
                write!(f, "Isolate {} {:?}", name, manifest)
            }
            SsaInstruction::EndIsolate => {
                write!(f, "EndIsolate")
            }
            SsaInstruction::Lease {
                target_reg,
                source_reg,
                duration_ms,
            } => {
                write!(
                    f,
                    "{} = Lease {} for {}ms",
                    target_reg, source_reg, duration_ms
                )
            }
            SsaInstruction::EndLease {
                source_reg,
                duration_ms,
            } => {
                write!(f, "EndLease {} for {}ms", source_reg, duration_ms)
            }
            SsaInstruction::Split { parent, branches } => {
                write!(f, "Split {} into {:?}", parent, branches)
            }
            SsaInstruction::Merge {
                branches,
                target,
                resolution,
            } => {
                write!(f, "Merge {:?} into {} {:?}", branches, target, resolution)
            }
            SsaInstruction::Entangle { regs } => {
                let regs_str: Vec<String> =
                    regs.iter().map(|r| r.to_string()).collect();
                write!(f, "Entangle [{}]", regs_str.join(", "))
            }
            SsaInstruction::SetEntropyMode { mode } => {
                write!(f, "SetEntropyMode {:?}", mode)
            }
            SsaInstruction::Anchor { name } => {
                write!(f, "Anchor {}", name)
            }
            SsaInstruction::Rewind { target, anchor } => {
                write!(f, "Rewind {} to {}", target, anchor)
            }
            SsaInstruction::Commit { vars } => {
                write!(f, "Commit {:?}", vars)
            }
            SsaInstruction::Watchdog {
                target,
                timeout_ms,
                recovery_jump: _,
            } => {
                write!(f, "Watchdog {} {}ms", target, timeout_ms)
            }
            SsaInstruction::Speculate {
                max_ms,
                fallback_target: _,
            } => {
                write!(f, "Speculate {}ms", max_ms)
            }
            SsaInstruction::EndSpeculate {
                max_ms,
                fallback_target: _,
            } => {
                write!(f, "EndSpeculate {}ms", max_ms)
            }
            SsaInstruction::Collapse => {
                write!(f, "Collapse")
            }
            SsaInstruction::Select {
                max_ms,
                cases,
                timeout_target: _,
            } => {
                let cases_str: Vec<String> = cases
                    .iter()
                    .map(|c| {
                        format!("{} -> {} to Block {}", c.chan_id, c.dest, c.target)
                    })
                    .collect();
                write!(f, "Select (max {}ms) [ {} ]", max_ms, cases_str.join(", "))
            }
            SsaInstruction::MatchEntropy { target, .. } => {
                write!(f, "MatchEntropy {}", target)
            }
            SsaInstruction::RelativisticBlock {
                target,
                block_pc,
                block_len,
            } => {
                write!(
                    f,
                    "RelativisticBlock {} pc: {} len: {}",
                    target, block_pc, block_len
                )
            }
            SsaInstruction::SpeculationMode { mode } => {
                write!(f, "SpeculationMode {:?}", mode)
            }
            SsaInstruction::OpenChan {
                name,
                capacity,
                decay_after_ms: _,
            } => {
                write!(f, "OpenChan {}({})", name, capacity)
            }
            SsaInstruction::ChanSend { chan_id, src } => {
                write!(f, "ChanSend {}, {}", chan_id, src)
            }
            SsaInstruction::ChanRecv { dest, chan_id } => {
                write!(f, "{} = ChanRecv {}", dest, chan_id)
            }
            SsaInstruction::AwaitChan { chan_id } => {
                write!(f, "AwaitChan {}", chan_id)
            }
            SsaInstruction::StructLit {
                dest,
                fields,
                type_name: _,
            } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{} = StructLit {{ {} }}", dest, fields_str.join(", "))
            }
            SsaInstruction::TopologyLit { dest, fields } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{} = TopologyLit {{ {} }}", dest, fields_str.join(", "))
            }
            SsaInstruction::ArrayLit { dest, elements } => {
                let elems_str: Vec<String> =
                    elements.iter().map(|e| e.to_string()).collect();
                write!(f, "{} = [ {} ]", dest, elems_str.join(", "))
            }
            SsaInstruction::FieldAccess {
                dest,
                target,
                field,
            } => {
                write!(f, "{} = {}.{}", dest, target, field)
            }
            SsaInstruction::FieldUpdate {
                target,
                old_target,
                field,
                src,
            } => {
                write!(f, "{} = {}.{} <- {}", target, old_target, field, src)
            }
            SsaInstruction::IndexAccess {
                dest,
                target,
                index,
            } => {
                write!(f, "{} = {}[{}]", dest, target, index)
            }
            SsaInstruction::IndexFieldUpdate {
                target,
                old_target,
                index,
                field,
                src,
            } => {
                write!(
                    f,
                    "{} = {}[{}].{} <- {}",
                    target, old_target, index, field, src
                )
            }
            SsaInstruction::AssertTime { op, limit_ms } => {
                write!(f, "AssertTime {:?} {}ms", op, limit_ms)
            }
            SsaInstruction::Capability { cap } => {
                write!(f, "Capability {:?}", cap)
            }
            SsaInstruction::For {
                dest_cond,
                item_reg,
                item_name: _,
                mode,
                source,
                pacing_ms: _,
                max_ms: _,
            } => {
                write!(
                    f,
                    "For {}, {} in {} (mode: {:?})",
                    dest_cond, item_reg, source, mode
                )
            }
            SsaInstruction::EndFor => {
                write!(f, "EndFor")
            }
            SsaInstruction::SplitMap {
                item_reg,
                item_name: _,
                mode,
                source,
                reconcile: _,
            } => {
                write!(f, "SplitMap {} in {} (mode: {:?})", item_reg, source, mode)
            }
            SsaInstruction::EndSplitMap => {
                write!(f, "EndSplitMap")
            }
            SsaInstruction::Defer {
                dest,
                cap,
                deadline_ms,
            } => {
                write!(f, "{} = Defer {:?} deadline: {}ms", dest, cap, deadline_ms)
            }
            SsaInstruction::Await { target } => {
                write!(f, "Await {}", target)
            }
            SsaInstruction::Loop { max_ms } => {
                write!(f, "Loop {}ms", max_ms)
            }
            SsaInstruction::EndLoop { max_ms } => {
                write!(f, "EndLoop {}ms", max_ms)
            }
            SsaInstruction::Break => {
                write!(f, "Break")
            }
            SsaInstruction::LoopTick => {
                write!(f, "LoopTick")
            }
            SsaInstruction::EndLoopTick => {
                write!(f, "EndLoopTick")
            }
            SsaInstruction::While { max_ms } => {
                write!(f, "While {}ms", max_ms)
            }
            SsaInstruction::EndWhile { max_ms } => {
                write!(f, "EndWhile {}ms", max_ms)
            }
            SsaInstruction::ForStep {
                dest_cond,
                item_reg,
                item_name: _,
                source,
                step_ms,
            } => {
                write!(
                    f,
                    "ForStep {}, {} in {} step {}ms",
                    dest_cond, item_reg, source, step_ms
                )
            }
            SsaInstruction::EndForStep => {
                write!(f, "EndForStep")
            }
            SsaInstruction::LoopTickOn { chan_id } => {
                write!(f, "LoopTickOn {}", chan_id)
            }
            SsaInstruction::NetworkRequest { domain } => {
                write!(f, "NetworkRequest {}", domain)
            }
            SsaInstruction::Jump { target } => {
                write!(f, "Jump Block {}", target)
            }
            SsaInstruction::JumpIf { cond, target } => {
                write!(f, "JumpIf {} to Block {}", cond, target)
            }
            SsaInstruction::JumpIfNot { cond, target } => {
                write!(f, "JumpIfNot {} to Block {}", cond, target)
            }
            SsaInstruction::Syscall {
                dest, target, args, ..
            } => {
                write!(f, "{} = syscall({:?}, {:?})", dest, target, args)
            }
            SsaInstruction::AutoDrop { target, spec } => {
                write!(
                    f,
                    "AutoDrop {} ({:?}::{}({}))",
                    target, spec.lib_name, spec.routine_name, spec.field_name
                )
            }
            SsaInstruction::Other(s) => {
                write!(f, "{}", s)
            }
        }
    }
}

impl std::fmt::Display for SsaTerminator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsaTerminator::Jump { target } => {
                write!(f, "Jump Block {}", target)
            }
            SsaTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                write!(
                    f,
                    "Branch {} -> Block {} else Block {}",
                    cond, then_block, else_block
                )
            }
            SsaTerminator::Return { src } => {
                let src_str = src
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "void".to_string());
                write!(f, "Return {}", src_str)
            }
            SsaTerminator::MatchEntropy {
                target,
                valid_block,
                decayed_block,
                pending_block,
                consumed_block,
            } => {
                let mut cases = Vec::new();
                if let Some(b) = valid_block {
                    cases.push(format!("Valid: Block {}", b));
                }
                if let Some(b) = decayed_block {
                    cases.push(format!("Decayed: Block {}", b));
                }
                if let Some(b) = pending_block {
                    cases.push(format!("Pending: Block {}", b));
                }
                if let Some(b) = consumed_block {
                    cases.push(format!("Consumed: Block {}", b));
                }
                write!(f, "MatchEntropy {} -> [ {} ]", target, cases.join(", "))
            }
            SsaTerminator::Select {
                max_ms,
                cases,
                timeout_block,
            } => {
                let cases_str: Vec<String> = cases
                    .iter()
                    .map(|c| {
                        format!("{} -> {} to Block {}", c.chan_id, c.dest, c.target)
                    })
                    .collect();
                let timeout_str = timeout_block
                    .map(|b| format!(", Timeout: Block {}", b))
                    .unwrap_or_default();
                write!(
                    f,
                    "Select (max {}ms) [ {} ]{}",
                    max_ms,
                    cases_str.join(", "),
                    timeout_str
                )
            }
            SsaTerminator::Unreachable => {
                write!(f, "Unreachable")
            }
        }
    }
}

impl std::fmt::Display for SsaCFG {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut block_ids: Vec<&BlockId> = self.blocks.keys().collect();
        block_ids.sort();

        let mut copies: HashMap<SsaReg, SsaReg> = HashMap::new();
        let mut constants: HashMap<SsaReg, String> = HashMap::new();

        for block in self.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    SsaInstruction::ConstInt { dest, value }
                    | SsaInstruction::LoadInt { dest, value } => {
                        constants.insert(*dest, value.to_string());
                    }
                    SsaInstruction::ConstFloat { dest, value }
                    | SsaInstruction::LoadFloat { dest, value } => {
                        constants.insert(*dest, value.to_string());
                    }
                    SsaInstruction::ConstBool { dest, value }
                    | SsaInstruction::LoadBool { dest, value } => {
                        constants.insert(*dest, value.to_string());
                    }
                    SsaInstruction::ConstString { dest, value }
                    | SsaInstruction::LoadString { dest, value } => {
                        constants.insert(*dest, format!("{:?}", value));
                    }
                    SsaInstruction::ConstNull { dest }
                    | SsaInstruction::LoadNull { dest } => {
                        constants.insert(*dest, "null".to_string());
                    }
                    SsaInstruction::Move { dest, src } => {
                        copies.insert(*dest, *src);
                    }
                    _ => {}
                }
            }
        }

        let _resolve_reg = |reg: SsaReg| -> String {
            let mut curr = reg;
            while let Some(&next) = copies.get(&curr) {
                curr = next;
            }
            if let Some(val) = constants.get(&curr) {
                val.clone()
            } else {
                format!("{}", curr)
            }
        };

        for id in block_ids {
            let block = &self.blocks[id];
            writeln!(f, "  Block {}:", id)?;
            for phi in &block.phi_nodes {
                let incoming_str: Vec<String> = phi
                    .incoming
                    .iter()
                    .map(|(b, reg)| format!("{} from Block {}", reg, b))
                    .collect();
                writeln!(
                    f,
                    "    {} = phi [ {} ]",
                    phi.dest,
                    incoming_str.join(", ")
                )?;
            }
            for instr in &block.instructions {
                writeln!(f, "    {}", instr)?;
            }
            writeln!(f, "    Terminator: {}", block.terminator)?;
        }
        Ok(())
    }
}

impl SsaCFG {
    pub fn to_dot(&self) -> String {
        let mut dot = String::new();
        dot.push_str("digraph SsaCFG {\n");
        dot.push_str("  node [shape=box, fontname=\"Courier\"];\n");

        let mut block_ids: Vec<&BlockId> = self.blocks.keys().collect();
        block_ids.sort();

        for &id in &block_ids {
            let block = &self.blocks[id];
            let mut label = format!("Block {}\\n", id);
            for phi in &block.phi_nodes {
                let incoming_str: Vec<String> = phi
                    .incoming
                    .iter()
                    .map(|(b, reg)| format!("{} from Block {}", reg, b))
                    .collect();
                label.push_str(&format!(
                    "  {} = phi [ {} ]\\n",
                    phi.dest,
                    incoming_str.join(", ")
                ));
            }
            for instr in &block.instructions {
                let clean_instr = format!("{}", instr)
                    .replace('"', "\\\"")
                    .replace('\n', "\\n");
                label.push_str(&format!("  {}\\n", clean_instr));
            }
            let clean_term = format!("{}", block.terminator)
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            label.push_str(&format!("  Terminator: {}\\n", clean_term));

            dot.push_str(&format!("  block_{} [label=\"{}\"];\n", id, label));

            // Successors
            match &block.terminator {
                SsaTerminator::Jump { target } => {
                    dot.push_str(&format!("  block_{} -> block_{};\n", id, target));
                }
                SsaTerminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    dot.push_str(&format!(
                        "  block_{} -> block_{};\n",
                        id, then_block
                    ));
                    dot.push_str(&format!(
                        "  block_{} -> block_{};\n",
                        id, else_block
                    ));
                }
                SsaTerminator::MatchEntropy {
                    valid_block,
                    decayed_block,
                    pending_block,
                    consumed_block,
                    ..
                } => {
                    if let Some(b) = valid_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"Valid\"];\n",
                            id, b
                        ));
                    }
                    if let Some(b) = decayed_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"Decayed\"];\n",
                            id, b
                        ));
                    }
                    if let Some(b) = pending_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"Pending\"];\n",
                            id, b
                        ));
                    }
                    if let Some(b) = consumed_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"Consumed\"];\n",
                            id, b
                        ));
                    }
                }
                SsaTerminator::Select {
                    cases,
                    timeout_block,
                    ..
                } => {
                    for case in cases {
                        dot.push_str(&format!(
                            "  block_{} -> block_{};\n",
                            id, case.target
                        ));
                    }
                    if let Some(b) = timeout_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"Timeout\"];\n",
                            id, b
                        ));
                    }
                }
                _ => {}
            }
        }

        dot.push_str("}\n");
        dot
    }
}
