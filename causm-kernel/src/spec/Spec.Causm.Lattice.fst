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

/// Tag Partial Order:
/// Consumed is bottom (most entropic), Valid is top (least entropic).
/// Consumed <= Decayed <= Leased <= Valid
val tag_leq : entropic_tag -> entropic_tag -> bool
let tag_leq t1 t2 =
  match t1, t2 with
  | TagConsumed, _ -> true
  | TagDecayed, TagConsumed -> false
  | TagDecayed, _ -> true
  | TagLeased, TagValid -> true
  | TagLeased, TagLeased -> true
  | TagLeased, _ -> false
  | TagValid, TagValid -> true
  | TagValid, _ -> false

/// Lattice Meet Operator (Infimum / Greatest Lower Bound):
/// Computes the safe converging entropic tag across CFG branch merges.
val tag_meet : entropic_tag -> entropic_tag -> entropic_tag
let tag_meet t1 t2 =
  match t1, t2 with
  | TagConsumed, _ | _, TagConsumed -> TagConsumed
  | TagDecayed, _  | _, TagDecayed  -> TagDecayed
  | TagLeased, TagValid | TagValid, TagLeased -> TagLeased
  | TagLeased, TagLeased -> TagLeased
  | TagValid, TagValid -> TagValid

/// Lattice Law 1: Meet Commutativity
val lemma_meet_comm : t1:entropic_tag -> t2:entropic_tag ->
  Lemma (tag_meet t1 t2 == tag_meet t2 t1)
let lemma_meet_comm t1 t2 = ()

/// Lattice Law 2: Meet Idempotence
val lemma_meet_idem : t:entropic_tag ->
  Lemma (tag_meet t t == t)
let lemma_meet_idem t = ()

/// Lattice Law 3: Meet Associativity
val lemma_meet_assoc : t1:entropic_tag -> t2:entropic_tag -> t3:entropic_tag ->
  Lemma (tag_meet (tag_meet t1 t2) t3 == tag_meet t1 (tag_meet t2 t3))
let lemma_meet_assoc t1 t2 t3 = ()

/// Lattice Law 4: Meet is a Lower Bound
val lemma_meet_lb : t1:entropic_tag -> t2:entropic_tag ->
  Lemma (tag_leq (tag_meet t1 t2) t1 == true /\ tag_leq (tag_meet t1 t2) t2 == true)
let lemma_meet_lb t1 t2 = ()

/// Partial Order Law 1: Reflexivity
val lemma_tag_refl : t:entropic_tag ->
  Lemma (tag_leq t t == true)
let lemma_tag_refl t = ()

/// Partial Order Law 2: Transitivity
val lemma_tag_trans : t1:entropic_tag -> t2:entropic_tag -> t3:entropic_tag ->
  Lemma (requires (tag_leq t1 t2 == true /\ tag_leq t2 t3 == true))
        (ensures (tag_leq t1 t3 == true))
let lemma_tag_trans t1 t2 t3 = ()

/// Partial Order Law 3: Antisymmetry
val lemma_tag_antisym : t1:entropic_tag -> t2:entropic_tag ->
  Lemma (requires (tag_leq t1 t2 == true /\ tag_leq t2 t1 == true))
        (ensures (t1 == t2))
let lemma_tag_antisym t1 t2 = ()

/// Lattice Law 5: Greatest Lower Bound (GLB / Infimum Property)
/// For all lower bounds k of t1 and t2, k <= meet(t1, t2).
val lemma_meet_glb : k:entropic_tag -> t1:entropic_tag -> t2:entropic_tag ->
  Lemma (requires (tag_leq k t1 == true /\ tag_leq k t2 == true))
        (ensures (tag_leq k (tag_meet t1 t2) == true))
let lemma_meet_glb k t1 t2 = ()

/// Lattice Law 6: Meet Monotonicity
val lemma_meet_monotonic : a:entropic_tag -> b:entropic_tag -> c:entropic_tag -> d:entropic_tag ->
  Lemma (requires (tag_leq a b == true /\ tag_leq c d == true))
        (ensures (tag_leq (tag_meet a c) (tag_meet b d) == true))
let lemma_meet_monotonic a b c d = ()

/// Bounded Lattice Primitives:
/// Top is TagValid, Bottom is TagConsumed
val tag_top : entropic_tag
let tag_top = TagValid

val tag_bottom : entropic_tag
let tag_bottom = TagConsumed

/// Universal Bottom Law: bottom <= t for all t
val lemma_bottom_is_min : t:entropic_tag ->
  Lemma (tag_leq tag_bottom t == true)
let lemma_bottom_is_min t = ()

/// Universal Top Law: t <= top for all t
val lemma_top_is_max : t:entropic_tag ->
  Lemma (tag_leq t tag_top == true)
let lemma_top_is_max t = ()

/// Meet Annihilator: meet(t, bottom) = bottom
val lemma_meet_bottom_annihilates : t:entropic_tag ->
  Lemma (tag_meet t tag_bottom == tag_bottom)
let lemma_meet_bottom_annihilates t = ()

/// Meet Identity: meet(t, top) = t
val lemma_meet_top_identity : t:entropic_tag ->
  Lemma (tag_meet t tag_top == t)
let lemma_meet_top_identity t = ()

/// Lattice Join Operator (Supremum / Least Upper Bound):
val tag_join : entropic_tag -> entropic_tag -> entropic_tag
let tag_join t1 t2 =
  match t1, t2 with
  | TagValid, _ | _, TagValid -> TagValid
  | TagLeased, _ | _, TagLeased -> TagLeased
  | TagDecayed, _ | _, TagDecayed -> TagDecayed
  | TagConsumed, TagConsumed -> TagConsumed

/// Join Commutativity
val lemma_join_comm : t1:entropic_tag -> t2:entropic_tag ->
  Lemma (tag_join t1 t2 == tag_join t2 t1)
let lemma_join_comm t1 t2 = ()

/// Join Idempotence
val lemma_join_idem : t:entropic_tag ->
  Lemma (tag_join t t == t)
let lemma_join_idem t = ()

/// Join Associativity
val lemma_join_assoc : t1:entropic_tag -> t2:entropic_tag -> t3:entropic_tag ->
  Lemma (tag_join (tag_join t1 t2) t3 == tag_join t1 (tag_join t2 t3))
let lemma_join_assoc t1 t2 t3 = ()

/// Join Upper Bound: t1 <= join(t1, t2) /\ t2 <= join(t1, t2)
val lemma_join_ub : t1:entropic_tag -> t2:entropic_tag ->
  Lemma (tag_leq t1 (tag_join t1 t2) == true /\ tag_leq t2 (tag_join t1 t2) == true)
let lemma_join_ub t1 t2 = ()

/// Least Upper Bound (LUB Property):
val lemma_join_lub : k:entropic_tag -> t1:entropic_tag -> t2:entropic_tag ->
  Lemma (requires (tag_leq t1 k == true /\ tag_leq t2 k == true))
        (ensures (tag_leq (tag_join t1 t2) k == true))
let lemma_join_lub k t1 t2 = ()

/// Absorption Law 1: meet(a, join(a, b)) = a
val lemma_lattice_absorb_meet : a:entropic_tag -> b:entropic_tag ->
  Lemma (tag_meet a (tag_join a b) == a)
let lemma_lattice_absorb_meet a b = ()

/// Absorption Law 2: join(a, meet(a, b)) = a
val lemma_lattice_absorb_join : a:entropic_tag -> b:entropic_tag ->
  Lemma (tag_join a (tag_meet a b) == a)
let lemma_lattice_absorb_join a b = ()

