// causm-kernel/src/pulse/Pulse.Causm.Arena.fst
module Pulse.Causm.Arena
#lang-pulse

open Pulse.Lib.Pervasives
open Pulse.Lib.Reference
open Pulse.Lib.Array
open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.TypeSystem
open Spec.Causm.WCET
module U32 = FStar.UInt32
module U64 = FStar.UInt64

type u32 = FStar.UInt32.t
type u64 = FStar.UInt64.t

type cell_t = {
  payload: u64;
  tag: u32; // 0: Valid, 1: Leased, 2: Decayed, 3: Consumed
  expire: u64;
}

/// Refinement Function: Low-level C machine integer tag to pure algebraic entropic_tag
noextract
let tag_to_spec (t: u32) : entropic_tag =
  if t = 0ul then TagValid
  else if t = 1ul then TagLeased
  else if t = 2ul then TagDecayed
  else TagConsumed

/// Refinement Function: Low-level C cell to pure algebraic entropic_state
noextract
let cell_to_state (c: cell_t) : entropic_state u64 =
  if c.tag = 0ul then StValid c.payload
  else if c.tag = 1ul then StLeased c.payload (U64.v c.expire)
  else if c.tag = 2ul then StDecayed
  else StConsumed

/// Refinement Function: Low-level C cell to abstract compiler reg_type in Spec.Causm.TypeSystem
noextract
let cell_to_reg_type (c: cell_t) : reg_type =
  if c.tag = 0ul then TyValid
  else if c.tag = 1ul then TyLeased (U64.v c.expire)
  else if c.tag = 2ul then TyDecayed
  else TyConsumed

let lemma_tag_meet_refinement (t1: u32) (t2: u32) (res: u32) : Lemma
  (requires (
    (t1 == 3ul \/ t2 == 3ul ==> res == 3ul) /\
    (t1 <> 3ul /\ t2 <> 3ul /\ (t1 == 2ul \/ t2 == 2ul) ==> res == 2ul) /\
    (t1 <> 3ul /\ t2 <> 3ul /\ t1 <> 2ul /\ t2 <> 2ul /\ (t1 == 1ul \/ t2 == 1ul) ==> res == 1ul) /\
    (t1 == 0ul /\ t2 == 0ul ==> res == 0ul)
  ))
  (ensures (
    (U32.v t1 <= 3 /\ U32.v t2 <= 3) ==> tag_to_spec res == Spec.Causm.Lattice.tag_meet (tag_to_spec t1) (tag_to_spec t2)
  ))
= ()

type arena_cell = ref cell_t

/// Invariant 1: Dereferencing cell payload requires Valid state
/// Formally proven: Cell is readable according to Spec.Causm.Lattice.is_readable
fn read_valid_cell
  (c: arena_cell)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 0ul /\ Spec.Causm.Lattice.is_readable (cell_to_state s) 0)
  returns  res: u64
  ensures  pts_to c s ** pure (res == s.payload /\ tag_to_spec s.tag == TagValid)
{
  let cell = !c;
  cell.payload
}

