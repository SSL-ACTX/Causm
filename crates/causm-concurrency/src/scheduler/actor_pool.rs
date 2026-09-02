use crate::mailbox::BoundedMailbox;
use crate::scheduler::slice::TimeSlice;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

pub struct ActorHandle<M> {
    pub name: String,
    pub mailbox: BoundedMailbox<M>,
    pub slice: TimeSlice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnResult {
    Completed,
    SliceExpired,
    MailboxEmpty,
}

/// Cooperative actor scheduler pool enforcing deterministic round-robin time slicing.
pub struct ActorPool<M> {
    actors: HashMap<String, ActorHandle<M>>,
    ready_queue: VecDeque<String>,
}

impl<M> ActorPool<M> {
    pub fn new() -> Self {
        Self {
            actors: HashMap::new(),
            ready_queue: VecDeque::new(),
        }
    }

    pub fn register_actor(
        &mut self,
        name: String,
        mailbox: BoundedMailbox<M>,
        slice_budget: Duration,
    ) {
        let handle = ActorHandle {
            name: name.clone(),
            mailbox,
            slice: TimeSlice::new(slice_budget),
        };
        self.actors.insert(name.clone(), handle);
        self.ready_queue.push_back(name);
    }

    pub fn get_actor_mut(&mut self, name: &str) -> Option<&mut ActorHandle<M>> {
        self.actors.get_mut(name)
    }

    pub fn send_to(
        &mut self,
        target: &str,
        message: M,
    ) -> Result<(), crate::mailbox::MailboxError<M>> {
        if let Some(actor) = self.actors.get_mut(target) {
            actor.mailbox.push(message)?;
            if !self.ready_queue.contains(&target.to_string()) {
                self.ready_queue.push_back(target.to_string());
            }
            Ok(())
        } else {
            Err(crate::mailbox::MailboxError::Full(message))
        }
    }

    pub fn next_ready_actor(&mut self) -> Option<String> {
        self.ready_queue.pop_front()
    }

    pub fn requeue_actor(&mut self, name: String) {
        if self.actors.contains_key(&name) && !self.ready_queue.contains(&name) {
            self.ready_queue.push_back(name);
        }
    }

    pub fn active_actors_count(&self) -> usize {
        self.actors.len()
    }
}

impl<M> Default for ActorPool<M> {
    fn default() -> Self {
        Self::new()
    }
}
