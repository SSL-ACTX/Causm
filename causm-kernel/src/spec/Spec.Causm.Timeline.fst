// causm-kernel/src/spec/Spec.Causm.Timeline.fst
module Spec.Causm.Timeline

type time_coordinate =
  | TimeGlobal   : tick:nat -> time_coordinate
  | TimeRelative : delta:nat -> time_coordinate
  | TimePeriodic : period:nat -> time_coordinate

type isochronous_budget = {
  wcet_limit : nat;
  elapsed    : nat;
}

val budget_ok : isochronous_budget -> bool
let budget_ok b = b.elapsed <= b.wcet_limit

val advance_clock : isochronous_budget -> nat -> isochronous_budget
let advance_clock b cost =
  { b with elapsed = b.elapsed + cost }
