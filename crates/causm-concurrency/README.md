# causm-concurrency

Phase 1 concurrency core for Causm: bounded mailboxes, lock-free queue primitives, and cooperative actor scheduling.

## Included primitives

- `SpscQueue<T, const N: usize>` for single-producer / single-consumer bounded transfer
- `MpmcQueue<T, const N: usize>` for bounded multi-producer / multi-consumer transfer
- `BoundedMailbox<T>` for deterministic message buffering with saturation policies
- `ActorPool<M>` with round-robin actor scheduling and time-slice tracking
- `TimeSlice` for cooperative turn budgets and deadline checks

## Saturation policies

`BoundedMailbox` accepts a `SaturationPolicy` derived from core semantics:

- `EvictDecayed`: drop the oldest item and accept the newest message
- `RingBuffer`: keep the mailbox bounded with a rotating oldest-drop policy
- `Throttle`: reject overflowed sends without mutating mailbox state
- `FailFast`: reject overflowed sends immediately and surface an error

## Example

```rust
use causm_concurrency::mailbox::{BoundedMailbox, SaturationPolicy};

let mut mailbox = BoundedMailbox::new(2, SaturationPolicy::RingBuffer);
mailbox.push("a").unwrap();
mailbox.push("b").unwrap();
mailbox.push("c").unwrap();
assert_eq!(mailbox.pop(), Some("b"));
```

This crate is intentionally pure-Rust and decoupled from VM execution so that future runtime integration and Kani verification can stay isolated to the Phase 1 core.
