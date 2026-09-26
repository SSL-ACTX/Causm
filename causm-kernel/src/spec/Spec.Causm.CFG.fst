// causm-kernel/src/spec/Spec.Causm.CFG.fst
module Spec.Causm.CFG

open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.TypeSystem

type block_id = nat

type terminator =
  | TermReturn : ret_reg:reg_id -> terminator
  | TermJump   : target:block_id -> terminator
  | TermBranch : cond_reg:reg_id -> then_bb:block_id -> else_bb:block_id -> terminator

noeq type basic_block = {
  id    : block_id;
  body  : list instr;
  term  : terminator;
}

/// A CFG is a map from block_id to basic_block
type cfg = block_id -> option basic_block

/// CFG Machine Execution State
noeq type cfg_state = {
  vm       : vm_state;
  cur_bb   : block_id;
  pc       : nat; // index into basic_block body
  halted   : bool;
  exit_val : option int;
}

val get_bb : cfg -> block_id -> option basic_block
let get_bb g bid = g bid

/// Evaluate terminator
val eval_terminator : cfg_state -> terminator -> option cfg_state
let eval_terminator s term =
  match term with
  | TermReturn ret_reg ->
      if is_readable (s.vm.regs ret_reg) s.vm.clock then
        let v_opt =
          match s.vm.regs ret_reg with
          | StValid v -> Some v
          | StLeased v _ -> Some v
          | _ -> None
        in
        Some ({ s with halted = true; exit_val = v_opt })
      else
        None

  | TermJump target ->
      Some ({ s with cur_bb = target; pc = 0 })

  | TermBranch cond_reg then_bb else_bb ->
      if is_readable (s.vm.regs cond_reg) s.vm.clock then
        let cond_is_zero =
          match s.vm.regs cond_reg with
          | StValid v -> v = 0
          | StLeased v _ -> v = 0
          | _ -> false
        in
        let next_bb = if cond_is_zero then else_bb else then_bb in
        Some ({ s with cur_bb = next_bb; pc = 0 })
      else
        None

/// Single step of the CFG interpreter
val cfg_step : cfg -> cfg_state -> option cfg_state
let cfg_step g s =
  if s.halted then None
  else
    match get_bb g s.cur_bb with
    | None -> None
    | Some bb ->
        if s.pc < List.Tot.length bb.body then
          let i = List.Tot.index bb.body s.pc in
          match eval_step s.vm i with
          | None -> None
          | Some new_vm ->
              Some ({ s with vm = new_vm; pc = s.pc + 1 })
        else if s.pc = List.Tot.length bb.body then
          eval_terminator s bb.term
        else
          None

/// Multi-step execution of CFG program with fuel
val cfg_steps : cfg -> cfg_state -> fuel:nat -> Tot (option cfg_state) (decreases fuel)
let rec cfg_steps g s fuel =
  if s.halted then Some s
  else if fuel = 0 then None
  else
    match cfg_step g s with
    | None -> None
    | Some s' -> cfg_steps g s' (fuel - 1)

/// Sequential evaluation of an entire instruction list (basic block body)
val eval_body : s:vm_state -> body:list instr -> Tot (option vm_state) (decreases body)
let rec eval_body s body =
  match body with
  | [] -> Some s
  | i :: rest ->
      match eval_step s i with
      | None -> None
      | Some s' -> eval_body s' rest




