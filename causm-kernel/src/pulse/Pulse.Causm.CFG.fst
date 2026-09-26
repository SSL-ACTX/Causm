// causm-kernel/src/pulse/Pulse.Causm.CFG.fst
module Pulse.Causm.CFG
#lang-pulse

open Pulse.Lib.Pervasives
open Pulse.Lib.Reference
open Pulse.Lib.Array
open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.CFG
open Spec.Causm.TypeSystem
open Pulse.Causm.Arena
open Pulse.Causm.Semantics

module U32 = FStar.UInt32
module U64 = FStar.UInt64
module SZ = FStar.SizeT
module Seq = FStar.Seq

type u32 = FStar.UInt32.t
type u64 = FStar.UInt64.t

/// Flat Encoded Instruction for Verified Low-Level Execution:
/// op: 0 = ILoadInt (dest, imm, _), 1 = IAdd (dest, src1, src2),
///     2 = IConsume (target, _, _), 3 = ILease (target, duration, _),
///     4 = ITick (delta, _, _), 5 = IEntangle (r1, r2, _)
type raw_instr_t = {
  op: u32;
  arg1: SZ.t;
  arg2: SZ.t;
  arg3: SZ.t;
  imm: u64;
}

/// Execute a single raw instruction step over mutable VM state
fn exec_instr_step
  (regs: array cell_t)
  (ent_matrix: array u8)
  (n: SZ.t)
  (current_clk: ref u64)
  (i: raw_instr_t)
  (#s_regs: Ghost.erased (Seq.seq cell_t))
  (#s_ent: Ghost.erased (Seq.seq u8))
  (#cur_clk: Ghost.erased u64)
  requires pts_to regs s_regs ** pts_to ent_matrix s_ent ** pts_to current_clk cur_clk ** pure (
    SZ.v n > 0 /\
    Seq.length s_regs == SZ.v n /\
    Seq.length s_ent == SZ.v n * SZ.v n
  )
  returns  ok: bool
  ensures  exists* (s_regs': Seq.seq cell_t) (s_ent': Seq.seq u8) (new_clk: u64).
             pts_to regs s_regs' ** pts_to ent_matrix s_ent' ** pts_to current_clk new_clk ** pure (
               Seq.length s_regs' == Seq.length s_regs /\
               Seq.length s_ent' == Seq.length s_ent
             )
{
  pts_to_len regs;
  pts_to_len ent_matrix;
  let clk = !current_clk;
  if (i.op = 0ul) {
    // ILoadInt (dest = arg1, val = imm)
    if (SZ.lt i.arg1 n) {
      exec_load_int regs i.arg1 i.imm;
      true
    } else {
      false
    }
  } else if (i.op = 1ul) {
    // IAdd (dest = arg1, src1 = arg2, src2 = arg3)
    if (SZ.lt i.arg1 n && SZ.lt i.arg2 n && SZ.lt i.arg3 n) {
      exec_add regs i.arg1 i.arg2 i.arg3 clk
    } else {
      false
    }
  } else if (i.op = 2ul) {
    // IConsume (target = arg1)
    if (SZ.lt i.arg1 n) {
      exec_consume_cascading regs ent_matrix n i.arg1
    } else {
      false
    }
  } else if (i.op = 3ul) {
    // ILease (target = arg1, duration = imm)
    if (SZ.lt i.arg1 n) {
      exec_lease regs i.arg1 clk i.imm
    } else {
      false
    }
  } else if (i.op = 4ul) {
    // ITick (delta = imm)
    let max_u64: u64 = 18446744073709551615uL;
    let rem = U64.sub max_u64 clk;
    if (U64.lte i.imm rem) {
      let next_clk = U64.add clk i.imm;
      current_clk := next_clk;
      true
    } else {
      false
    }
  } else if (i.op = 5ul) {
    // IEntangle (r1 = arg1, r2 = arg2)
    if (SZ.lt i.arg1 n && SZ.lt i.arg2 n) {
      exec_entangle regs ent_matrix n i.arg1 i.arg2
    } else {
      false
    }
  } else {
    false
  }
}

/// Execute a full basic block instruction sequence iteratively
fn rec exec_body_loop
  (regs: array cell_t)
  (ent_matrix: array u8)
  (n: SZ.t)
  (current_clk: ref u64)
  (body: array raw_instr_t)
  (body_len: SZ.t)
  (pc: SZ.t)
  (#s_regs: Ghost.erased (Seq.seq cell_t))
  (#s_ent: Ghost.erased (Seq.seq u8))
  (#cur_clk: Ghost.erased u64)
  (#s_body: Ghost.erased (Seq.seq raw_instr_t))
  requires pts_to regs s_regs ** pts_to ent_matrix s_ent ** pts_to current_clk cur_clk ** pts_to body s_body ** pure (
    SZ.v n > 0 /\
    Seq.length s_regs == SZ.v n /\
    Seq.length s_ent == SZ.v n * SZ.v n /\
    Seq.length s_body == SZ.v body_len /\
    SZ.v pc <= SZ.v body_len
  )
  returns  ok: bool
  ensures  exists* (s_regs': Seq.seq cell_t) (s_ent': Seq.seq u8) (new_clk: u64).
             pts_to regs s_regs' ** pts_to ent_matrix s_ent' ** pts_to current_clk new_clk ** pts_to body s_body ** pure (
               Seq.length s_regs' == Seq.length s_regs /\
               Seq.length s_ent' == Seq.length s_ent
             )
  decreases (SZ.v body_len - SZ.v pc)
{
  pts_to_len body;
  if (SZ.lt pc body_len) {
    let instr = body.(pc);
    let step_ok = exec_instr_step regs ent_matrix n current_clk instr;
    if (step_ok) {
      exec_body_loop regs ent_matrix n current_clk body body_len (SZ.add pc 1sz)
    } else {
      false
    }
  } else {
    true
  }
}
