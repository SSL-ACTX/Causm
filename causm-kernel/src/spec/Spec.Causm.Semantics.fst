// causm-kernel/src/spec/Spec.Causm.Semantics.fst
module Spec.Causm.Semantics

open Spec.Causm.Lattice
open Spec.Causm.Timeline

type reg_id = nat

type instr =
  | ILoadInt : dest:reg_id -> value:int -> instr
  | IAdd     : dest:reg_id -> src1:reg_id -> src2:reg_id -> instr
  | IConsume : target:reg_id -> instr
  | ILease   : target:reg_id -> duration:nat -> instr
  | ITick    : delta:nat -> instr

type reg_file = reg_id -> entropic_state int

noeq type vm_state = {
  regs  : reg_file;
  clock : nat;
}

val empty_regs : reg_file
let empty_regs = fun _ -> StConsumed

val update_reg : reg_file -> reg_id -> entropic_state int -> reg_file
let update_reg r id st =
  fun (x: reg_id) -> if x = id then st else r x

val eval_step : vm_state -> instr -> option vm_state
let eval_step s i =
  match i with
  | ILoadInt dest v ->
      let new_regs = update_reg s.regs dest (StValid v) in
      Some ({ s with regs = new_regs })

  | IAdd dest s1 s2 ->
      if is_readable (s.regs s1) s.clock && is_readable (s.regs s2) s.clock then
        let get_val (st: entropic_state int) : option int =
          match st with
          | StValid v -> Some v
          | StLeased v _ -> Some v
          | _ -> None
        in
        match get_val (s.regs s1), get_val (s.regs s2) with
        | Some v1, Some v2 ->
            let sum = v1 + v2 in
            let new_regs = update_reg s.regs dest (StValid sum) in
            Some ({ s with regs = new_regs })
        | _ -> None
      else
        None

  | IConsume target ->
      if is_consumable (s.regs target) then
        let new_regs = update_reg s.regs target StConsumed in
        Some ({ s with regs = new_regs })
      else
        None

  | ILease target duration ->
      if is_consumable (s.regs target) then
        match s.regs target with
        | StValid v ->
            let exp = s.clock + duration in
            let new_regs = update_reg s.regs target (StLeased v exp) in
            Some ({ s with regs = new_regs })
        | _ -> None
      else
        None

  | ITick delta ->
      let new_clk = s.clock + delta in
      // Transition all expired leased registers to decayed
      let updated_regs = fun (x: reg_id) ->
        match s.regs x with
        | StLeased v exp ->
            if new_clk >= exp then StDecayed else StLeased v exp
        | other -> other
      in
      Some ({ regs = updated_regs; clock = new_clk })
