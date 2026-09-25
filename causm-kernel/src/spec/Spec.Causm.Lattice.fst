// causm-kernel/src/spec/Spec.Causm.Lattice.fst
module Spec.Causm.Lattice

type entropic_tag =
  | TagValid
  | TagLeased
  | TagDecayed
  | TagConsumed

type entropic_state (a: Type0) =
  | StValid    : v:a -> entropic_state a
  | StLeased   : v:a -> expire_tick:nat -> entropic_state a
  | StDecayed  : entropic_state a
  | StConsumed : entropic_state a

val tag_of : #a:Type0 -> entropic_state a -> entropic_tag
let tag_of #a st =
  match st with
  | StValid _    -> TagValid
  | StLeased _ _ -> TagLeased
  | StDecayed    -> TagDecayed
  | StConsumed   -> TagConsumed

val is_readable : #a:Type0 -> entropic_state a -> nat -> bool
let is_readable #a st current_clock =
  match st with
  | StValid _ -> true
  | StLeased _ exp -> current_clock < exp
  | StDecayed -> false
  | StConsumed -> false

val is_consumable : #a:Type0 -> entropic_state a -> bool
let is_consumable #a st =
  match st with
  | StValid _ -> true
  | _ -> false
