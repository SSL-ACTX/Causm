// causm-kernel/src/spec/Spec.Causm.TypeSystem.fst
module Spec.Causm.TypeSystem

open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics

/// Abstract static type for a register
type reg_type =
  | TyValid    : reg_type
  | TyLeased   : expire_tick:nat -> reg_type
  | TyDecayed  : reg_type
  | TyConsumed : reg_type

/// Reg-Type Subtyping Partial Order:
/// TyConsumed <= TyDecayed <= TyLeased(e1) <= TyLeased(e2) [if e1 <= e2] <= TyValid
val reg_type_leq : reg_type -> reg_type -> bool
let reg_type_leq t1 t2 =
  match t1, t2 with
  | TyConsumed, _ -> true
  | TyDecayed, TyConsumed -> false
  | TyDecayed, _ -> true
  | TyLeased e1, TyLeased e2 -> e1 <= e2
  | TyLeased _, TyValid -> true
  | TyLeased _, _ -> false
  | TyValid, TyValid -> true
  | TyValid, _ -> false

/// Meet operator for reg_type:
/// Takes the greatest lower bound of static types across branch convergence.
val reg_type_meet : reg_type -> reg_type -> reg_type
let reg_type_meet t1 t2 =
  match t1, t2 with
  | TyConsumed, _ | _, TyConsumed -> TyConsumed
  | TyDecayed, _  | _, TyDecayed  -> TyDecayed
  | TyLeased e1, TyLeased e2 -> TyLeased (if e1 <= e2 then e1 else e2)
  | TyLeased e, TyValid | TyValid, TyLeased e -> TyLeased e
  | TyValid, TyValid -> TyValid

/// Static type environment mapping registers to their abstract entropic types
type type_env = reg_id -> reg_type

/// Environment meet across incoming CFG edges
val env_meet : type_env -> type_env -> type_env
let env_meet e1 e2 =
  fun r -> reg_type_meet (e1 r) (e2 r)


/// Typing judgement: when does a concrete runtime state conform to the static type environment?
let state_well_typed (env: type_env) (s: vm_state) : prop =
  forall (r: reg_id).
    match env r, s.regs r with
    | TyValid, StValid _ -> True
    | TyLeased exp, StLeased _ e -> exp == e
    | TyDecayed, StDecayed -> True
    | TyConsumed, StConsumed -> True
    | _, _ -> False

/// Static check: when is a register readable according to the static type at a given clock?
let is_ty_readable (ty: reg_type) (clk: nat) : bool =
  match ty with
  | TyValid -> true
  | TyLeased exp -> clk < exp
  | _ -> false

/// Static check: when is a register consumable according to its static type?
let is_ty_consumable (ty: reg_type) : bool =
  match ty with
  | TyValid -> true
  | _ -> false

/// Static well-formedness of an instruction with respect to the static environment and clock
let wf_instruction (env: type_env) (clk: nat) (i: instr) : bool =
  match i with
  | ILoadInt _ _ -> true
  | IAdd _ s1 s2 ->
      is_ty_readable (env s1) clk && is_ty_readable (env s2) clk
  | IConsume target ->
      is_ty_consumable (env target)
  | ILease target _ ->
      is_ty_consumable (env target)
  | ITick _ -> true
  | IEntangle r1 r2 ->
      r1 <> r2 && is_ty_consumable (env r1) && is_ty_consumable (env r2)

/// Static update of the type environment upon evaluating an instruction
let type_step (env: type_env) (clk: nat) (ent: entangle_rel) (i: instr) : type_env =
  match i with
  | ILoadInt dest _ ->
      fun r -> if r = dest then TyValid else env r
  | IAdd dest _ _ ->
      fun r -> if r = dest then TyValid else env r
  | IConsume target ->
      fun r -> if r = target || ent target r then TyConsumed else env r
  | ILease target duration ->
      fun r -> if r = target then TyLeased (clk + duration) else env r
  | ITick delta ->
      let new_clk = clk + delta in
      fun r ->
        (match env r with
         | TyLeased exp -> if new_clk >= exp then TyDecayed else TyLeased exp
         | other -> other)
  | IEntangle _ _ -> env
