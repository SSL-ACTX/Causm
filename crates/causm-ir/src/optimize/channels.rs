use crate::optimize::OptimizationPass;
use crate::ssa::{SsaCFG, SsaInstruction, SsaTerminator};
use std::collections::HashSet;

/// Channel liveness analysis & dead channel elimination pass.
/// Removes unused `OpenChan` instructions if the channel is never sent to, received from,
/// or used in `AwaitChan` / `LoopTickOn` / `Select` **across the entire program**.
///
/// The `globally_referenced_channels` set is pre-populated by `optimize_program` with
/// all channel names that are referenced in any IR block or routine — not just the one
/// currently being optimized. This prevents incorrect elimination of `OpenChan` when the
/// open is in one temporal block (e.g. `@40ms`) and the corresponding send/recv are in a
/// different block (e.g. `@50ms` or a branch block).
pub struct ChannelLivenessPass {
    pub globally_referenced_channels: HashSet<String>,
}

impl ChannelLivenessPass {
    pub fn new(globally_referenced_channels: HashSet<String>) -> Self {
        Self {
            globally_referenced_channels,
        }
    }
}

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
        // Build the union of channels used locally within this CFG and the globally
        // referenced channels collected from the entire program.
        let mut used_channels = self.globally_referenced_channels.clone();

        // 1. Scan for used channel names across all instructions and terminators
        //    in this specific CFG (in case an OpenChan and its uses are co-located).
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
