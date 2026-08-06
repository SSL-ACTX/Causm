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

- [ ] Normalize all structured control flow.
  - Ensure loops always become:
    - header
    - body
    - backedge
    - exit

- [ ] Reduce embedded control-flow instructions inside IR nodes.

---

## Temporal Analysis

- [ ] Keep temporal instructions visible for later optimization.
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

- [ ] Entropy propagation analysis.
- [ ] Dead entropy-state elimination.
- [ ] Detect redundant `MatchEntropy`.

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

- [ ] Channel liveness analysis.
- [ ] Dead channel elimination.
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

- [ ] Branch lifetime analysis.
- [ ] Merge correctness verification.
- [ ] Parallel dependency analysis.
- [ ] Race detection (if mutable state exists).

---

## Leases

Current implementation looks solid.

Potential additions:

- [ ] Nested lease optimization.
- [ ] Lease lifetime verification.
- [ ] Lease escape analysis.
- [ ] Automatic redundant lease removal.

---

## Pretty Printer

- [ ] Reduce SSA verbosity.
- [ ] Collapse trivial copy chains.
- [ ] Print constants inline when possible.
- [ ] Optional "optimized SSA" dump after cleanup passes.
- [ ] Optional CFG visualization.

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

- [ ] Verify every CFG block has valid SSA dominance.
- [ ] Verify phi nodes only contain predecessor values.
- [ ] Verify structured temporal constructs lower correctly.
- [ ] Verify `Split`/`Merge` branch consistency.
- [ ] Verify entropy state transitions.
- [ ] Verify lease lifetimes.

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
