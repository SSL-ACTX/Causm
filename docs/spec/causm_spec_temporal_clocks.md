# Causm Specification: Temporal Quad-Clock Model & Capability Integration

## 1. Overview of Temporal Quad-Clock Architecture

In Causm, time and execution progression are formally decomposed across **four distinct clock models**. This separation guarantees that static entropic safety verification can be proved independently of non-deterministic hardware or external network conditions.

| Clock Layer | Description & Role |
| :--- | :--- |
| **1. Static WCET Budget** | Proved statically at compile-time by Z3. |
| **2. TVM Logical Clock** | Discrete step advancement in runtime VM. |
| **3. External Cap Latency** | Physical latency requested/incurred by I/O. |
| **4. OS Elapsed Time** | Real wall-clock time on host system CPU. |

### 1.1 Static WCET Budget (Analyzer & Z3)
- Computed during static entropic analysis using Z3 solver.
- Proves bounds for worst-case execution time, temporal gates (`@40ms:`), loop bounds (`(max 25ms)`), and routine execution contracts (`taking 10ms`).
- Does **not** perform actual I/O or execute network requests.

### 1.2 TVM Logical Clock (`local_clock`)
- Managed per timeline branch within the Virtual Machine (`causm-runtime`).
- Tracks deterministic clock progression and advances when promises yield/await or time-slices elapse.

### 1.3 External Capability Latency
- The simulated or physical duration requested by external capability drivers (`System.NetworkFetch`, `System.Log`).
- If capability execution duration exceeds a promise's `deadline`, the TVM transitions the promise state to `EntropicState::Consumed`.

### 1.4 OS Elapsed Time
- The physical host wall-clock elapsed time on the host CPU.

---

## 2. Stdlib Boundary & Entropy Resolution Contract

Capabilities registered in `causm-stdlib` return a typed `Result<Payload, String>`:

```rust
pub type CapHandler = Box<dyn Fn(&HashMap<String, String>) -> Result<Payload, String>>;
```

### 2.1 Capability State Resolution Invariant
When `await(promise)` is executed by TVM:
1. **Network / I/O Success:** The capability handler returns `Ok(payload)`. TVM inserts `EntropicState::Valid(payload)` into the target register. `match entropy(promise)` routes to the `Valid(val)` branch.
2. **Network / Transport Failure or Deadline Breach:** The capability handler returns `Err(error)` or breaches `deadline_at`. TVM inserts `EntropicState::Consumed` into the target register. `match entropy(promise)` routes to the `Consumed` branch.
