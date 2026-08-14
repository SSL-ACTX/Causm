use super::context::LoweringContext;
use causm_core::{Expression, TypeFieldDef};
use causm_ir::Reg;
use std::collections::HashMap;

pub fn lower_expression(ctx: &mut LoweringContext, expr: &Expression) -> Reg {
    match expr {
        Expression::Integer(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadInt { dest, value: *v });
            dest
        }
        Expression::Float(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadFloat { dest, value: *v });
            dest
        }
        Expression::Boolean(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadBool { dest, value: *v });
            dest
        }
        Expression::Literal(s) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadString {
                dest,
                value: s.clone(),
            });
            dest
        }
        Expression::Null => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadNull { dest });
            dest
        }
        Expression::Identifier(name) => ctx.get_reg(name),
        Expression::BinaryOp { left, op, right } => {
            let l = lower_expression(ctx, left);
            let r = lower_expression(ctx, right);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::BinaryOp {
                dest,
                op: *op,
                left: l,
                right: r,
            });
            dest
        }
        Expression::UnaryOp { op, expr } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::UnaryOp { dest, op: *op, src });
            dest
        }

        Expression::MethodCall {
            target,
            method,
            args,
            resolved_routine,
            resolved_budget,
        } => {
            let routine_name =
                resolved_routine.borrow().clone().unwrap_or_else(|| {
                    panic!("MethodCall was not resolved during semantic analysis");
                });
            let mut arg_regs = Vec::new();
            arg_regs.push(lower_expression(ctx, target));
            for arg in args {
                arg_regs.push(lower_expression(ctx, arg));
            }
            let dest = ctx.alloc_reg();
            if routine_name == "<enum_constructor>" {
                let mut payload_regs = Vec::new();
                for arg in args {
                    payload_regs.push(lower_expression(ctx, arg));
                }
                let src = if !payload_regs.is_empty() {
                    payload_regs[0]
                } else {
                    ctx.alloc_reg()
                };
                ctx.push(causm_ir::Instruction::Move { dest, src });
            } else if routine_name == "<dynamic>" {
                let budget = *resolved_budget.borrow();
                ctx.push(causm_ir::Instruction::DynamicCall {
                    method: method.clone(),
                    args: arg_regs,
                    dest,
                    budget,
                });
            } else {
                ctx.push(causm_ir::Instruction::Call {
                    routine: routine_name,
                    args: arg_regs,
                    dest,
                });
            }
            dest
        }

        Expression::Call { routine, args } => {
            let mut arg_regs = Vec::new();
            for arg in args {
                arg_regs.push(lower_expression(ctx, arg));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Call {
                routine: routine.clone(),
                args: arg_regs,
                dest,
            });
            dest
        }
        Expression::CloneOp(name) => {
            let src = ctx.get_reg(name);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Clone { dest, src });
            dest
        }
        Expression::FieldAccess { target, field } => {
            let mut const_expr = None;
            if let Expression::Identifier(ref name) = &**target {
                if let Some(fields_map) = ctx.type_decls.get(name) {
                    if let Some(field_def) = fields_map.get(field) {
                        if field_def.is_const {
                            if let Some(ref val_expr) = field_def.default_value {
                                const_expr = Some(val_expr.clone());
                            }
                        }
                    }
                }
            }
            if let Some(expr) = const_expr {
                return lower_expression(ctx, &expr);
            }
            let t = lower_expression(ctx, target);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::FieldAccess {
                dest,
                target: t,
                field: field.clone(),
            });
            dest
        }
        Expression::IndexAccess { target, index } => {
            let t = lower_expression(ctx, target);
            let i = lower_expression(ctx, index);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::IndexAccess {
                dest,
                target: t,
                index: i,
            });
            dest
        }
        Expression::StructLit(type_name, fields) => {
            let mut field_regs = HashMap::new();
            let mut sorted_fields: Vec<(&String, &Expression)> =
                fields.iter().collect();
            sorted_fields.sort_by_key(|(name, _)| *name);
            for (name, expr) in sorted_fields {
                field_regs.insert(name.clone(), lower_expression(ctx, expr));
            }
            let type_name_opt = type_name.borrow().clone();
            let mut defaults_to_lower = Vec::new();
            if let Some(ref name) = type_name_opt {
                if let Some(fields_map) = ctx.type_decls.get(name) {
                    let mut sorted_fields_map: Vec<(&String, &TypeFieldDef)> =
                        fields_map.iter().collect();
                    sorted_fields_map.sort_by_key(|(name, _)| *name);
                    for (field_name, field_def) in sorted_fields_map {
                        if !field_def.is_const
                            && !field_regs.contains_key(field_name)
                        {
                            if let Some(ref default_expr) = field_def.default_value {
                                defaults_to_lower.push((
                                    field_name.clone(),
                                    default_expr.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            for (field_name, expr) in defaults_to_lower {
                field_regs.insert(field_name, lower_expression(ctx, &expr));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::StructLit {
                dest,
                fields: field_regs,
                type_name: type_name_opt,
            });
            dest
        }
        Expression::TopologyLit(fields) => {
            let mut field_regs = HashMap::new();
            let mut sorted_fields: Vec<(&String, &Expression)> =
                fields.iter().collect();
            sorted_fields.sort_by_key(|(name, _)| *name);
            for (name, expr) in sorted_fields {
                field_regs.insert(name.clone(), lower_expression(ctx, expr));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::TopologyLit {
                dest,
                fields: field_regs,
            });
            dest
        }
        Expression::ArrayLiteral(elements) => {
            let mut elem_regs = Vec::new();
            for e in elements {
                elem_regs.push(lower_expression(ctx, e));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ArrayLit {
                dest,
                elements: elem_regs,
            });
            dest
        }
        Expression::ChannelReceive(chan_id) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ChanRecv {
                dest,
                chan_id: chan_id.clone(),
            });
            dest
        }
        Expression::Syscall {
            target,
            args,
            duration_ms,
        } => {
            let mut arg_regs = Vec::new();
            for a in args {
                arg_regs.push(lower_expression(ctx, a));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Syscall {
                dest,
                target: target.clone(),
                args: arg_regs,
                duration_ms: *duration_ms,
            });
            dest
        }
        Expression::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => {
            let tag_reg = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadString {
                dest: tag_reg,
                value: variant_name.clone(),
            });
            let mut fields_map = HashMap::new();
            fields_map.insert("tag".to_string(), tag_reg);
            for (idx, arg_expr) in args.iter().enumerate() {
                let arg_reg = lower_expression(ctx, arg_expr);
                fields_map.insert(format!("_{}", idx), arg_reg);
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::StructLit {
                dest,
                fields: fields_map,
                type_name: Some(format!("{}::{}", enum_name, variant_name)),
            });
            dest
        }
        Expression::Deferred {
            capability,
            params,
            deadline_ms,
        } => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Defer {
                dest,
                cap: causm_core::Capability {
                    path: capability.clone(),
                    parameters: params.clone(),
                },
                deadline_ms: *deadline_ms,
            });
            dest
        }
        Expression::TypeAssertion { target, cast_type } => {
            let src = lower_expression(ctx, target);
            let dest = ctx.alloc_reg();
            let type_name = match cast_type {
                causm_core::TypeName::Custom(ref s) => s.clone(),
                _ => panic!("Type assertion target must be a custom type name"),
            };
            ctx.push(causm_ir::Instruction::TypeAssert {
                dest,
                src,
                type_name,
            });
            dest
        }
        Expression::TypeCast { expr, target_type } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::TypeCast {
                dest,
                src,
                target_type: target_type.clone(),
            });
            dest
        }
        Expression::TryUnwrap(expr) => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Move { dest, src });
            dest
        }
        Expression::RefOp(expr) => lower_expression(ctx, expr),
    }
}
