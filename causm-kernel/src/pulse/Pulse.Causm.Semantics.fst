// causm-kernel/src/pulse/Pulse.Causm.Semantics.fst
module Pulse.Causm.Semantics
#lang-pulse

open Pulse.Lib.Pervasives
open Pulse.Lib.Reference
open Pulse.Lib.Array
open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.TypeSystem
open Pulse.Causm.Arena

module U8 = FStar.UInt8
module U32 = FStar.UInt32
module U64 = FStar.UInt64
module SZ = FStar.SizeT
module Seq = FStar.Seq

type u8 = FStar.UInt8.t
type u32 = FStar.UInt32.t
type u64 = FStar.UInt64.t

/// In-place Entanglement Graph Representation:
/// An NxN adjacency matrix stored in a 1D array of size N*N where ent(r1, r2) != 0 means entangled.
type entangle_matrix = array u8

/// Check if two registers are entangled in the NxN matrix
fn is_entangled
  (m: array u8)
  (n: SZ.t)
  (r1: SZ.t)
  (r2: SZ.t)
  (#s: Ghost.erased (Seq.seq u8))
  requires pts_to m s ** pure (
    SZ.v n > 0 /\
    SZ.v r1 < SZ.v n /\
    SZ.v r2 < SZ.v n /\
    Seq.length s == SZ.v n * SZ.v n
  )
  returns  res: bool
  ensures  pts_to m s ** pure (
    Seq.length s == SZ.v n * SZ.v n
  )
{
  pts_to_len m;
  let idx = SZ.add (SZ.mul r1 n) r2;
  let val_u8 = m.(idx);
  let zero: u8 = 0uy;
  val_u8 <> zero
}

/// Set entanglement edge r1 <-> r2 symmetrically
fn set_entangle_edge
  (m: array u8)
  (n: SZ.t)
  (r1: SZ.t)
  (r2: SZ.t)
  (#s: Ghost.erased (Seq.seq u8))
  requires pts_to m s ** pure (
    SZ.v n > 0 /\
    SZ.v r1 < SZ.v n /\
    SZ.v r2 < SZ.v n /\
    Seq.length s == SZ.v n * SZ.v n
  )
  ensures  exists* (s': Seq.seq u8).
             pts_to m s' ** pure (
               Seq.length s' == Seq.length s
             )
{
  pts_to_len m;
  let one: u8 = 1uy;
  let idx1 = SZ.add (SZ.mul r1 n) r2;
  let idx2 = SZ.add (SZ.mul r2 n) r1;
  m.(idx1) <- one;
  m.(idx2) <- one;
}

/// Instruction 1: Execute ILoadInt (dest <- v)
/// Sets cell to Valid state with payload v, matching Spec.Causm.Semantics.eval_step
fn exec_load_int
  (regs: array cell_t)
  (dest: SZ.t)
  (v: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to regs s ** pure (SZ.v dest < Seq.length s)
  ensures  exists* (s': Seq.seq cell_t).
             pts_to regs s' ** pure (
               SZ.v dest < Seq.length s /\
               s' == Seq.upd s (SZ.v dest) { payload = v; tag = 0ul; expire = 0uL } /\
               tag_to_spec (Seq.index s' (SZ.v dest)).tag == TagValid
             )
{
  arena_init_valid regs dest v;
}

/// Instruction 2: Execute IAdd (dest <- src1 + src2)
/// Reads both src1 and src2 if readable at current_clk, sums them and writes to dest
fn exec_add
  (regs: array cell_t)
  (dest: SZ.t)
  (src1: SZ.t)
  (src2: SZ.t)
  (current_clk: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to regs s ** pure (
    SZ.v dest < Seq.length s /\
    SZ.v src1 < Seq.length s /\
    SZ.v src2 < Seq.length s
  )
  returns  ok: bool
  ensures  exists* (s': Seq.seq cell_t).
             pts_to regs s' ** pure (
               Seq.length s' == Seq.length s
             )
{
  pts_to_len regs;
  let diag1 = check_cell_access regs src1 current_clk;
  let diag2 = check_cell_access regs src2 current_clk;
  if (diag1 = 0ul && diag2 = 0ul) {
    let c1 = regs.(src1);
    let c2 = regs.(src2);
    let v1 = c1.payload;
    let v2 = c2.payload;
    let sum = U64.add_mod v1 v2;
    arena_init_valid regs dest sum;
    true
  } else {
    false
  }
}

/// Instruction 3: Execute IEntangle (r1 <-> r2)
/// Verifies both registers are consumable, then records edge in entanglement matrix
fn exec_entangle
  (regs: array cell_t)
  (ent_matrix: array u8)
  (n: SZ.t)
  (r1: SZ.t)
  (r2: SZ.t)
  (#s_regs: Ghost.erased (Seq.seq cell_t))
  (#s_ent: Ghost.erased (Seq.seq u8))
  requires pts_to regs s_regs ** pts_to ent_matrix s_ent ** pure (
    SZ.v n > 0 /\
    Seq.length s_regs == SZ.v n /\
    Seq.length s_ent == SZ.v n * SZ.v n /\
    SZ.v r1 < SZ.v n /\
    SZ.v r2 < SZ.v n
  )
  returns  ok: bool
  ensures  exists* (s_ent': Seq.seq u8).
             pts_to regs s_regs ** pts_to ent_matrix s_ent' ** pure (
               Seq.length s_ent' == Seq.length s_ent
             )
{
  pts_to_len regs;
  pts_to_len ent_matrix;
  if (r1 <> r2) {
    let diag1 = check_cell_consume regs r1;
    let diag2 = check_cell_consume regs r2;
    if (diag1 = 0ul && diag2 = 0ul) {
      set_entangle_edge ent_matrix n r1 r2;
      true
    } else {
      false
    }
  } else {
    false
  }
}

/// Instruction 4: Cascade Consumption Across Entanglement Graph
/// Recursively iterates through arena, consuming all registers connected to target
fn rec consume_entangled_sweep
  (regs: array cell_t)
  (ent_matrix: array u8)
  (n: SZ.t)
  (target: SZ.t)
  (curr: SZ.t)
  (#s_regs: Ghost.erased (Seq.seq cell_t))
  (#s_ent: Ghost.erased (Seq.seq u8))
  requires pts_to regs s_regs ** pts_to ent_matrix s_ent ** pure (
    SZ.v n > 0 /\
    Seq.length s_regs == SZ.v n /\
    Seq.length s_ent == SZ.v n * SZ.v n /\
    SZ.v target < SZ.v n /\
    SZ.v curr <= SZ.v n
  )
  ensures  exists* (s_regs': Seq.seq cell_t).
             pts_to regs s_regs' ** pts_to ent_matrix s_ent ** pure (
               Seq.length s_regs' == Seq.length s_regs
             )
  decreases (SZ.v n - SZ.v curr)
{
  pts_to_len regs;
  pts_to_len ent_matrix;
  if (SZ.lt curr n) {
    let connected = is_entangled ent_matrix n target curr;
    if (connected) {
      arena_consume regs curr;
      consume_entangled_sweep regs ent_matrix n target (SZ.add curr 1sz);
    } else {
      consume_entangled_sweep regs ent_matrix n target (SZ.add curr 1sz);
    };
  }
}

/// Instruction 5: Execute IConsume (target) with Entanglement Cascade
/// Verifies target is consumable, consumes target, and cascades consumption to entangled nodes
fn exec_consume_cascading
  (regs: array cell_t)
  (ent_matrix: array u8)
  (n: SZ.t)
  (target: SZ.t)
  (#s_regs: Ghost.erased (Seq.seq cell_t))
  (#s_ent: Ghost.erased (Seq.seq u8))
  requires pts_to regs s_regs ** pts_to ent_matrix s_ent ** pure (
    SZ.v n > 0 /\
    Seq.length s_regs == SZ.v n /\
    Seq.length s_ent == SZ.v n * SZ.v n /\
    SZ.v target < SZ.v n
  )
  returns  ok: bool
  ensures  exists* (s_regs': Seq.seq cell_t).
             pts_to regs s_regs' ** pts_to ent_matrix s_ent ** pure (
               Seq.length s_regs' == Seq.length s_regs
             )
{
  pts_to_len regs;
  let diag = check_cell_consume regs target;
  if (diag = 0ul) {
    arena_consume regs target;
    consume_entangled_sweep regs ent_matrix n target 0sz;
    true
  } else {
    false
  }
}

/// Instruction 6: Execute ILease (target, duration)
/// Transitions Valid register to Leased with expiration clock + duration
fn exec_lease
  (regs: array cell_t)
  (target: SZ.t)
  (current_clk: u64)
  (duration: u64)
  (#s: Ghost.erased (Seq.seq cell_t))
  requires pts_to regs s ** pure (SZ.v target < Seq.length s)
  returns  ok: bool
  ensures  exists* (s': Seq.seq cell_t).
             pts_to regs s' ** pure (
               Seq.length s' == Seq.length s
             )
{
  pts_to_len regs;
  let diag = check_cell_lease regs target current_clk duration;
  if (diag = 0ul) {
    arena_lease regs target current_clk duration;
    true
  } else {
    false
  }
}
