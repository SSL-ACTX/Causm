pub use causm_core::SaturationPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxOverflowAction {
    EvictedOldest,
    Overwritten,
    DroppedNew,
    Blocked,
}