/// Invariant 2: Consumption destroys access right, transitioning cell to TagConsumed
/// Formally proven: Precondition satisfies is_consumable, postcondition yields TagConsumed
fn consume_cell
  (c: arena_cell)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 0ul /\ Spec.Causm.Lattice.is_consumable (cell_to_state s))
  ensures  exists* (s': cell_t). pts_to c s' ** pure (s'.tag == 3ul /\ tag_to_spec s'.tag == TagConsumed)
{
  let cell = !c;
  c := { payload = 0uL; tag = 3ul; expire = 0uL };
}

/// Invariant 3: Leased borrows transition to TagDecayed when clock advances
/// Formally proven: Postcondition yields TagDecayed
fn tick_cell_decay
  (c: arena_cell)
  (current_clk: u64)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 1ul /\ tag_to_spec s.tag == TagLeased /\ U64.gte current_clk s.expire)
  ensures  exists* (s': cell_t). pts_to c s' ** pure (s'.tag == 2ul /\ tag_to_spec s'.tag == TagDecayed)
{
  let cell = !c;
  c := { payload = 0uL; tag = 2ul; expire = cell.expire };
}

/// Invariant 4: Lease creation transitions Valid to Leased with specified expiration
/// Formally proven: Transitions TagValid to TagLeased
fn lease_cell
  (c: arena_cell)
  (current_clk: u64)
  (duration: u64)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 0ul /\ tag_to_spec s.tag == TagValid /\ FStar.UInt.size (U64.v current_clk + U64.v duration) 64)
  ensures  exists* (s': cell_t). pts_to c s' ** pure (s'.tag == 1ul /\ tag_to_spec s'.tag == TagLeased /\ U64.v s'.expire == U64.v current_clk + U64.v duration)
{
  let cell = !c;
  let new_expire = U64.add current_clk duration;
  c := { payload = cell.payload; tag = 1ul; expire = new_expire };
}

/// Invariant 5: Initializing a cell directly establishes a fresh Valid state
fn init_valid_cell
  (c: arena_cell)
  (v: u64)
  (#s: Ghost.erased cell_t)
  requires pts_to c s
  ensures  exists* (s': cell_t). pts_to c s' ** pure (s'.tag == 0ul /\ s'.payload == v)
{
  c := { payload = v; tag = 0ul; expire = 0uL };
}

module SZ = FStar.SizeT
module Seq = FStar.Seq

/// Array Invariant 1: Safe indexed read requiring valid cell or active non-expired lease
fn arena_read
  (a: array cell_t)
  (idx: SZ.t)
  (current_clk: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s /\
                               ((Seq.index s (SZ.v idx)).tag == 0ul \/
                                ((Seq.index s (SZ.v idx)).tag == 1ul /\ U64.lt current_clk (Seq.index s (SZ.v idx)).expire)))
  returns  res: u64
  ensures  pts_to a s ** pure (SZ.v idx < Seq.length s /\ res == (Seq.index s (SZ.v idx)).payload)
{
  pts_to_len a;
  let cell = a.(idx);
  cell.payload
}

/// Array Invariant 2: Safe indexed consume transitioning element to tag 3 (Consumed)
fn arena_consume
  (a: array cell_t)
  (idx: SZ.t)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to a s' **
             pure (SZ.v idx < Seq.length s /\ s' == Seq.upd s (SZ.v idx) { payload = 0uL; tag = 3ul; expire = 0uL } /\
                   tag_to_spec (Seq.index s' (SZ.v idx)).tag == TagConsumed)
{
  pts_to_len a;
  a.(idx) <- { payload = 0uL; tag = 3ul; expire = 0uL };
}

/// Array Invariant 3: Safe indexed lease establishing expiration timestamp
fn arena_lease
  (a: array cell_t)
  (idx: SZ.t)
  (current_clk: u64)
  (duration: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s /\
                               (Seq.index s (SZ.v idx)).tag == 0ul /\
                               FStar.UInt.size (U64.v current_clk + U64.v duration) 64)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to a s' **
             pure (Seq.length s' == Seq.length s /\
                   SZ.v idx < Seq.length s' /\
                   (Seq.index s' (SZ.v idx)).tag == 1ul /\
                   U64.v (Seq.index s' (SZ.v idx)).expire == U64.v current_clk + U64.v duration /\
                   (Seq.index s' (SZ.v idx)).payload == (Seq.index s (SZ.v idx)).payload)
{
  pts_to_len a;
  let cell = a.(idx);
  let new_expire = U64.add current_clk duration;
  a.(idx) <- { payload = cell.payload; tag = 1ul; expire = new_expire };
}

/// Array Invariant 4: Initialize a slot in the arena with a valid payload
fn arena_init_valid
  (a: array cell_t)
  (idx: SZ.t)
  (v: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to a s' **
             pure (SZ.v idx < Seq.length s /\ s' == Seq.upd s (SZ.v idx) { payload = v; tag = 0ul; expire = 0uL })
{
  pts_to_len a;
  a.(idx) <- { payload = v; tag = 0ul; expire = 0uL };
}

/// Array Invariant 5: Decay an expired leased cell at a specific index
fn arena_tick_decay
  (a: array cell_t)
  (idx: SZ.t)
  (current_clk: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s /\
                               (Seq.index s (SZ.v idx)).tag == 1ul /\
                               U64.gte current_clk (Seq.index s (SZ.v idx)).expire)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to a s' **
             pure (SZ.v idx < Seq.length s /\
                   s' == Seq.upd s (SZ.v idx) { payload = 0uL;
                                                tag = 2ul;
                                                expire = (Seq.index s (SZ.v idx)).expire })
{
  pts_to_len a;
  let cell = a.(idx);
  a.(idx) <- { payload = 0uL; tag = 2ul; expire = cell.expire };
}

/// Lattice Meet Primitives for CFG Branch Convergence:
/// Formally proven refinement: Low-level machine tag meet strictly refines algebraic Spec.Causm.Lattice.tag_meet
fn tag_meet_u32 (t1: u32) (t2: u32)
  requires pure (U32.v t1 <= 3 /\ U32.v t2 <= 3)
  returns res: u32
  ensures pure (
    tag_to_spec res == Spec.Causm.Lattice.tag_meet (tag_to_spec t1) (tag_to_spec t2) /\
    (t1 == 3ul \/ t2 == 3ul ==> res == 3ul) /\
    (t1 <> 3ul /\ t2 <> 3ul /\ (t1 == 2ul \/ t2 == 2ul) ==> res == 2ul) /\
    (t1 <> 3ul /\ t2 <> 3ul /\ t1 <> 2ul /\ t2 <> 2ul /\ (t1 == 1ul \/ t2 == 1ul) ==> res == 1ul) /\
    (t1 == 0ul /\ t2 == 0ul ==> res == 0ul)
  )
{
  let res =
    if (t1 = 3ul || t2 = 3ul) {
      3ul
    } else if (t1 = 2ul || t2 = 2ul) {
      2ul
    } else if (t1 = 1ul || t2 = 1ul) {
      1ul
    } else {
      0ul
    };
  lemma_tag_meet_refinement t1 t2 res;
  res
}

/// Array Invariant 6: Merge a register cell across two CFG branch predecessor outcomes
/// Statically converges to the lattice infimum (meet), ensuring sound conservative decay/consumption.
/// Formally proven: Resulting cell tag is the exact Spec.Causm.Lattice.tag_meet of predecessor tags.
fn arena_merge_meet
  (dst: array cell_t)
  (idx: SZ.t)
  (pred_tag: u32)
  (pred_expire: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to dst s ** pure (SZ.v idx < Seq.length s /\
                                 U32.v pred_tag <= 3 /\
                                 U32.v (Seq.index s (SZ.v idx)).tag <= 3)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to dst s' **
             pure (Seq.length s' == Seq.length s /\
                   SZ.v idx < Seq.length s' /\
                   (let cur = Seq.index s (SZ.v idx) in
                    let m = (Seq.index s' (SZ.v idx)).tag in
                    tag_to_spec m == Spec.Causm.Lattice.tag_meet (tag_to_spec cur.tag) (tag_to_spec pred_tag) /\
                    (cur.tag == 3ul \/ pred_tag == 3ul ==> m == 3ul) /\
                    (cur.tag == 2ul \/ pred_tag == 2ul ==> (m == 2ul \/ m == 3ul)) /\
                    (m == 3ul \/ m == 2ul ==> (Seq.index s' (SZ.v idx)).payload == 0uL)))
{
  pts_to_len dst;
  let cur = dst.(idx);
  let m = tag_meet_u32 cur.tag pred_tag;
  if (m = 3ul || m = 2ul) {
    let zero: u64 = 0uL;
    dst.(idx) <- { payload = zero; tag = m; expire = 0uL };
  } else {
    // If either was leased, pick the tighter (or incoming) expiration
    let final_expire =
      if (cur.tag = 1ul && pred_tag = 1ul) {
        if (U64.lte cur.expire pred_expire) { cur.expire } else { pred_expire }
      } else if (pred_tag = 1ul) {
        pred_expire
      } else {
        cur.expire
      };
    dst.(idx) <- { payload = cur.payload; tag = m; expire = final_expire };
  };
}

/// Invariant 7: Isochronous WCET Budget Tracker
/// Verifies dynamic step cost against WCET bound, returning false if budget would overflow.
fn wcet_budget_step
  (consumed: ref u64)
  (cost: u64)
  (max_limit: u64)
  (#cur: Ghost.erased u64)
  requires pts_to consumed cur
  returns  ok: bool
  ensures  exists* (new_cur: u64).
             pts_to consumed new_cur **
             pure (
               if (U64.v cur + U64.v cost <= U64.v max_limit /\
                   FStar.UInt.size (U64.v cur + U64.v cost) 64) then
                 ok == true /\ U64.v new_cur == U64.v cur + U64.v cost
               else
                 ok == false /\ new_cur == cur
             )
{
  let current_val = !consumed;
  if (U64.lte current_val max_limit) {
    let rem = U64.sub max_limit current_val;
    if (U64.lte cost rem) {
      let next_val = U64.add current_val cost;
      consumed := next_val;
      true
    } else {
      false
    }
  } else {
    false
  }
}

/// Invariant 8: Linear Entropic Access Verifier with Fine-Grained Diagnostics
/// Diagnostics: 0ul = Ok, 1ul = UseAfterConsume, 2ul = UseAfterDecay, 3ul = LeaseExpired
/// Formally proven: diag == 0ul if and only if cell is readable according to Spec.Causm.Lattice.is_readable
fn check_cell_access
  (a: array cell_t)
  (idx: SZ.t)
  (current_clk: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s)
  returns  diag: u32
  ensures  pts_to a s **
           pure (
             SZ.v idx < Seq.length s /\
             (let cell = Seq.index s (SZ.v idx) in
              (diag == 0ul <==> Spec.Causm.Lattice.is_readable (cell_to_state cell) (U64.v current_clk)) /\
              (cell.tag == 0ul ==> diag == 0ul) /\
              (cell.tag == 3ul ==> diag == 1ul /\ tag_to_spec cell.tag == TagConsumed) /\
              (cell.tag == 2ul ==> diag == 2ul /\ tag_to_spec cell.tag == TagDecayed) /\
              (cell.tag == 1ul /\ U64.lt current_clk cell.expire ==> diag == 0ul) /\
              (cell.tag == 1ul /\ U64.gte current_clk cell.expire ==> diag == 3ul))
           )
{
  pts_to_len a;
  let cell = a.(idx);
  if (cell.tag = 0ul) {
    0ul
  } else if (cell.tag = 3ul) {
    1ul
  } else if (cell.tag = 2ul) {
    2ul
  } else if (cell.tag = 1ul) {
    if (U64.lt current_clk cell.expire) {
      0ul
    } else {
      3ul
    }
  } else {
    1ul
  }
}

/// Invariant 9: Linear Consumption Safety Verifier
/// Diagnostics: 0ul = Ok, 4ul = DoubleConsume, 5ul = ConsumeDecayed, 6ul = ConsumeActiveLease
/// Formally proven: diag == 0ul if and only if cell is consumable according to Spec.Causm.Lattice.is_consumable
fn check_cell_consume
  (a: array cell_t)
  (idx: SZ.t)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s)
  returns  diag: u32
  ensures  pts_to a s **
           pure (
             SZ.v idx < Seq.length s /\
             (let cell = Seq.index s (SZ.v idx) in
              (diag == 0ul <==> Spec.Causm.Lattice.is_consumable (cell_to_state cell)) /\
              (cell.tag == 0ul ==> diag == 0ul) /\
              (cell.tag == 3ul ==> diag == 4ul /\ tag_to_spec cell.tag == TagConsumed) /\
              (cell.tag == 2ul ==> diag == 5ul /\ tag_to_spec cell.tag == TagDecayed) /\
              (cell.tag == 1ul ==> diag == 6ul /\ tag_to_spec cell.tag == TagLeased))
           )
{
  pts_to_len a;
  let cell = a.(idx);
  if (cell.tag = 0ul) {
    0ul
  } else if (cell.tag = 3ul) {
    4ul
  } else if (cell.tag = 2ul) {
    5ul
  } else {
    6ul
  }
}

/// Invariant 10: Hierarchical Sub-Lease Verifier
/// Diagnostics: 0ul = Ok, 7ul = CannotLeaseNonValid, 8ul = SubLeaseDurationOverflow
/// Formally proven: Lease requires TagValid (tag_to_spec cell.tag == TagValid) and non-overflowing duration
fn check_cell_lease
  (a: array cell_t)
  (idx: SZ.t)
  (current_clk: u64)
  (duration: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s)
  returns  diag: u32
  ensures  pts_to a s **
           pure (
             SZ.v idx < Seq.length s /\
             (let cell = Seq.index s (SZ.v idx) in
              (cell.tag == 0ul /\ FStar.UInt.size (U64.v current_clk + U64.v duration) 64 ==> diag == 0ul /\ tag_to_spec cell.tag == TagValid) /\
              (cell.tag <> 0ul ==> diag == 7ul /\ tag_to_spec cell.tag <> TagValid) /\
              (cell.tag == 0ul /\ ~(FStar.UInt.size (U64.v current_clk + U64.v duration) 64) ==> diag == 8ul))
           )
{
  pts_to_len a;
  let cell = a.(idx);
  if (cell.tag = 0ul) {
    let max_u64: u64 = 18446744073709551615uL;
    let rem = U64.sub max_u64 current_clk;
    if (U64.lte duration rem) {
      0ul
    } else {
      8ul
    }
  } else {
    7ul
  }
}

/// Invariant 11: Zero-Jitter Isochronous Pacing Step
/// Consumes target_pace cycles, ensuring execution timing jitter is zero.
fn wcet_isochronous_pace_step
  (consumed: ref u64)
  (cost: u64)
  (target_pace: u64)
  (max_limit: u64)
  (#cur: Ghost.erased u64)
  requires pts_to consumed cur
  returns  ok: bool
  ensures  exists* (new_cur: u64).
             pts_to consumed new_cur **
             pure (
               if (U64.v cost <= U64.v target_pace /\
                   U64.v cur + U64.v target_pace <= U64.v max_limit /\
                   FStar.UInt.size (U64.v cur + U64.v target_pace) 64) then
                 ok == true /\ U64.v new_cur == U64.v cur + U64.v target_pace
               else
                 ok == false /\ new_cur == cur
             )
{
  let current_val = !consumed;
  if (U64.lte cost target_pace) {
    if (U64.lte current_val max_limit) {
      let rem = U64.sub max_limit current_val;
      if (U64.lte target_pace rem) {
        let next_val = U64.add current_val target_pace;
        consumed := next_val;
        true
      } else {
        false
      }
    } else {
      false
    }
  } else {
    false
  }
}
