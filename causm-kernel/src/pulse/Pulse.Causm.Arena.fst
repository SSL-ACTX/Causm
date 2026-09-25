// causm-kernel/src/pulse/Pulse.Causm.Arena.fst
module Pulse.Causm.Arena
#lang-pulse

open Pulse.Lib.Pervasives
open Pulse.Lib.Reference
open Pulse.Lib.Array
module U32 = FStar.UInt32
module U64 = FStar.UInt64

type u32 = FStar.UInt32.t
type u64 = FStar.UInt64.t

type cell_t = {
  payload: u64;
  tag: u32; // 0: Valid, 1: Leased, 2: Decayed, 3: Consumed
  expire: u64;
}

type arena_cell = ref cell_t

/// Invariant 1: Dereferencing cell payload requires Valid state
fn read_valid_cell
  (c: arena_cell)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 0ul)
  returns  res: u64
  ensures  pts_to c s ** pure (res == s.payload)
{
  let cell = !c;
  cell.payload
}

/// Invariant 2: Consumption destroys access right, transitioning cell to St_Consumed
fn consume_cell
  (c: arena_cell)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 0ul)
  ensures  exists* (s': cell_t). pts_to c s' ** pure (s'.tag == 3ul)
{
  let cell = !c;
  c := { payload = 0uL; tag = 3ul; expire = 0uL };
}

/// Invariant 3: Leased borrows transition to St_Decayed when clock advances
fn tick_cell_decay
  (c: arena_cell)
  (current_clk: u64)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 1ul /\ U64.gte current_clk s.expire)
  ensures  exists* (s': cell_t). pts_to c s' ** pure (s'.tag == 2ul)
{
  let cell = !c;
  c := { payload = 0uL; tag = 2ul; expire = cell.expire };
}

/// Invariant 4: Lease creation transitions Valid to Leased with specified expiration
fn lease_cell
  (c: arena_cell)
  (current_clk: u64)
  (duration: u64)
  (#s: Ghost.erased cell_t)
  requires pts_to c s ** pure (s.tag == 0ul /\ FStar.UInt.size (U64.v current_clk + U64.v duration) 64)
  ensures  exists* (s': cell_t). pts_to c s' ** pure (s'.tag == 1ul /\ U64.v s'.expire == U64.v current_clk + U64.v duration)
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
  requires pts_to a s ** pure (SZ.v idx < Seq.length s /\ (Seq.index s (SZ.v idx)).tag == 0ul)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to a s' **
             pure (SZ.v idx < Seq.length s /\ s' == Seq.upd s (SZ.v idx) { payload = 0uL; tag = 3ul; expire = 0uL })
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
/// Tags: 0ul = Valid, 1ul = Leased, 2ul = Decayed, 3ul = Consumed
fn tag_meet_u32 (t1: u32) (t2: u32)
  returns res: u32
  ensures pure (
    (t1 == 3ul \/ t2 == 3ul ==> res == 3ul) /\
    (t1 <> 3ul /\ t2 <> 3ul /\ (t1 == 2ul \/ t2 == 2ul) ==> res == 2ul) /\
    (t1 <> 3ul /\ t2 <> 3ul /\ t1 <> 2ul /\ t2 <> 2ul /\ (t1 == 1ul \/ t2 == 1ul) ==> res == 1ul) /\
    (t1 == 0ul /\ t2 == 0ul ==> res == 0ul)
  )
{
  if (t1 = 3ul || t2 = 3ul) {
    3ul
  } else if (t1 = 2ul || t2 = 2ul) {
    2ul
  } else if (t1 = 1ul || t2 = 1ul) {
    1ul
  } else {
    0ul
  }
}

/// Array Invariant 6: Merge a register cell across two CFG branch predecessor outcomes
/// Statically converges to the lattice infimum (meet), ensuring sound conservative decay/consumption.
fn arena_merge_meet
  (dst: array cell_t)
  (idx: SZ.t)
  (pred_tag: u32)
  (pred_expire: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to dst s ** pure (SZ.v idx < Seq.length s)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to dst s' **
             pure (Seq.length s' == Seq.length s /\
                   SZ.v idx < Seq.length s' /\
                   (let cur = Seq.index s (SZ.v idx) in
                    let m = (Seq.index s' (SZ.v idx)).tag in
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




