// causm-kernel/src/proof/Proof.Causm.TypeSafety.fst
module Proof.Causm.TypeSafety

open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.TypeSystem

/// Fundamental Soundness Lemma 1: Type-Readable implies State-Readable
val lemma_ty_readable_sound :
  env:type_env -> s:vm_state -> r:reg_id ->
  Lemma (requires (state_well_typed env s /\ is_ty_readable (env r) s.clock == true))
        (ensures (is_readable (s.regs r) s.clock == true))
let lemma_ty_readable_sound env s r = ()

/// Fundamental Soundness Lemma 2: Type-Consumable implies State-Consumable
val lemma_ty_consumable_sound :
  env:type_env -> s:vm_state -> r:reg_id ->
  Lemma (requires (state_well_typed env s /\ is_ty_consumable (env r) == true))
        (ensures (is_consumable (s.regs r) == true))
let lemma_ty_consumable_sound env s r = ()

/// Fundamental Soundness Theorem 1: PROGRESS
/// If a state is well-typed and an instruction is well-formed under the static environment,
/// the operational semantics NEVER get stuck (eval_step always returns Some s').
val theorem_progress :
  env:type_env -> s:vm_state -> i:instr ->
  Lemma (requires (state_well_typed env s /\ wf_instruction env s.clock i == true))
        (ensures (Some? (eval_step s i)))
let theorem_progress env s i =
  match i with
  | ILoadInt _ _ -> ()
  | IAdd dest s1 s2 ->
      lemma_ty_readable_sound env s s1;
      lemma_ty_readable_sound env s s2
  | IConsume target ->
      lemma_ty_consumable_sound env s target
  | ILease target duration ->
      lemma_ty_consumable_sound env s target
  | ITick delta -> ()
  | IEntangle r1 r2 ->
      lemma_ty_consumable_sound env s r1;
      lemma_ty_consumable_sound env s r2

/// Fundamental Soundness Theorem 2: PRESERVATION / SUBJECT REDUCTION
/// Evaluating an instruction transitions the concrete state such that it conforms
/// to the statically updated type environment.
val theorem_preservation :
  env:type_env -> s:vm_state -> i:instr -> s':vm_state ->
  Lemma (requires (state_well_typed env s /\
                   wf_instruction env s.clock i == true /\
                   eval_step s i == Some s'))
        (ensures (state_well_typed (type_step env s.clock s.entangled i) s'))
let theorem_preservation env s i s' =
  let env' = type_step env s.clock s.entangled i in
  match i with
  | ILoadInt dest v -> ()
  | IAdd dest s1 s2 -> ()
  | IConsume target -> ()
  | ILease target duration -> ()
  | ITick delta -> ()
  | IEntangle r1 r2 -> ()
