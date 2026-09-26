// causm-kernel/src/proof/Proof.Causm.CFGSoundness.fst
module Proof.Causm.CFGSoundness

open Spec.Causm.Lattice
open Spec.Causm.Timeline
open Spec.Causm.Semantics
open Spec.Causm.TypeSystem
open Spec.Causm.CFG
open Proof.Causm.TypeSafety

/// Static Terminator Well-Formedness
let wf_terminator (env: type_env) (clk: nat) (t: terminator) : bool =
  match t with
  | TermReturn ret_reg ->
      is_ty_readable (env ret_reg) clk
  | TermJump _ ->
      true
  | TermBranch cond_reg _ _ ->
      is_ty_readable (env cond_reg) clk

/// Lemma: Well-formed terminator execution never gets stuck
val lemma_terminator_progress :
  env:type_env -> s:cfg_state -> t:terminator ->
  Lemma (requires (state_well_typed env s.vm /\
                   wf_terminator env s.vm.clock t == true))
        (ensures (Some? (eval_terminator s t)))
let lemma_terminator_progress env s t =
  match t with
  | TermReturn ret_reg ->
      lemma_ty_readable_sound env s.vm ret_reg
  | TermJump _ -> ()
  | TermBranch cond_reg _ _ ->
      lemma_ty_readable_sound env s.vm cond_reg

/// Inductive definition: A sequence of instructions is well-formed under an environment
val wf_body : env:type_env -> clk:nat -> ent:entangle_rel -> body:list instr -> Tot bool (decreases body)
let rec wf_body env clk ent body =
  match body with
  | [] -> true
  | i :: rest ->
      wf_instruction env clk i &&
      (let env' = type_step env clk ent i in
       let clk' = instr_clock_step clk i in
       let ent' = instr_entangle_step ent i in
       wf_body env' clk' ent' rest)

/// Block typing: A block is well-typed if its body and terminator are valid under incoming env
let wf_block (in_env: type_env) (clk: nat) (ent: entangle_rel) (bb: basic_block) : bool =
  wf_body in_env clk ent bb.body

/// Main Theorem: CFG Single-Step Progress
/// If the current PC is pointing at a well-typed instruction or terminator,
/// the CFG step is guaranteed to proceed without trapping.
val theorem_cfg_step_progress :
  g:cfg -> s:cfg_state -> bb:basic_block -> env:type_env ->
  Lemma (requires (get_bb g s.cur_bb == Some bb /\
                   state_well_typed env s.vm /\
                   s.halted == false /\
                   ((s.pc < List.Tot.length bb.body /\
                     wf_instruction env s.vm.clock (List.Tot.index bb.body s.pc) == true) \/
                    (s.pc == List.Tot.length bb.body /\
                     wf_terminator env s.vm.clock bb.term == true))))
        (ensures (Some? (cfg_step g s)))
let theorem_cfg_step_progress g s bb env =
  if s.pc < List.Tot.length bb.body then
    let i = List.Tot.index bb.body s.pc in
    theorem_progress env s.vm i
  else
    lemma_terminator_progress env s bb.term

/// Main Theorem: CFG Single-Step Preservation
/// When an intra-block instruction executes within a basic block,
/// the resulting state satisfies the statically updated environment.
val theorem_cfg_step_preservation :
  g:cfg -> s:cfg_state -> bb:basic_block -> env:type_env -> s':cfg_state ->
  Lemma (requires (get_bb g s.cur_bb == Some bb /\
                   state_well_typed env s.vm /\
                   s.halted == false /\
                   s.pc < List.Tot.length bb.body /\
                   wf_instruction env s.vm.clock (List.Tot.index bb.body s.pc) == true /\
                   cfg_step g s == Some s'))
        (ensures (let i = List.Tot.index bb.body s.pc in
                  state_well_typed (type_step env s.vm.clock s.vm.entangled i) s'.vm /\
                  s'.pc == s.pc + 1 /\
                  s'.cur_bb == s.cur_bb))
let theorem_cfg_step_preservation g s bb env s' =
  let i = List.Tot.index bb.body s.pc in
  theorem_preservation env s.vm i s'.vm

/// Inductive Body Preservation: Evaluating an entire basic block body
/// preserves type conformance from incoming environment to outgoing environment.
val theorem_body_preservation :
  env:type_env -> s:vm_state -> body:list instr -> s':vm_state ->
  Lemma (requires (state_well_typed env s /\
                   wf_body env s.clock s.entangled body == true /\
                   eval_body s body == Some s'))
        (ensures (state_well_typed (type_step_body env s.clock s.entangled body) s'))
        (decreases body)
let rec theorem_body_preservation env s body s' =
  match body with
  | [] -> ()
  | i :: rest ->
      (match eval_step s i with
       | Some s_next ->
           theorem_preservation env s i s_next;
           let env' = type_step env s.clock s.entangled i in
           theorem_body_preservation env' s_next rest s'
       | None -> ())

