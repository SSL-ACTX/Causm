pub mod bounded;
pub mod policies;

pub use bounded::{BoundedMailbox, MailboxError};
pub use policies::{MailboxOverflowAction, SaturationPolicy};
