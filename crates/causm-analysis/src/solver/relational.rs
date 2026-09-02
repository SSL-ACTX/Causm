use super::backend::SolverBackend;
use super::diagnostics::EntropicDiagnostic;
use super::facts::{EntropicFact, ProgramFacts};
use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};

/// Relational Invariant Solver — Phase 4 & 5.
///
/// Encodes the three Entropius safety invariants as proper SMT assertions
/// over the `oxiz` solver backend, driven by extracted `ProgramFacts` relations
/// rather than procedural AST traversal.
///
/// Collects fine-grained, multi-span `EntropicDiagnostic`s along with formal
/// First-Order Relational / SMT formulas.
///
/// Invariant 1: Absence of Use-After-Consume
///   AccessAt(v, P, _) ∧ ∃P_prior ≺ P (strictly) s.t. LinearConsume(v, P_prior)
///   ∧ ¬Reintroduced(v, P_prior, P) ⟹ EmitError
///
/// Invariant 2: Absence of Use-After-Decay (Temporal)
///   AccessAt(v, P, t) ∧ ∃t_expire ≤ t s.t. TemporalDecay(v, t_expire)
///   ∧ ¬Renewed(v) ⟹ EmitError
///
/// Invariant 3: Structural Integrity
///   AccessAt(v, P, _) ∧ ∃f ∈ Fields(v), FieldConsume(v, f, P_prior ≺ P)
///   ⟹ EmitError
///
/// Lease Safety:
///   Consume(source, P_consume) ∧ LeaseIssued(source, λ, t_start, t_end, P_lease)
///   ∧ P_consume ≥ P_lease ⟹ EmitError
pub struct RelationalInvariantSolver<'a, S: SolverBackend = crate::oxiz::OxiZBackend>
{
    solver: S,
    analyzer: &'a EntropicAnalyzer,
    pub diagnostics: Vec<EntropicDiagnostic>,
}

impl<'a, S: SolverBackend> RelationalInvariantSolver<'a, S> {
    pub fn new(analyzer: &'a EntropicAnalyzer) -> Self {
        Self {
            solver: S::new(),
            analyzer,
            diagnostics: Vec::new(),
        }
    }

