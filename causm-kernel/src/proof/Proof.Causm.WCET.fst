// causm-kernel/src/proof/Proof.Causm.WCET.fst
module Proof.Causm.WCET

open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.CFG
open Spec.Causm.WCET
open FStar.List.Tot

/// Theorem 1: Instruction Cost Positivity
/// Every instruction has a strictly positive execution cost (cost >= 1).
val lemma_instr_cost_positive : i:instr ->
  Lemma (instr_cost i >= 1)
let lemma_instr_cost_positive i = ()

/// Theorem 2: Block WCET Lower Bound
/// The WCET of any basic block is strictly positive, bounded from below by terminator cost.
val lemma_block_wcet_ge_term : bb:basic_block ->
  Lemma (block_wcet bb >= term_cost bb.term /\ block_wcet bb >= 1)
let lemma_block_wcet_ge_term bb = ()

/// Theorem 3: Body WCET Additivity over List Concatenation
val lemma_body_wcet_append : l1:list instr -> l2:list instr ->
  Lemma (body_wcet (l1 @ l2) == body_wcet l1 + body_wcet l2)
let rec lemma_body_wcet_append l1 l2 =
  match l1 with
  | [] -> ()
  | _ :: rest -> lemma_body_wcet_append rest l2

/// Theorem 4: Bounded Loop WCET Monotonicity
/// If loop body cost is bounded by B, and iterations by N, total loop WCET <= N * B.
val lemma_loop_wcet_bound : iter:nat -> n:nat -> b1:nat -> b2:nat ->
  Lemma (requires (iter <= n /\ b1 <= b2))
        (ensures (loop_wcet iter b1 <= loop_wcet n b2))
let lemma_loop_wcet_bound iter n b1 b2 = ()

/// Theorem 5: Isochronous Budget Monotonicity
/// Advancing execution by an instruction strictly increases consumed cycles by instr_cost i.
val lemma_step_cost_monotonic : s:wcet_tracked_state -> i:instr ->
  Lemma (requires (s.consumed + instr_cost i <= s.max_limit))
        (ensures (s.consumed < s.consumed + instr_cost i /\
                  s.consumed + instr_cost i <= s.max_limit))
let lemma_step_cost_monotonic s i = ()

/// Theorem 6: Dynamic WCET Step Preserves Isochronous Bound
/// Executing a step with wcet_step unconditionally guarantees the resulting state remains within the max cycle limit.
val theorem_wcet_step_within_limit :
  g:cfg -> s:wcet_tracked_state -> s':wcet_tracked_state ->
  Lemma (requires (wcet_step g s == Some s'))
        (ensures (is_within_limit s' == true /\ s'.consumed > s.consumed))
let theorem_wcet_step_within_limit g s s' = ()

/// Theorem 7: Bounded Multi-Step Execution Consumption Bound
/// For any sequence of steps, total cycle consumption strictly tracks the sum of step costs.
val lemma_wcet_step_accumulates :
  g:cfg -> s:wcet_tracked_state -> s':wcet_tracked_state -> cost:pos ->
  Lemma (requires (cfg_step_cost g s.state == Some cost /\
                   wcet_step g s == Some s'))
        (ensures (s'.consumed == s.consumed + cost))
let lemma_wcet_step_accumulates g s s' cost = ()
