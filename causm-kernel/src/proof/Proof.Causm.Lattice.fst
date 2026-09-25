// causm-kernel/src/proof/Proof.Causm.Lattice.fst
module Proof.Causm.Lattice

open Spec.Causm.Lattice

val lemma_consumed_not_readable : #a:Type0 -> st:entropic_state a -> clk:nat ->
  Lemma (requires tag_of st == TagConsumed)
        (ensures is_readable st clk == false)
let lemma_consumed_not_readable #a st clk = ()

val lemma_decayed_not_readable : #a:Type0 -> st:entropic_state a -> clk:nat ->
  Lemma (requires tag_of st == TagDecayed)
        (ensures is_readable st clk == false)
let lemma_decayed_not_readable #a st clk = ()

val lemma_consume_only_valid : #a:Type0 -> st:entropic_state a ->
  Lemma (requires is_consumable st == true)
        (ensures tag_of st == TagValid)
let lemma_consume_only_valid #a st = ()

open Spec.Causm.TypeSystem

/// reg_type Poset Law 1: Reflexivity
val lemma_reg_type_refl : t:reg_type ->
  Lemma (reg_type_leq t t == true)
let lemma_reg_type_refl t = ()

/// reg_type Poset Law 2: Transitivity
val lemma_reg_type_trans : t1:reg_type -> t2:reg_type -> t3:reg_type ->
  Lemma (requires (reg_type_leq t1 t2 == true /\ reg_type_leq t2 t3 == true))
        (ensures (reg_type_leq t1 t3 == true))
let lemma_reg_type_trans t1 t2 t3 = ()

/// reg_type Poset Law 3: Antisymmetry
val lemma_reg_type_antisym : t1:reg_type -> t2:reg_type ->
  Lemma (requires (reg_type_leq t1 t2 == true /\ reg_type_leq t2 t1 == true))
        (ensures (t1 == t2))
let lemma_reg_type_antisym t1 t2 = ()

/// reg_type Meet Commutativity
val lemma_reg_type_meet_comm : t1:reg_type -> t2:reg_type ->
  Lemma (reg_type_meet t1 t2 == reg_type_meet t2 t1)
let lemma_reg_type_meet_comm t1 t2 = ()

/// reg_type Meet Idempotence
val lemma_reg_type_meet_idem : t:reg_type ->
  Lemma (reg_type_meet t t == t)
let lemma_reg_type_meet_idem t = ()

/// reg_type Meet Associativity
val lemma_reg_type_meet_assoc : t1:reg_type -> t2:reg_type -> t3:reg_type ->
  Lemma (reg_type_meet (reg_type_meet t1 t2) t3 == reg_type_meet t1 (reg_type_meet t2 t3))
let lemma_reg_type_meet_assoc t1 t2 t3 = ()

/// reg_type Meet is Lower Bound
val lemma_reg_type_meet_lb : t1:reg_type -> t2:reg_type ->
  Lemma (reg_type_leq (reg_type_meet t1 t2) t1 == true /\ reg_type_leq (reg_type_meet t1 t2) t2 == true)
let lemma_reg_type_meet_lb t1 t2 = ()

/// reg_type Greatest Lower Bound (GLB)
val lemma_reg_type_meet_glb : k:reg_type -> t1:reg_type -> t2:reg_type ->
  Lemma (requires (reg_type_leq k t1 == true /\ reg_type_leq k t2 == true))
        (ensures (reg_type_leq k (reg_type_meet t1 t2) == true))
let lemma_reg_type_meet_glb k t1 t2 = ()