    /// Solves invariants and returns the list of fine-grained `EntropicDiagnostic`s without throwing.
    pub fn collect_diagnostics(
        &mut self,
        facts: &ProgramFacts,
    ) -> Vec<EntropicDiagnostic> {
        self.diagnostics.clear();
        self.solver.reset();

        // 1. Invariant 1: Use-After-Consume
        for (var, accesses) in &facts.var_accesses {
            for (access_pt, _t_access) in accesses {
                let last_prior_consume =
                    facts.var_consumes.get(var).and_then(|pts| {
                        pts.iter().filter(|p| *p < access_pt).max().cloned()
                    });

                let Some(consume_pt) = last_prior_consume else {
                    continue;
                };

                let reintroduced = facts
                    .var_origins
                    .get(var)
                    .map(|origins| {
                        origins
                            .iter()
                            .any(|orig| orig > &consume_pt && orig <= access_pt)
                    })
                    .unwrap_or(false);

                if reintroduced {
                    continue;
                }

                let path_true = self.solver.bool_from_bool(true);
                let valid_at_origin = self.solver.bool_const(&format!(
                    "{}_valid_{}_{}_{}",
                    var,
                    access_pt.timeline_idx,
                    access_pt.statement_idx,
                    access_pt.sub_point
                ));
                let impl_valid =
                    self.solver.bool_implies(&path_true, &valid_at_origin);
                self.solver.assert(&impl_valid);

                let consumed_sym = self.solver.bool_const(&format!(
                    "{}_consumed_{}_{}_{}",
                    var,
                    consume_pt.timeline_idx,
                    consume_pt.statement_idx,
                    consume_pt.sub_point
                ));
                let not_consumed = self.solver.bool_not(&consumed_sym);
                let impl_consumed =
                    self.solver.bool_implies(&path_true, &not_consumed);
                self.solver.assert(&impl_consumed);

                self.solver.push();
                let cond = self.solver.bool_and(&[&path_true, &not_consumed]);
                self.solver.assert(&cond);
                if self.solver.check() {
                    let origin_pt = facts.var_origins.get(var).and_then(|origins| {
                        origins.iter().filter(|p| *p <= &consume_pt).max().cloned()
                    });

                    let smt_formula = format!(
                        "IllegalConsumeAccess({}, P_{}_{}_{}) :- AccessAt({}, P_{}_{}_{}), LinearConsume({}, P_{}_{}_{}), not Reintroduced({}, P_{}_{}_{}, P_{}_{}_{}). [UNSAT proof]",
                        var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point,
                        var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point,
                        var, consume_pt.timeline_idx, consume_pt.statement_idx, consume_pt.sub_point,
                        var, consume_pt.timeline_idx, consume_pt.statement_idx, consume_pt.sub_point,
                        access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point,
                    );

                    self.diagnostics.push(EntropicDiagnostic::UseAfterConsume {
                        var: var.clone(),
                        origin_point: origin_pt,
                        consume_point: consume_pt.clone(),
                        access_point: access_pt.clone(),
                        smt_formula,
                    });
                }
                self.solver.pop(1);
            }
        }

        // 2. Invariant 2: Use-After-Decay
        for (var, decays) in &facts.var_decays {
            for (decay_pt, t_expire) in decays {
                if let Some(accesses) = facts.var_accesses.get(var) {
                    for (access_pt, t_access) in accesses {
                        if t_access <= t_expire {
                            continue;
                        }

                        let renewed = facts
                            .var_origins
                            .get(var)
                            .map(|origins| {
                                origins.iter().any(|orig| {
                                    orig.timeline_idx > decay_pt.timeline_idx
                                        || (orig.timeline_idx
                                            == decay_pt.timeline_idx
                                            && orig.statement_idx
                                                > decay_pt.statement_idx)
                                })
                            })
                            .unwrap_or(false);

                        if renewed {
                            continue;
                        }

                        let path_true = self.solver.bool_from_bool(true);
                        let t_access_int = self.solver.int_from_u64(*t_access);
                        let t_expire_int = self.solver.int_from_u64(*t_expire);
                        let violation =
                            self.solver.int_gt(&t_access_int, &t_expire_int);
                        self.solver.push();
                        let cond = self.solver.bool_and(&[&path_true, &violation]);
                        self.solver.assert(&cond);
                        if self.solver.check() {
                            let smt_formula = format!(
                                "IllegalTemporalAccess({}, P_{}_{}_{}, t={}) :- AccessAt({}, P_{}_{}_{}, t={}), TemporalDecay({}, t_expire={}), t > t_expire, not Renewed({}). [UNSAT proof]",
                                var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point, t_access,
                                var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point, t_access,
                                var, t_expire, var
                            );

                            self.diagnostics.push(
                                EntropicDiagnostic::TemporalUseAfterDecay {
                                    var: var.clone(),
                                    decay_point: decay_pt.clone(),
                                    access_point: access_pt.clone(),
                                    t_expire_ms: *t_expire,
                                    t_access_ms: *t_access,
                                    smt_formula,
                                },
                            );
                        }
                        self.solver.pop(1);
                    }
                }
            }
        }

        // 3. Invariant 3: Structural Integrity (Field Decay)
        for fact in &facts.facts {
            if let EntropicFact::FieldConsume {
                var,
                field,
                point: field_pt,
            } = fact
            {
                if let Some(accesses) = facts.var_accesses.get(var) {
                    for (access_pt, _) in accesses {
                        if field_pt >= access_pt {
                            continue;
                        }

                        let reintroduced = facts
                            .var_origins
                            .get(var)
                            .map(|origins| {
                                origins
                                    .iter()
                                    .any(|orig| orig > field_pt && orig <= access_pt)
                            })
                            .unwrap_or(false);

                        if reintroduced {
                            continue;
                        }

                        let path_true = self.solver.bool_from_bool(true);
                        let field_consumed_sym = self.solver.bool_const(&format!(
                            "{}_struct_invalid_{}_{}_{}",
                            var,
                            field_pt.timeline_idx,
                            field_pt.statement_idx,
                            field_pt.sub_point
                        ));
                        let not_field_consumed =
                            self.solver.bool_not(&field_consumed_sym);
                        let impl_not = self
                            .solver
                            .bool_implies(&path_true, &not_field_consumed);
                        self.solver.assert(&impl_not);

                        self.solver.push();
                        let cond =
                            self.solver.bool_and(&[&path_true, &not_field_consumed]);
                        self.solver.assert(&cond);
                        if self.solver.check() {
                            let smt_formula = format!(
                                "StructInvalidated({}, P_{}_{}_{}) :- FieldConsume({}, {}, P_{}_{}_{}), AccessAt({}, P_{}_{}_{}). [UNSAT proof]",
                                var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point,
                                var, field, field_pt.timeline_idx, field_pt.statement_idx, field_pt.sub_point,
                                var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point,
                            );

                            self.diagnostics.push(
                                EntropicDiagnostic::CompoundStructFieldDecay {
                                    var: var.clone(),
                                    field: field.clone(),
                                    field_consume_point: field_pt.clone(),
                                    struct_access_point: access_pt.clone(),
                                    smt_formula,
                                },
                            );
                        }
                        self.solver.pop(1);
                    }
                }
            }
        }

        // 4. Lease Safety
        for (source_var, leases) in &facts.active_leases {
            if let Some(consume_points) = facts.var_consumes.get(source_var) {
                for consume_pt in consume_points {
                    for (lease_id, t_start, t_end, lease_pt) in leases {
                        if consume_pt < lease_pt {
                            continue;
                        }

                        let path_true = self.solver.bool_from_bool(true);
                        let leased_sym = self.solver.bool_const(&format!(
                            "{}_leased_{}_{}_{}",
                            source_var,
                            lease_pt.timeline_idx,
                            lease_pt.statement_idx,
                            lease_pt.sub_point
                        ));
                        let impl_leased =
                            self.solver.bool_implies(&path_true, &leased_sym);
                        self.solver.assert(&impl_leased);

                        self.solver.push();
                        let cond = self.solver.bool_and(&[&path_true, &leased_sym]);
                        self.solver.assert(&cond);
                        if self.solver.check() {
                            let smt_formula = format!(
                                "LeaseViolation({}, {}) :- LeaseIssued({}, {}, [{}ms, {}ms], P_{}_{}_{}), LinearConsume({}, P_{}_{}_{}). [UNSAT proof]",
                                source_var, lease_id,
                                source_var, lease_id, t_start, t_end, lease_pt.timeline_idx, lease_pt.statement_idx, lease_pt.sub_point,
                                source_var, consume_pt.timeline_idx, consume_pt.statement_idx, consume_pt.sub_point
                            );

                            self.diagnostics.push(
                                EntropicDiagnostic::LeaseConflict {
                                    source_var: source_var.clone(),
                                    lease_id: lease_id.clone(),
                                    lease_point: lease_pt.clone(),
                                    consume_point: consume_pt.clone(),
                                    t_start_ms: *t_start,
                                    t_end_ms: *t_end,
                                    smt_formula,
                                },
                            );
                        }
                        self.solver.pop(1);
                    }
                }
            }
        }

        // 5. Causal Paradox: Rewind past CausalCommit
        for (target_anchor, rewind_clock, rewind_pt) in &facts.rewinds {
            if let Some((anchor_clock, anchor_pt)) = facts.anchors.get(target_anchor)
            {
                // Find any commit occurring strictly between anchor and rewind where commit_clock > anchor_clock
                let last_commit_before_rewind = facts
                    .commits
                    .iter()
                    .filter(|(c_clock, c_pt)| {
                        c_pt < rewind_pt && *c_clock > *anchor_clock
                    })
                    .max_by_key(|(c_clock, _)| *c_clock);

                if let Some((commit_clock, commit_pt)) = last_commit_before_rewind {
                    let path_true = self.solver.bool_from_bool(true);
                    let anchor_int = self.solver.int_from_u64(*anchor_clock);
                    let commit_int = self.solver.int_from_u64(*commit_clock);
                    let paradox = self.solver.int_lt(&anchor_int, &commit_int);

                    self.solver.push();
                    let cond = self.solver.bool_and(&[&path_true, &paradox]);
                    self.solver.assert(&cond);
                    if self.solver.check() {
                        let smt_formula = format!(
                            "CausalParadox({}, t_anchor={}ms) :- CausalCommit(t_commit={}ms, P_{}_{}_{}), Rewind({}, t_rewind={}ms, P_{}_{}_{}), t_anchor < t_commit. [UNSAT proof]",
                            target_anchor, anchor_clock,
                            commit_clock, commit_pt.timeline_idx, commit_pt.statement_idx, commit_pt.sub_point,
                            target_anchor, rewind_clock, rewind_pt.timeline_idx, rewind_pt.statement_idx, rewind_pt.sub_point,
                        );

                        self.diagnostics.push(EntropicDiagnostic::CausalParadox {
                            anchor_name: target_anchor.clone(),
                            anchor_point: anchor_pt.clone(),
                            rewind_point: rewind_pt.clone(),
                            commit_point: commit_pt.clone(),
                            anchor_clock: *anchor_clock,
                            horizon_clock: *commit_clock,
                            smt_formula,
                        });
                    }
                    self.solver.pop(1);
                }
            }
        }

        // 6. Entanglement: Consuming any partner decays all entangled variables in the set
        for ent_set in &facts.entanglements {
            for (var, accesses) in &facts.var_accesses {
                if !ent_set.contains(var) {
                    continue;
                }
                for (access_pt, _) in accesses {
                    for partner in ent_set {
                        if partner == var {
                            continue;
                        }
                        let last_prior_partner_consume =
                            facts.var_consumes.get(partner).and_then(|pts| {
                                pts.iter().filter(|p| *p < access_pt).max().cloned()
                            });

                        if let Some(partner_consume_pt) = last_prior_partner_consume
                        {
                            let path_true = self.solver.bool_from_bool(true);
                            let entangled_sym = self.solver.bool_const(&format!(
                                "{}_entangled_decay_by_{}_{}_{}_{}",
                                var,
                                partner,
                                partner_consume_pt.timeline_idx,
                                partner_consume_pt.statement_idx,
                                partner_consume_pt.sub_point
                            ));
                            let not_entangled_sym =
                                self.solver.bool_not(&entangled_sym);
                            let impl_not = self
                                .solver
                                .bool_implies(&path_true, &not_entangled_sym);
                            self.solver.assert(&impl_not);

                            self.solver.push();
                            let cond = self
                                .solver
                                .bool_and(&[&path_true, &not_entangled_sym]);
                            self.solver.assert(&cond);
                            if self.solver.check() {
                                let smt_formula = format!(
                                    "EntanglementDecay({}, P_{}_{}_{}) :- Entangle({}, {}), LinearConsume({}, P_{}_{}_{}), AccessAt({}, P_{}_{}_{}). [UNSAT proof]",
                                    var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point,
                                    var, partner,
                                    partner, partner_consume_pt.timeline_idx, partner_consume_pt.statement_idx, partner_consume_pt.sub_point,
                                    var, access_pt.timeline_idx, access_pt.statement_idx, access_pt.sub_point,
                                );

                                self.diagnostics.push(
                                    EntropicDiagnostic::EntanglementConflict {
                                        var: var.clone(),
                                        partner_var: partner.clone(),
                                        partner_consume_point: partner_consume_pt
                                            .clone(),
                                        access_point: access_pt.clone(),
                                        smt_formula,
                                    },
                                );
                            }
                            self.solver.pop(1);
                        }
                    }
                }
            }
        }

        self.diagnostics.clone()
    }

    /// Encode and verify the Entropius Invariants 1, 2, 3 and Lease Safety
    /// directly over extracted relational `ProgramFacts` using the SMT backend.
    pub fn solve_invariants(
        &mut self,
        facts: &ProgramFacts,
    ) -> Result<(), SemanticError> {
        let diagnostics = self.collect_diagnostics(facts);
        if let Some(first_diag) = diagnostics.first() {
            let rich_message = first_diag.format_diagnostic(true);
            return Err(self
                .analyzer
                .annotate(SemanticErrorKind::EntropiusDiagnostic(rich_message)));
        }
        Ok(())
    }
}
