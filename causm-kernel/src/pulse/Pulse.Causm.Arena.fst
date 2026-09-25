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

/// Array Invariant 1: Safe indexed read requiring valid cell at index
fn arena_read
  (a: array cell_t)
  (idx: SZ.t)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to a s ** pure (SZ.v idx < Seq.length s /\ (Seq.index s (SZ.v idx)).tag == 0ul)
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


