use crate::ssa::{SsaCFG, SsaInstruction, SsaTerminator};
use crate::optimize::OptimizationPass;
use std::collections::HashSet;

/// Channel liveness analysis & dead channel elimination pass.
/// Removes unused `OpenChan` instructions if the channel is never sent to, received from,
/// or used in `AwaitChan` / `LoopTickOn` / `Select`.
pub struct ChannelLivenessPass;

impl OptimizationPass for ChannelLivenessPass {
    fn name(&self) -> &str {
        "channel-liveness"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        let mut used_channels = HashSet::new();

        // 1. Scan for used channel names across all instructions and terminators
        for block in ssa_cfg.blocks.values() {
            for instr in &block.instructions {
                match instr {
                    SsaInstruction::ChanSend { chan_id, .. }
                    | SsaInstruction::ChanRecv { chan_id, .. }
                    | SsaInstruction::AwaitChan { chan_id }
                    | SsaInstruction::LoopTickOn { chan_id } => {
                        used_channels.insert(chan_id.clone());
                    }
                    SsaInstruction::Select { cases, .. } => {
                        for c in cases {
                            used_channels.insert(c.chan_id.clone());
                        }
                    }
                    _ => {}
                }
            }

            if let SsaTerminator::Select { cases, .. } = &block.terminator {
                for c in cases {
                    used_channels.insert(c.chan_id.clone());
                }
            }
        }

        // 2. Eliminate unused `OpenChan` instructions
        let mut changed = false;
        for block in ssa_cfg.blocks.values_mut() {
            let before_len = block.instructions.len();
            block.instructions.retain(|instr| {
                if let SsaInstruction::OpenChan { name, .. } = instr {
                    used_channels.contains(name)
                } else {
                    true
                }
            });
            if block.instructions.len() != before_len {
                changed = true;
            }
        }

        changed
    }
}
