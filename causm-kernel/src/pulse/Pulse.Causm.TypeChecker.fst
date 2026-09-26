// causm-kernel/src/pulse/Pulse.Causm.TypeChecker.fst
module Pulse.Causm.TypeChecker
#lang-pulse

open Pulse.Lib.Pervasives
open Pulse.Lib.Reference
open Pulse.Lib.Array
open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.TypeSystem
open Pulse.Causm.Arena
open Pulse.Causm.Semantics
open Pulse.Causm.CFG

module U32 = FStar.UInt32
module U64 = FStar.UInt64
module SZ = FStar.SizeT
module Seq = FStar.Seq

type u32 = FStar.UInt32.t
type u64 = FStar.UInt64.t

/// Static Type Verification of a Flat Instruction
/// Verifies well-typedness of operands under current clock before execution
fn verify_instruction_typing
  (regs: array cell_t)
  (n: SZ.t)
  (current_clk: u64)
  (i: raw_instr_t)
  (#s_regs: Ghost.erased (Seq.seq cell_t))
  requires pts_to regs s_regs ** pure (
    SZ.v n > 0 /\
    Seq.length s_regs == SZ.v n
  )
  returns  ok: bool
  ensures  pts_to regs s_regs ** pure (
    Seq.length s_regs == SZ.v n
  )
{
  pts_to_len regs;
  if (i.op = 0ul) {
    // ILoadInt is unconditionally well-typed if dest is in bounds
    SZ.lt i.arg1 n
  } else if (i.op = 1ul) {
    // IAdd requires both src1 and src2 to be readable at current_clk
    if (SZ.lt i.arg1 n && SZ.lt i.arg2 n && SZ.lt i.arg3 n) {
      let diag1 = check_cell_access regs i.arg2 current_clk;
      let diag2 = check_cell_access regs i.arg3 current_clk;
      diag1 = 0ul && diag2 = 0ul
    } else {
      false
    }
  } else if (i.op = 2ul) {
    // IConsume requires target to be consumable
    if (SZ.lt i.arg1 n) {
      let diag = check_cell_consume regs i.arg1;
      diag = 0ul
    } else {
      false
    }
  } else if (i.op = 3ul) {
    // ILease requires target to be consumable and duration non-overflowing
    if (SZ.lt i.arg1 n) {
      let diag = check_cell_lease regs i.arg1 current_clk i.imm;
      diag = 0ul
    } else {
      false
    }
  } else if (i.op = 4ul) {
    // ITick is well-typed if clock addition does not overflow
    let max_u64: u64 = 18446744073709551615uL;
    let rem = U64.sub max_u64 current_clk;
    U64.lte i.imm rem
  } else if (i.op = 5ul) {
    // IEntangle requires distinct registers and both consumable
    if (SZ.lt i.arg1 n && SZ.lt i.arg2 n && i.arg1 <> i.arg2) {
      let diag1 = check_cell_consume regs i.arg1;
      let diag2 = check_cell_consume regs i.arg2;
      diag1 = 0ul && diag2 = 0ul
    } else {
      false
    }
  } else {
    false
  }
}

/// Static Verification of an Entire Basic Block
/// Iterates across block instructions verifying well-typedness and simulating state progression
fn rec verify_body_typing
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
    let clk = !current_clk;
    let well_typed = verify_instruction_typing regs n clk instr;
    if (well_typed) {
      let step_ok = exec_instr_step regs ent_matrix n current_clk instr;
      if (step_ok) {
        verify_body_typing regs ent_matrix n current_clk body body_len (SZ.add pc 1sz)
      } else {
        false
      }
    } else {
      false
    }
  } else {
    true
  }
}
