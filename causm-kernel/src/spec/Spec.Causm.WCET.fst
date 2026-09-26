// causm-kernel/src/spec/Spec.Causm.WCET.fst
module Spec.Causm.WCET

open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.CFG

/// Abstract cost model: Each instruction takes a strictly positive number of cycles (pos)
val instr_cost : instr -> pos
let instr_cost i =
  match i with
  | ILoadInt _ _ -> 1
  | IAdd _ _ _ -> 1
  | IConsume _ -> 2
  | ILease _ _ -> 2
  | ITick delta -> 1 + delta
  | IEntangle _ _ -> 3

/// Terminator cost in machine cycles
val term_cost : terminator -> pos
let term_cost t =
  match t with
  | TermReturn _ -> 2
  | TermJump _ -> 1
  | TermBranch _ _ _ -> 2

/// Body WCET: sum of costs of all instructions in a basic block body
val body_wcet : list instr -> nat
let rec body_wcet instrs =
  match instrs with
  | [] -> 0
  | i :: rest -> instr_cost i + body_wcet rest

/// Basic Block WCET: body cost + terminator cost
val block_wcet : basic_block -> pos
let block_wcet bb =
  body_wcet bb.body + term_cost bb.term

/// Loop execution bound model: bounded iteration count * single iteration WCET
val loop_wcet : iter_bound:nat -> body_cost:nat -> nat
let loop_wcet iter_bound body_cost =
  iter_bound * body_cost

/// Dynamic isochronous execution state with tracked cycle budget
noeq type wcet_tracked_state = {
  state     : cfg_state;
  consumed  : nat;
  max_limit : nat;
}

val is_within_limit : wcet_tracked_state -> bool
let is_within_limit s = s.consumed <= s.max_limit

/// Dynamic single-step cost for a CFG state
val cfg_step_cost : cfg -> cfg_state -> option pos
let cfg_step_cost g s =
  if s.halted then None
  else
    match get_bb g s.cur_bb with
    | None -> None
    | Some bb ->
        if s.pc < List.Tot.length bb.body then
          Some (instr_cost (List.Tot.index bb.body s.pc))
        else if s.pc = List.Tot.length bb.body then
          Some (term_cost bb.term)
        else
          None

/// Single step of WCET tracked execution
val wcet_step : cfg -> wcet_tracked_state -> option wcet_tracked_state
let wcet_step g s =
  match cfg_step_cost g s.state with
  | None -> None
  | Some cost ->
      if s.consumed + cost <= s.max_limit then
        match cfg_step g s.state with
        | None -> None
        | Some new_st ->
            Some ({
              state = new_st;
              consumed = s.consumed + cost;
              max_limit = s.max_limit;
            })
      else
        None

/// Headroom Invariant: Cycles remaining in current isochronous budget
val wcet_headroom : wcet_tracked_state -> nat
let wcet_headroom s =
  if s.consumed <= s.max_limit then s.max_limit - s.consumed else 0

/// Hard Real-Time Isochronous Pace Invariant:
/// Pads step consumption to target fixed pace tau, eliminating execution jitter.
val isochronous_pace_step :
  g:cfg -> s:wcet_tracked_state -> target_pace:pos ->
  option wcet_tracked_state
let isochronous_pace_step g s target_pace =
  match cfg_step_cost g s.state with
  | None -> None
  | Some cost ->
      if cost <= target_pace && s.consumed + target_pace <= s.max_limit then
        match cfg_step g s.state with
        | None -> None
        | Some new_st ->
            Some ({
              state = new_st;
              consumed = s.consumed + target_pace;
              max_limit = s.max_limit;
            })
      else
        None
