use super::context::LoweringContext;
use super::statements::lower_spanned;
use causm_core::Program;
use causm_ir::{IrBlock, IrProgram, IrRoutine, Reg};
use std::collections::HashMap;

pub fn lower_program(program: &Program) -> IrProgram {
    let mut blocks = Vec::new();
    let mut ctx = LoweringContext::new();

    for tb in &program.timelines {
        let start_idx = ctx.instructions.len();
        for stmt in &tb.statements {
            lower_spanned(&mut ctx, stmt);
        }
        let block_instrs = ctx.instructions.split_off(start_idx);
        let block_spans = ctx.spans.split_off(start_idx);
        blocks.push(IrBlock {
            time: tb.time.clone(),
            instructions: block_instrs,
            spans: block_spans,
        });
    }

    let mut default_methods = HashMap::new();
    let mut sorted_struct_names: Vec<&String> = ctx.type_decls.keys().collect();
    sorted_struct_names.sort();
    for struct_name in sorted_struct_names {
        let mut sorted_interfaces: Vec<(
            &String,
            &Vec<causm_core::InterfaceMethod>,
        )> = ctx.interfaces.iter().collect();
        sorted_interfaces.sort_by_key(|(name, _)| *name);
        for (_interface_name, methods) in sorted_interfaces {
            let mut implements = true;
            for im in methods {
                let r_name = format!("{}.{}", struct_name, im.name);
                if !ctx.routines.contains_key(&r_name) && im.default_body.is_none() {
                    implements = false;
                    break;
                }
            }

            if implements {
                for im in methods {
                    let r_name = format!("{}.{}", struct_name, im.name);
                    if !ctx.routines.contains_key(&r_name)
                        && !default_methods.contains_key(&r_name)
                    {
                        if let Some(ref default_body) = im.default_body {
                            let mut sub_ctx = LoweringContext::new();
                            sub_ctx.type_decls = ctx.type_decls.clone();
                            sub_ctx.type_decay_limits =
                                ctx.type_decay_limits.clone();

                            for (i, param) in im.params.iter().enumerate() {
                                sub_ctx
                                    .symbols
                                    .insert(param.name.clone(), Reg(i as u32));
                                sub_ctx.next_reg = (i + 1) as u32;
                            }

                            if let Some((ref param_name, ref expected_state)) =
                                im.state_constraint
                            {
                                if let Some(&reg) = sub_ctx.symbols.get(param_name) {
                                    sub_ctx.push(
                                        causm_ir::Instruction::AssertState {
                                            src: reg,
                                            state: expected_state.clone(),
                                        },
                                    );
                                }
                            }

                            for s in default_body {
                                lower_spanned(&mut sub_ctx, s);
                            }

                            let routine = IrRoutine {
                                params: im.params
                                    .iter()
                                    .map(|p| {
                                        let mut t = p.typ
                                            .as_ref()
                                            .map(causm_core::types::Type::from_typename)
                                            .unwrap_or(causm_core::types::Type::Unknown);
                                        if p.name == "self" {
                                            t = causm_core::types::Type::Custom(struct_name.clone());
                                        }
                                        (p.mode.clone(), p.name.clone(), t)
                                    })
                                    .collect(),
                                return_type: im.return_type
                                    .as_ref()
                                    .map(causm_core::types::Type::from_typename)
                                    .unwrap_or(causm_core::types::Type::Unknown),
                                taking_ms: im.taking_ms,
                                instructions: sub_ctx.instructions,
                                spans: sub_ctx.spans,
                            };
                            default_methods.insert(r_name, routine);
                        }
                    }
                }
            }
        }
    }
    ctx.routines.extend(default_methods);

    IrProgram {
        blocks,
        routines: ctx.routines,
        symbols: ctx.symbols,
        type_decay_limits: ctx.type_decay_limits,
        struct_extends: ctx.struct_extends,
        decay_handlers: ctx.decay_handlers,
    }
}
