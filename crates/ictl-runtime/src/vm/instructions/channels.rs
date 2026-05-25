use crate::vm::error::TemporalError;
use crate::vm::state::{CausalEvent, Message, Vm};
use ictl_frontend::ir::Reg;
use std::collections::VecDeque;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn OpenChan(
        &mut self,
        _branch_id: &str,
        name: String,
        capacity: usize,
    ) -> Result<(), TemporalError> {
        self.channels
            .insert(name.clone(), VecDeque::with_capacity(capacity));
        self.pending_channels
            .insert(name, VecDeque::with_capacity(capacity));
        Ok(())
    }

    pub(crate) fn ChanSend(
        &mut self,
        branch_id: &str,
        chan_id: String,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let sent_at = {
            let branch = self.get_branch_mut(branch_id)?;
            branch.birth_global_time + branch.local_clock
        };
        let message = Message {
            id: self.next_payload_id,
            sender: branch_id.to_string(),
            payload: val,
            sent_at,
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

        self.insert_reg(
            branch_id,
            dest.0,
            ictl_core::value::EntropicState::Valid(message.payload.clone()),
        )?;
        self.causal_history.push(CausalEvent::ChannelRecv {
            branch_id: branch_id.to_string(),
            channel_id: chan_id,
            message,
        });
        Ok(())
    }
}
