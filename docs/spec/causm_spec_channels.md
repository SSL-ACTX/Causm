# Specification: Entropic Channels

## 1. Overview
Entropic Channels provide a mechanism for deterministic mid-execution communication between split timelines. Unlike formal `merge` operations which occur at the end of a timeline's lifecycle, channels allow for asynchronous state transfer while maintaining strict single-ownership invariants.

## 2. Channel Manifests
Channels must be declared in the `isolate` manifest using the `Chan` capability.

```causm
isolate Producer {
    require Chan.Outbound(id="data_pipe", type=int)
}

isolate Consumer {
    require Chan.Inbound(id="data_pipe", type=int, latency=5ms)
}
```

- **id**: A unique global identifier for the channel.
- **type**: The entropic type allowed for transmission.
- **latency**: The maximum allowed temporal offset between transmission and reception.

## 3. Transmission Primitives

### 3.1 `chan_send`
Invoking `chan_send` moves a variable from the sender's memory arena into the channel buffer.
- **Entropic Effect**: The variable is marked as `Consumed` in the sender's arena.
- **Temporal Effect**: The operation is a "Causal Commitment," advancing the sender's causal horizon.

### 3.2 `chan_recv` (Non-Blocking)
Attempts to extract a value from the channel. If the channel is empty, it returns `Null`.
- **Entropic Effect**: If successful, a new `Valid` variable is initialized in the receiver's arena.

## 4. Synchronization Primitives

### 4.1 `await_chan`
Suspends the receiver's local clock until a message is available in the specified channel.
- **Temporal Alignment**: Upon reception, the receiver's `local_clock` is advanced to align with the sender's global temporal coordinate, ensuring a deterministic causal link.
- **WCET Impact**: The analyzer assumes the maximum `latency` defined in the manifest for WCET estimation.

## 5. Formal Invariants
The Z3 Correctness Kernel proves the following for all channel operations:
1.  **Single Ownership**: A variable sent via `chan_send` cannot be accessed again on the sender's timeline.
2.  **Type Safety**: The payload type must match the manifest declaration.
3.  **No Causal Loops**: A timeline cannot receive a message from its own future (proven via causal horizon tracking).
