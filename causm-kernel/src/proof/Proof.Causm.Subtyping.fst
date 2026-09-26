// causm-kernel/src/proof/Proof.Causm.Subtyping.fst
module Proof.Causm.Subtyping

open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
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

/// Environment Subtyping Soundness
val lemma_subtyping_sound : env1:type_env -> env2:type_env -> s:vm_state ->
  Lemma (requires (state_well_typed env1 s /\ (forall r. reg_type_leq (env2 r) (env1 r) == true)))
        (ensures (state_well_typed env2 s))
let lemma_subtyping_sound env1 env2 s = ()

/// Environment Meet Soundness across Converging Branches
val lemma_env_meet_sound : env1:type_env -> env2:type_env -> s:vm_state ->
  Lemma (requires (state_well_typed env1 s \/ state_well_typed env2 s))
        (ensures (state_well_typed (env_meet env1 env2) s))
let lemma_env_meet_sound env1 env2 s = ()

/// Hierarchical Sub-Leasing Invariant:
/// A lease with shorter expiration e1 is a sound subtype of longer expiration e2 (e1 <= e2).
val lemma_leased_subtyping_monotonic : e1:nat -> e2:nat ->
  Lemma (requires (e1 <= e2))
        (ensures (reg_type_leq (TyLeased e1) (TyLeased e2) == true))
let lemma_leased_subtyping_monotonic e1 e2 = ()

/// Sub-Lease Conformance Preservation:
/// If a concrete cell conforms to TyLeased e2, and e1 <= e2, it soundly conforms to TyLeased e1.
val lemma_leased_conformance_tighter : e1:nat -> e2:nat -> st:entropic_state int ->
  Lemma (requires (reg_conforms (TyLeased e2) st /\ e1 <= e2))
        (ensures (reg_conforms (TyLeased e1) st))
let lemma_leased_conformance_tighter e1 e2 st = ()
