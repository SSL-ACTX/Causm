// causm-kernel/src/proof/Proof.Causm.Soundness.fst
module Proof.Causm.Soundness

open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics

/// Theorem 1: Preservation of Non-Readability for Consumed Registers
/// If a register was consumed, and an instruction evaluates, the register remains
/// unreadable unless it was the explicit destination of a fresh value load.
val lemma_consumed_persists_under_eval :
  s:vm_state -> i:instr -> s':vm_state -> r:reg_id ->
  Lemma (requires (eval_step s i == Some s' /\
                   tag_of (s.regs r) == TagConsumed /\
                   (match i with
                    | ILoadInt dest _ -> dest <> r
                    | IAdd dest _ _   -> dest <> r
                    | _               -> True)))
        (ensures (is_readable (s'.regs r) s'.clock == false))
let lemma_consumed_persists_under_eval s i s' r =
  match i with
  | ILoadInt dest _ -> ()
  | IAdd dest s1 s2 -> ()
  | IConsume target -> ()
  | IEntangle r1 r2 -> ()
  | ILease target duration -> ()
  | ITick delta -> ()

/// Theorem 2: Clock Progression Monotonicity
/// Any evaluation step strictly monotonically advances or preserves the clock.
val lemma_clock_monotonic :
  s:vm_state -> i:instr -> s':vm_state ->
  Lemma (requires eval_step s i == Some s')
        (ensures s'.clock >= s.clock)
let lemma_clock_monotonic s i s' =
  match i with
  | ITick delta -> ()
  | _ -> ()

/// Theorem 3: Leased Expiration Inductive Invariant
/// When clock advances past a lease expiration tick, reading that register is strictly impossible.
val lemma_expired_lease_unreadable_after_tick :
  s:vm_state -> delta:nat -> s':vm_state -> r:reg_id -> v:int -> exp:nat ->
  Lemma (requires (s.regs r == StLeased v exp /\
                   eval_step s (ITick delta) == Some s' /\
                   s.clock + delta >= exp))
        (ensures (is_readable (s'.regs r) s'.clock == false))
let lemma_expired_lease_unreadable_after_tick s delta s' r v exp = ()

/// Theorem 4: Double-Consume Safety
/// Attempting to consume an already-consumed or decayed register fails safely (eval_step returns None).
val lemma_double_consume_rejected :
  s:vm_state -> r:reg_id ->
  Lemma (requires tag_of (s.regs r) == TagConsumed \/ tag_of (s.regs r) == TagDecayed)
        (ensures eval_step s (IConsume r) == None)
let lemma_double_consume_rejected s r = ()

/// Theorem 5: Entanglement Consumption Invariant
/// Consuming target register r1 strictly forces all entangled registers r2 into StConsumed state,
/// rendering them immediately unreadable.
val lemma_entangled_consume_propagates :
  s:vm_state -> r1:reg_id -> r2:reg_id -> s':vm_state ->
  Lemma (requires (s.entangled r1 r2 == true /\
                   eval_step s (IConsume r1) == Some s'))
        (ensures (s'.regs r2 == StConsumed /\ is_readable (s'.regs r2) s'.clock == false))
let lemma_entangled_consume_propagates s r1 r2 s' = ()

