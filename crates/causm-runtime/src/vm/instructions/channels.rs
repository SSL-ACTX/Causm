use crate::vm::error::TemporalError;
use crate::vm::state::{CausalEvent, Message, Vm};
use causm_ir::Reg;
use std::collections::VecDeque;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn OpenChan(
        &mut self,
        _branch_id: &str,
        name: String,
        capacity: usize,
        decay_after_ms: Option<u64>,
    ) -> Result<(), TemporalError> {
        self.channels
            .insert(name.clone(), VecDeque::with_capacity(capacity));
        self.pending_channels
            .insert(name.clone(), VecDeque::with_capacity(capacity));
        if let Some(decay) = decay_after_ms {
            self.channel_decay_limits.insert(name, decay);
        }
        Ok(())
    }

    pub(crate) fn ChanSend(
        &mut self,
        branch_id: &str,
        chan_id: String,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let type_name = {
            let branch = self.get_branch_mut(branch_id)?;
            let idx = src.0 as usize;
            if idx < branch.arena.metadata.len() {
                branch.arena.metadata[idx]
                    .as_ref()
                    .and_then(|m| m.type_name.clone())
            } else {
                None
            }
        };
        let sent_at = {
            let branch = self.get_branch_mut(branch_id)?;
            branch.birth_global_time + branch.local_clock
        };
        let message = Message {
            id: self.next_payload_id,
            sender: branch_id.to_string(),
            payload: val,
            sent_at,
            type_name,
        };
        self.next_payload_id += 1;

        let is_isochronous = self.get_branch_mut(branch_id)?.slice_ms.is_some();

        if is_isochronous {
            if let Some(pending) = self.pending_channels.get_mut(&chan_id) {
                pending.push_back(message.clone());
            } else {
                return Err(TemporalError::ChannelFault(format!(
                    "Channel not found: {}",
                    chan_id
                )));
            }
        } else if let Some(chan) = self.channels.get_mut(&chan_id) {
            chan.push_back(message.clone());
        } else {
            return Err(TemporalError::ChannelFault(format!(
                "Channel not found: {}",
                chan_id
            )));
        }

        self.causal_history.push(CausalEvent::ChannelSend {
            branch_id: branch_id.to_string(),
            channel_id: chan_id,
            payload_id: message.id,
        });
        Ok(())
    }

    pub(crate) fn AwaitChan(
        &mut self,
        branch_id: &str,
        chan_id: String,
    ) -> Result<(), TemporalError> {
        let sent_at = {
            let chan = self.channels.get(&chan_id).ok_or_else(|| {
                TemporalError::ChannelFault(format!(
                    "Channel not found: {}",
                    chan_id
                ))
            })?;
            chan.front().map(|m| m.sent_at).ok_or_else(|| {
                TemporalError::ChannelFault(format!("Channel empty: {}", chan_id))
            })?
        };

        let branch = self.get_branch_mut(branch_id)?;
        let current_global_time = branch.birth_global_time + branch.local_clock;

        if current_global_time < sent_at {
            let wait = sent_at - current_global_time;
            branch.local_clock += wait;
            branch.consume_budget(wait)?;
        }

        Ok(())
    }

    pub(crate) fn ChanRecv(
        &mut self,
        branch_id: &str,
        dest: Reg,
        chan_id: String,
    ) -> Result<(), TemporalError> {
        let message = {
            let chan = self.channels.get_mut(&chan_id).ok_or_else(|| {
                TemporalError::ChannelFault(format!(
                    "Channel not found: {}",
                    chan_id
                ))
            })?;
            chan.pop_front().ok_or_else(|| {
                TemporalError::ChannelFault(format!("Channel empty: {}", chan_id))
            })?
        };

        let current_global_time = {
            let branch = self.get_branch_mut(branch_id)?;
            branch.birth_global_time + branch.local_clock
        };

        let elapsed = current_global_time.saturating_sub(message.sent_at);
        let decay_limit = self.channel_decay_limits.get(&chan_id).cloned();
        let is_decayed = if let Some(limit) = decay_limit {
            elapsed >= limit
        } else {
            false
        };

        let final_state = if is_decayed {
            causm_core::value::EntropicState::Valid(message.payload.clone())
                .decay_recursive()
        } else {
            causm_core::value::EntropicState::Valid(message.payload.clone())
        };

        self.insert_reg(branch_id, dest.0, final_state)?;

        // Populate metadata for the register in the receiver's arena.
        {
            let branch = self.get_branch_mut(branch_id)?;
            let idx = dest.0 as usize;
            if idx < branch.arena.metadata.len() {
                branch.arena.metadata[idx] =
                    Some(causm_core::value::ValueMetadata {
                        instantiated_at: message.sent_at,
                        type_name: message.type_name.clone(),
                        decay_after_ms: decay_limit,
                    });
            }
        }

        // Execute recovery / decay handler if one is registered for the type.
        if is_decayed {
            if let Some(type_name) = &message.type_name {
                if let Some(handler_instrs) =
                    self.decay_handlers.get(type_name).cloned()
                {
                    self._is_decaying = true;
                    let res = self.execute_instructions(branch_id, &handler_instrs);
                    self._is_decaying = false;
                    res?;
                }
            }
        }

        self.causal_history.push(CausalEvent::ChannelRecv {
            branch_id: branch_id.to_string(),
            channel_id: chan_id,
            message,
        });
        Ok(())
    }
}
