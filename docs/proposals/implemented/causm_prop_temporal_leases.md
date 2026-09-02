# Proposal: Temporal Leases and Transient Entropic Borrowing

**Status:** Approved & Implemented  
**Author:** Seuriin <seuriin@gmail.com>  
**Category:** Entropic Memory & Borrowing Semantics  

---

This document specifies the formal requirements for introducing **Temporal Leases** into the Causm. This mechanism provides transient, time-bound access to entropic memory structures, mitigating the necessity for resource-intensive explicit replication (`clone`) while preserving the deterministic security guarantees of the Entropic Memory Model.

---

## 1. The Challenge of Transient Access
Currently, accessing a value within Causm inherently triggers structural decay or complete consumption. If a timeline requires read-only access to a data structure (e.g., for validation or metric gathering) without destroying the source, the developer is forced to explicitly `clone()` the data. 

As defined in the Causm RFC, replication incurs a deterministic CPU penalty based on structural depth and data volume. Overuse of replication for transient reading unnecessarily exhausts execution budgets and impacts the temporal pacing of isolated timelines. The previously proposed `inspect` construct provides a scoped read-only view, but lacks rigorous temporal bounding for complex operations. 

## 2. The `lease` Mechanism
The **Temporal Lease** establishes a mathematically bounded borrow of a variable for a strcausmy defined temporal duration. 

* **Functional Requirement**: The `lease` primitive temporarily grants access to an entropic structure. The access is strcausmy enforced by a deterministic temporal budget (the lease duration).
* **Entropic Lock State**: While leased, the original variable enters a `Leased(Duration)` state. It cannot be mutated, transmitted, or consumed until the lease expires or is explicitly relinquished.
* **Automatic Restoration**: Upon expiration of the lease duration (as tracked by the relative local clock), the lease is automatically revoked, and the parent variable is restored to its `Valid` state.

## 3. Formal Syntax Specification

```causm
@0ms: {
    let system_state = get_current_state()

    // Establish a lease for exactly 15ms.
    // 'system_state' transitions to Leased(15ms)
    lease read_only_view = system_state for 15ms {
        
        let valid = validate_state(read_only_view)
        
        if (valid == false) {
            System.Log(message="State invalid.")
        }
        // Implicit relinquishment occurs at the end of the block 
        // if execution time < 15ms.
    }
    
    // At @+15ms relative to the lease initiation, 'system_state' is 'Valid' again.
    // Deterministic padding ensures execution synchronization.
}
```

## 4. Semantics and Entropic Regulations

### 4.1 Temporal Expiration and Pacing
Leases are governed by the **Padding Rule**. If the logic within the `lease` block executes faster than the specified lease duration, the TVM injects deterministic temporal padding to ensure the local clock advances by exactly the lease amount before restoring the parent variable's entropic state. 

If the logic attempts to exceed the lease duration, the TVM triggers a **Temporal Fault**, terminating the isolate/branch, preventing use-after-lease violations.

### 4.2 Prohibited Operations Within a Lease
- **Mutation**: Data accessed via a lease is strcausmy immutable.
- **Entropic Transmission**: Leased data cannot be sent over channels (`chan_send`) or passed to routines that expect owned parameters, as this would violate the Single-Ownership Invariant.
- **Nested Leasing**: To prevent unbounded static analysis complexity, a leased variable cannot be sub-leased. 

### 4.3 Structural Decay of Leases
If a specific field of a leased structure is accessed, the lease itself undergoes *virtual decay*. 
```causm
lease meta = network_packet for 10ms {
    // Virtual decay of 'meta'. 'network_packet' remains locked.
    let header_lease = meta.header 
    
    // header_lease is valid for the remaining duration of the 10ms budget.
}
```

## 5. Virtual Machine Implementation Specifications

1. **Entropic State Tracking**: The `EntropicState` enum is extended to include `Leased { expiration_coord: TemporalCoordinate }`.
2. **Static Analysis (WCET)**: The static analyzer must calculate the Worst-Case Execution Time (WCET) of the `lease` block. 
   - If `WCET > Lease_Duration`, a compile-time error is emitted.
   - If `WCET <= Lease_Duration`, the TVM schedules padding `P = Lease_Duration - Actual_Execution_Time`.
3. **Reclamation**: Leases do not impact Garbage Collection directly, as they do not generate new memory allocations. They represent a scoped pointer accompanied by a temporal invariant.

---

### Architectural Alignment
Temporal Leases directly integrate the concept of memory borrowing with the language's fundamental temporal execution primitives. By substituting arbitrary lifetime scopes with deterministic time budgets, Causm maintains its core commitment to causal predictability while significantly improving developer ergonomics and resource efficiency.