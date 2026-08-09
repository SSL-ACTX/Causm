# Causm SSA / IR TODOs

## High Priority

- [x] Lower structured loop bodies (`ForStep`, `LoopTickOn`, possibly `For`) into explicit CFG blocks instead of embedding instruction arrays.
  - Current IR mixes SSA and AST.
  - Optimizers (GVN, LICM, DCE, PRE, etc.) work much better over normal CFG.

- [x] Preserve temporal semantics while lowering.
  - Keep instructions like:
    - `While { max_ms }`
    - `EndWhile`
    - `Slice`
    - `LoopTickOn`
    - `ForStep`
  - But let their bodies become ordinary basic blocks.

---

## SSA Cleanup

- [x] Remove unnecessary `Move` instructions after SSA construction.
  - Example:
    ```
    LoadInt R1 = 5
    Move R2 = R1
    ```
    could become
    ```
    ConstInt R2 = 5
    ```
    or directly
    ```
    R2 = 5
    ```

- [x] Add a copy-propagation pass.
  - Eliminate redundant SSA copies.
  - Reduce register count.
  - Improve readability.

- [x] Add dead instruction elimination after SSA generation.
  - Many temporary registers become unused after propagation.

---

## Constants

- [x] Consider introducing dedicated constant instructions.
  - `ConstInt`
  - `ConstBool`
  - `ConstFloat`
  - `ConstString`
  - `ConstNull`

---

## CFG

- [x] Normalize all structured control flow.
  - Ensure loops always become:
    - header
    - body
    - backedge
    - exit

- [x] Reduce embedded control-flow instructions inside IR nodes.

---

## Temporal Analysis

- [x] Keep temporal instructions visible for later optimization.
  - Do **not** lower them into runtime calls too early.

Useful analyses:

- Worst-case execution time (WCET)
- Deadline verification
- Schedule validation
- Clock consistency
- Time-budget optimization

---

## Entropy System

Current design looks good.

Keep:

- `MatchEntropy`
- `Consume`
- `Lease`
- `EndLease`

These preserve semantic information that would otherwise disappear.

Possible additions:

- [x] Entropy propagation analysis.
- [x] Dead entropy-state elimination.
- [x] Detect redundant `MatchEntropy`.

---

## Channels

Current design looks good.

Keep as first-class IR instructions:

- `OpenChan`
- `ChanSend`
- `ChanRecv`
- `AwaitChan`
- `LoopTickOn`

Potential future optimizations:

- [x] Channel liveness analysis.
- [x] Dead channel elimination.
- [ ] Queue occupancy analysis.
- [ ] Static capacity verification.

---

## Concurrency

Current design looks promising.

Keep:

- `Split`
- `Merge`
- `RelativisticBlock`

Potential analyses:

- [x] Branch lifetime analysis.
- [x] Merge correctness verification.
- [ ] Parallel dependency analysis.
- [ ] Race detection (if mutable state exists).

---

## Leases

Current implementation looks solid.

Potential additions:

- [x] Nested lease optimization.
- [x] Lease lifetime verification.
- [x] Lease escape analysis.
- [x] Automatic redundant lease removal.

---

## Pretty Printer

- [x] Reduce SSA verbosity.
- [x] Collapse trivial copy chains.
- [x] Print constants inline when possible.
- [x] Optional "optimized SSA" dump after cleanup passes.
- [x] Optional CFG visualization.

---

## Optimizer Passes

Potential pass order:

1. Constant propagation
2. Copy propagation
3. Dead code elimination
4. CFG simplification
5. Phi simplification
6. Common subexpression elimination
7. Global value numbering
8. Sparse conditional constant propagation
9. Loop invariant code motion
10. Temporal optimization passes

---

## Verification

- [x] Verify every CFG block has valid SSA dominance.
- [x] Verify phi nodes only contain predecessor values.
- [x] Verify structured temporal constructs lower correctly.
- [x] Verify `Split`/`Merge` branch consistency.
- [ ] Verify entropy state transitions.
- [x] Verify lease lifetimes.

---

# Things That Already Look Good So Far ;)

- Proper SSA with phi nodes.
- Explicit CFG.
- Semantic temporal instructions.
- Entropy-aware IR.
- Lease-aware IR.
- First-class channels.
- Explicit concurrency (`Split`, `Merge`).
- `RelativisticBlock` abstraction.
- `Consume` as a semantic operation.
- `MatchEntropy` lowering.
- Time-aware constructs preserved in IR.
- Domain semantics remain visible instead of disappearing into runtime calls.
