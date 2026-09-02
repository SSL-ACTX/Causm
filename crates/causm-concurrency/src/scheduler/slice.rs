use std::time::{Duration, Instant};

/// Microsecond-level time-slice tracking and enforcement for cooperative actor turns.
#[derive(Debug, Clone)]
pub struct TimeSlice {
    budget: Duration,
    started_at: Option<Instant>,
    accumulated: Duration,
}

impl TimeSlice {
    pub fn new(budget: Duration) -> Self {
        Self {
            budget,
            started_at: None,
            accumulated: Duration::ZERO,
        }
    }

    pub fn from_millis(ms: u64) -> Self {
        Self::new(Duration::from_millis(ms))
    }

    pub fn from_micros(us: u64) -> Self {
        Self::new(Duration::from_micros(us))
    }

    pub fn start(&mut self) {
        self.started_at = Some(Instant::now());
    }

    pub fn pause(&mut self) {
        if let Some(start) = self.started_at.take() {
            self.accumulated += start.elapsed();
        }
    }

    pub fn reset(&mut self) {
        self.started_at = None;
        self.accumulated = Duration::ZERO;
    }

    pub fn elapsed(&self) -> Duration {
        let current = self
            .started_at
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        self.accumulated + current
    }

    pub fn remaining(&self) -> Duration {
        self.budget.saturating_sub(self.elapsed())
    }

    pub fn is_expired(&self) -> bool {
        self.elapsed() >= self.budget
    }

    pub fn budget(&self) -> Duration {
        self.budget
    }
}
