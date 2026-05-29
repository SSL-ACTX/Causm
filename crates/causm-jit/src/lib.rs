use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::collections::HashMap;
use thiserror::Error;

use causm_frontend::ir::{Instruction, IrRoutine};

#[derive(Debug, Error)]
pub enum JitError {
    #[error("Compilation error: {0}")]
    CompileError(String),
    #[error("Finalization error: {0}")]
    FinalizeError(String),
}

pub struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
}

impl Jit {
    pub fn new(external_symbols: HashMap<String, *const u8>) -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {}", msg);
        });
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();
        let mut builder =
            JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        // Register symbols for external calls
        builder.symbol("read_tsc", hw_timing::read_tsc as *const u8);
        builder.symbol("spin_pad", hw_timing::spin_pad as *const u8);
        
        for (name, ptr) in external_symbols {
            builder.symbol(name, ptr);
        }

        let module = JITModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
        }
    }

    pub fn compile_routine(
        &mut self,
        name: &str,
        routine: &IrRoutine,
    ) -> Result<*const u8, JitError> {
        self.module.clear_context(&mut self.ctx);

        let mut sig = self.module.make_signature();
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // Vm pointer
        sig.params
            .push(AbiParam::new(self.module.target_config().pointer_type())); // Timeline pointer
        sig.params.push(AbiParam::new(types::I64)); // Start TSC (provided by VM)

        // Add routine parameters as i64
        for _ in &routine.params {
            sig.params.push(AbiParam::new(types::I64));
        }

        sig.returns.push(AbiParam::new(types::I64));

        let func_id = self
            .module
            .declare_function(name, Linkage::Export, &sig)
            .map_err(|e| JitError::CompileError(e.to_string()))?;

        self.ctx.func.signature = sig;

        let mut builder =
            FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let mut regs: HashMap<u32, Variable> = HashMap::new();
        let mut next_var = 0;

        // Map routine parameters (R0, R1, ...)
        for i in 0..routine.params.len() {
            let arg_val = builder.block_params(entry_block)[i + 3];
            let v = Variable::new(next_var);
            builder.declare_var(v, types::I64);
            next_var += 1;
            builder.def_var(v, arg_val);
            regs.insert(i as u32, v);
        }

        // Use provided start_tsc from arguments
        let provided_start_tsc = builder.block_params(entry_block)[2];
        let start_tsc_var = if routine.taking_cycles.is_some() {
            let v = Variable::new(next_var);
            builder.declare_var(v, types::I64);
            next_var += 1;
            builder.def_var(v, provided_start_tsc);
            Some(v)
        } else {
            None
        };

        let mut all_blocks: Vec<Block> = Vec::new();
        let end_block = builder.create_block();
        all_blocks.push(end_block);

        let mut blocks: HashMap<usize, Block> = HashMap::new();
        // Pre-create blocks for all potential jump targets AND fallthroughs
        for (i, instr) in routine.instructions.iter().enumerate() {
            match instr {
                Instruction::Jump { target } => {
                    blocks.entry(*target).or_insert_with(|| {
                        let b = builder.create_block();
                        all_blocks.push(b);
                        b
                    });
                }
                Instruction::JumpIf { target, .. } => {
                    blocks.entry(*target).or_insert_with(|| {
                        let b = builder.create_block();
                        all_blocks.push(b);
                        b
                    });
                    blocks.entry(i + 1).or_insert_with(|| {
                        let b = builder.create_block();
                        all_blocks.push(b);
                        b
                    });
                }
                Instruction::JumpIfNot { target, .. } => {
                    blocks.entry(*target).or_insert_with(|| {
                        let b = builder.create_block();
                        all_blocks.push(b);
                        b
                    });
                    blocks.entry(i + 1).or_insert_with(|| {
                        let b = builder.create_block();
                        all_blocks.push(b);
                        b
                    });
                }
                _ => {}
            }
        }

        let mut block_filled = false;
        for (i, instr) in routine.instructions.iter().enumerate() {
            if let Some(block) = blocks.get(&i) {
                if !block_filled {
                    builder.ins().jump(*block, &[]);
                }
                builder.switch_to_block(*block);
                block_filled = false;
            }

            if block_filled {
                continue;
            }

            match instr {
                Instruction::Jump { target } => {
                    let target_block = *blocks.get(target).unwrap();
                    builder.ins().jump(target_block, &[]);
                    block_filled = true;
                }
                Instruction::JumpIf { cond, target } => {
                    let c_val = builder.use_var(*regs.get(&cond.0).unwrap());
                    let target_block = *blocks.get(target).unwrap();
                    let next_block = *blocks.get(&(i + 1)).unwrap();

                    let zero = builder.ins().iconst(types::I64, 0);
                    let cond_bool = builder.ins().icmp(IntCC::NotEqual, c_val, zero);
                    builder.ins().brif(
                        cond_bool,
                        target_block,
                        &[],
                        next_block,
                        &[],
                    );
                    block_filled = true;
                }
                Instruction::JumpIfNot { cond, target } => {
                    let c_val = builder.use_var(*regs.get(&cond.0).unwrap());
                    let target_block = *blocks.get(target).unwrap();
                    let next_block = *blocks.get(&(i + 1)).unwrap();

                    let zero = builder.ins().iconst(types::I64, 0);
                    let cond_bool = builder.ins().icmp(IntCC::Equal, c_val, zero);
                    builder.ins().brif(
                        cond_bool,
                        target_block,
                        &[],
                        next_block,
                        &[],
                    );
                    block_filled = true;
                }
                Instruction::LoadInt { dest, value } => {
                    let var = *regs.entry(dest.0).or_insert_with(|| {
                        let v = Variable::new(next_var);
                        builder.declare_var(v, types::I64);
                        next_var += 1;
                        v
                    });
                    let val = builder.ins().iconst(types::I64, *value);
                    builder.def_var(var, val);
                }
                Instruction::Move { dest, src } => {
                    let src_val = builder.use_var(
                        *regs
                            .get(&src.0)
                            .expect(&format!("Reg R{} not found for Move", src.0)),
                    );
                    let d_var = *regs.entry(dest.0).or_insert_with(|| {
                        let v = Variable::new(next_var);
                        builder.declare_var(v, types::I64);
                        next_var += 1;
                        v
                    });
                    builder.def_var(d_var, src_val);
                }
                Instruction::CMov {
                    dest,
                    cond,
                    then_src,
                    else_src,
                } => {
                    let c_val = builder.use_var(*regs.get(&cond.0).expect(
                        &format!("Reg R{} not found for CMov cond", cond.0),
                    ));
                    let t_val = builder.use_var(*regs.get(&then_src.0).expect(
                        &format!("Reg R{} not found for CMov then", then_src.0),
                    ));
                    let e_val = builder.use_var(*regs.get(&else_src.0).expect(
                        &format!("Reg R{} not found for CMov else", else_src.0),
                    ));

                    // Convert i64 to boolean for select
                    let zero = builder.ins().iconst(types::I64, 0);
                    let cond_bool = builder.ins().icmp(IntCC::NotEqual, c_val, zero);

                    let res = builder.ins().select(cond_bool, t_val, e_val);

                    let d_var = *regs.entry(dest.0).or_insert_with(|| {
                        let v = Variable::new(next_var);
                        builder.declare_var(v, types::I64);
                        next_var += 1;
                        v
                    });
                    builder.def_var(d_var, res);
                }
                Instruction::Return { src } => {
                    if let Some(reg) = src {
                        let val = builder.use_var(*regs.get(&reg.0).expect(
                            &format!("Reg R{} not found for Return", reg.0),
                        ));
                        // Store in R0 equivalent in JIT
                        let d_var = *regs.entry(0).or_insert_with(|| {
                            let v = Variable::new(next_var);
                            builder.declare_var(v, types::I64);
                            next_var += 1;
                            v
                        });
                        builder.def_var(d_var, val);
                    }
                    builder.ins().jump(end_block, &[]);
                    block_filled = true;
                }
                Instruction::BinaryOp {
                    dest,
                    op,
                    left,
                    right,
                } => {
                    let l_val = builder.use_var(*regs.get(&left.0).expect(
                        &format!("Reg R{} not found for BinaryOp left", left.0),
                    ));
                    let r_val = builder.use_var(*regs.get(&right.0).expect(
                        &format!("Reg R{} not found for BinaryOp right", right.0),
                    ));
                    let res = match op {
                        causm_core::BinaryOperator::Add => {
                            builder.ins().iadd(l_val, r_val)
                        }
                        causm_core::BinaryOperator::Sub => {
                            builder.ins().isub(l_val, r_val)
                        }
                        causm_core::BinaryOperator::Mul => {
                            builder.ins().imul(l_val, r_val)
                        }
                        causm_core::BinaryOperator::Gt => {
                            let cond = builder.ins().icmp(
                                IntCC::SignedGreaterThan,
                                l_val,
                                r_val,
                            );
                            let zero = builder.ins().iconst(types::I64, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            builder.ins().select(cond, one, zero)
                        }
                        causm_core::BinaryOperator::Lt => {
                            let cond = builder.ins().icmp(
                                IntCC::SignedLessThan,
                                l_val,
                                r_val,
                            );
                            let zero = builder.ins().iconst(types::I64, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            builder.ins().select(cond, one, zero)
                        }
                        _ => builder.ins().iconst(types::I64, 0), // TODO: other ops
                    };
                    let d_var = *regs.entry(dest.0).or_insert_with(|| {
                        let v = Variable::new(next_var);
                        builder.declare_var(v, types::I64);
                        next_var += 1;
                        v
                    });
                    builder.def_var(d_var, res);
                }
                Instruction::YieldPad => {
                    let mut sig = self.module.make_signature();
                    sig.params.push(AbiParam::new(types::I64));
                    let spin_pad_func = self
                        .module
                        .declare_function("spin_pad", Linkage::Import, &sig)
                        .unwrap();
                    let local_spin_pad = self
                        .module
                        .declare_func_in_func(spin_pad_func, &mut builder.func);
                    let padding = builder.ins().iconst(types::I64, 50); // Small fixed pad for sync
                    builder.ins().call(local_spin_pad, &[padding]);
                }
                _ => {}
            }
        }

        if !block_filled {
            builder.ins().jump(end_block, &[]);
        }

        builder.switch_to_block(end_block);

        // End timing and spin-pad if needed
        if let (Some(target_cycles), Some(start_var)) =
            (routine.taking_cycles, start_tsc_var)
        {
            let mut sig = self.module.make_signature();
            sig.returns.push(AbiParam::new(types::I64));
            let read_tsc_func = self
                .module
                .declare_function("read_tsc", Linkage::Import, &sig)
                .unwrap();
            let local_read_tsc = self
                .module
                .declare_func_in_func(read_tsc_func, &mut builder.func);

            // SpeedMicro: Compensate for the exit path overhead (estimated)
            let overhead = 220;
            let adjusted_target = target_cycles.saturating_sub(overhead);
            let target_val =
                builder.ins().iconst(types::I64, adjusted_target as i64);
            let start_tsc = builder.use_var(start_var);

            let loop_top = builder.create_block();
            let exit_block = builder.create_block();
            all_blocks.push(loop_top);
            all_blocks.push(exit_block);

            // SpeedMicro: Elastic Determinism
            let last_tsc_var = Variable::new(next_var);
            builder.declare_var(last_tsc_var, types::I64);
            next_var += 1;
            builder.def_var(last_tsc_var, start_tsc);

            let dynamic_start_var = Variable::new(next_var);
            builder.declare_var(dynamic_start_var, types::I64);
            builder.def_var(dynamic_start_var, start_tsc);

            builder.ins().jump(loop_top, &[]);
            builder.switch_to_block(loop_top);

            let last_tsc = builder.use_var(last_tsc_var);
            let current_start = builder.use_var(dynamic_start_var);

            let now_call = builder.ins().call(local_read_tsc, &[]);
            let now_tsc = builder.inst_results(now_call)[0];

            let delta = builder.ins().isub(now_tsc, last_tsc);
            let interrupt_threshold = builder.ins().iconst(types::I64, 5000);
            let is_interrupt = builder.ins().icmp(
                IntCC::SignedGreaterThan,
                delta,
                interrupt_threshold,
            );

            let interrupt_block = builder.create_block();
            let normal_block = builder.create_block();
            all_blocks.push(interrupt_block);
            all_blocks.push(normal_block);

            builder.ins().brif(
                is_interrupt,
                interrupt_block,
                &[],
                normal_block,
                &[],
            );

            builder.switch_to_block(interrupt_block);
            let new_start = builder.ins().iadd(current_start, delta);
            builder.def_var(dynamic_start_var, new_start);

            // SpeedMicro: Elastic Determinism
            // Call temporal_freeze(vm_ptr, delta)
            let mut freeze_sig = self.module.make_signature();
            freeze_sig.params.push(AbiParam::new(
                self.module.target_config().pointer_type(),
            ));
            freeze_sig.params.push(AbiParam::new(types::I64));
            let freeze_func = self
                .module
                .declare_function("temporal_freeze", Linkage::Import, &freeze_sig)
                .unwrap();
            let local_freeze =
                self.module.declare_func_in_func(freeze_func, &mut builder.func);
            let vm_ptr = builder.block_params(entry_block)[0];
            builder.ins().call(local_freeze, &[vm_ptr, delta]);

            builder.ins().jump(normal_block, &[]);

            builder.switch_to_block(normal_block);
            builder.def_var(last_tsc_var, now_tsc);

            let current_start_val = builder.use_var(dynamic_start_var);
            let elapsed_now = builder.ins().isub(now_tsc, current_start_val);

            let loop_cond =
                builder
                    .ins()
                    .icmp(IntCC::SignedLessThan, elapsed_now, target_val);
            builder
                .ins()
                .brif(loop_cond, loop_top, &[], exit_block, &[]);

            builder.switch_to_block(exit_block);
        }

        // Seal all blocks
        for block in &all_blocks {
            builder.seal_block(*block);
        }

        let return_val = if let Some(v) = regs.get(&0) {
            builder.use_var(*v)
        } else {
            builder.ins().iconst(types::I64, 0)
        };

        builder.ins().return_(&[return_val]);
        builder.finalize();

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| JitError::FinalizeError(e.to_string()))?;

        self.module
            .finalize_definitions()
            .map_err(|e| JitError::FinalizeError(e.to_string()))?;

        let code = self.module.get_finalized_function(func_id);
        Ok(code)
    }
}

/// SpeedMicro: Hardware Timing Helpers
pub mod hw_timing {
    #[cfg(target_arch = "x86_64")]
    pub extern "C" fn read_tsc() -> u64 {
        let mut aux: u32 = 0;
        unsafe { core::arch::x86_64::__rdtscp(&mut aux) }
    }

    #[cfg(target_arch = "aarch64")]
    pub extern "C" fn read_tsc() -> u64 {
        let mut val: u64;
        unsafe {
            std::arch::asm!("mrs {}, cntvct_el0", out(reg) val);
        }
        val
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub extern "C" fn read_tsc() -> u64 {
        // Fallback to system time if TSC is not available
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    pub extern "C" fn spin_pad(target_cycles: u64) {
        let start = read_tsc();
        while read_tsc() - start < target_cycles {
            std::hint::spin_loop();
        }
    }
}

/// SpeedMicro: Memory Pinning
pub fn pin_memory(ptr: *mut u8, len: usize) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    unsafe {
        if libc::mlock(ptr as *const libc::c_void, len) != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    #[cfg(windows)]
    unsafe {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::System::Memory::*;
            if VirtualLock(ptr as *const std::ffi::c_void, len) == 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    Ok(())
}
