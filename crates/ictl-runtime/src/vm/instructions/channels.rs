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
        println!(
            "[VM] ChanSend: branch={}, chan={}, val={:?}",
            branch_id, chan_id, val
        );
        let message = Message {
            id: self.next_payload_id,
            sender: branch_id.to_string(),
            payload: val,
        };
        self.next_payload_id += 1;

        let is_isochronous = self.get_branch_mut(branch_id)?.slice_ms.is_some();

        if is_isochronous {
            if let Some(pending) = self.pending_channels.get_mut(&chan_id) {
                println!("[VM] ChanSend: pushing to PENDING");
                pending.push_back(message.clone());
            } else {
                return Err(TemporalError::ChannelFault(format!(
                    "Channel not found: {}",
                    chan_id
                )));
            }
        } else if let Some(chan) = self.channels.get_mut(&chan_id) {
            println!("[VM] ChanSend: pushing to ACTIVE");
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

    pub(crate) fn ChanRecv(
        &mut self,
        branch_id: &str,
        dest: Reg,
        chan_id: String,
    ) -> Result<(), TemporalError> {
        println!("[VM] ChanRecv: branch={}, chan={}", branch_id, chan_id);
        let message = {
            let chan = self.channels.get_mut(&chan_id).ok_or_else(|| {
                TemporalError::ChannelFault(format!(
                    "Channel not found: {}",
                    chan_id
                ))
            })?;
            println!("[VM] ChanRecv: queue size={}", chan.len());
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
