use crate::mailbox::policies::{MailboxOverflowAction, SaturationPolicy};
use std::collections::VecDeque;

#[derive(Debug, thiserror::Error)]
pub enum MailboxError<T> {
    #[error("Mailbox is full")]
    Full(T),
    #[error("Mailbox capacity must be greater than 0")]
    ZeroCapacity,
}

/// A statically-bounded mailbox backed by a fixed capacity envelope and deterministic saturation policy.
pub struct BoundedMailbox<T> {
    buffer: VecDeque<T>,
    capacity: usize,
    policy: SaturationPolicy,
}

impl<T> BoundedMailbox<T> {
    pub fn new(capacity: usize, policy: SaturationPolicy) -> Self {
        assert!(
            capacity > 0,
            "BoundedMailbox capacity must be greater than 0"
        );
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            policy,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    pub fn policy(&self) -> SaturationPolicy {
        self.policy
    }

    pub fn set_policy(&mut self, policy: SaturationPolicy) {
        self.policy = policy;
    }

    /// Push a message into the mailbox applying the configured saturation policy on overflow.
    pub fn push(
        &mut self,
        message: T,
    ) -> Result<Option<MailboxOverflowAction>, MailboxError<T>> {
        if self.buffer.len() < self.capacity {
            self.buffer.push_back(message);
            return Ok(None);
        }

        match self.policy {
            SaturationPolicy::RingBuffer | SaturationPolicy::EvictDecayed => {
                // Evict the oldest message to accept the newest message
                self.buffer.pop_front();
                self.buffer.push_back(message);
                Ok(Some(MailboxOverflowAction::EvictedOldest))
            }
            SaturationPolicy::Throttle => {
                // Reject the new message, preserving unread mailbox state
                Err(MailboxError::Full(message))
            }
            SaturationPolicy::FailFast => Err(MailboxError::Full(message)),
        }
    }

    /// Try pushing a message without triggering overflow eviction.
    pub fn try_push(&mut self, message: T) -> Result<(), MailboxError<T>> {
        if self.buffer.len() >= self.capacity {
            Err(MailboxError::Full(message))
        } else {
            self.buffer.push_back(message);
            Ok(())
        }
    }

    /// Pop the next message from the mailbox.
    pub fn pop(&mut self) -> Option<T> {
        self.buffer.pop_front()
    }

    /// Pop the next message from the front of the mailbox.
    pub fn pop_front(&mut self) -> Option<T> {
        self.buffer.pop_front()
    }

    /// Push a message to the front of the mailbox.
    pub fn push_front(
        &mut self,
        message: T,
    ) -> Result<Option<MailboxOverflowAction>, MailboxError<T>> {
        if self.buffer.len() < self.capacity {
            self.buffer.push_front(message);
            return Ok(None);
        }

        match self.policy {
            SaturationPolicy::RingBuffer | SaturationPolicy::EvictDecayed => {
                self.buffer.pop_back();
                self.buffer.push_front(message);
                Ok(Some(MailboxOverflowAction::EvictedOldest))
            }
            SaturationPolicy::Throttle => Err(MailboxError::Full(message)),
            SaturationPolicy::FailFast => Err(MailboxError::Full(message)),
        }
    }

    /// Append the contents of another mailbox to this mailbox.
    pub fn append(&mut self, other: &mut Self) {
        while let Some(item) = other.pop_front() {
            match self.push(item) {
                Ok(_) => {}
                Err(MailboxError::Full(item)) => {
                    if matches!(self.policy, SaturationPolicy::RingBuffer | SaturationPolicy::EvictDecayed) {
                        self.buffer.pop_front();
                        self.buffer.push_back(item);
                    } else {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    /// Remove the item at the given index.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.buffer.remove(index)
    }

    /// Get an iterator over the mailbox contents.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }

    /// Peek at the next message without consuming it.
    pub fn peek(&self) -> Option<&T> {
        self.buffer.front()
    }

    /// Peek at the latest message without consuming it.
    pub fn peek_back(&self) -> Option<&T> {
        self.buffer.back()
    }

    /// Clear all messages in the mailbox.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}
