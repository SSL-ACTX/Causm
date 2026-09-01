#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    Deferred,
    Ready,
    Passed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerificationPlan {
    pub mailbox_invariants: bool,
    pub queue_invariants: bool,
    pub pace_invariants: bool,
}

impl VerificationPlan {
    pub const fn new() -> Self {
        Self {
            mailbox_invariants: true,
            queue_invariants: true,
            pace_invariants: true,
        }
    }
}

impl Default for VerificationPlan {
    fn default() -> Self {
        Self::new()
    }
}

pub fn verification_status() -> VerificationStatus {
    VerificationStatus::Deferred
}

pub fn verification_plan() -> VerificationPlan {
    VerificationPlan::new()
}
