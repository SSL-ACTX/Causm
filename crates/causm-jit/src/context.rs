use causm_ir::IrRoutine;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use thiserror::Error;

use crate::timing;

#[derive(Debug, Error)]
pub enum JitError {
    #[error("Compilation error: {0}")]
    CompileError(String),
    #[error("Finalization error: {0}")]
    FinalizeError(String),
}

pub struct CausmJit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
}

impl CausmJit {
    pub fn new() -> Result<Self, JitError> {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        let isa_builder = cranelift_native::builder().map_err(|msg| {
            JitError::CompileError(format!("host machine not supported: {}", msg))
        })?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| JitError::CompileError(e.to_string()))?;
        let mut builder =
            JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        builder.symbol("causm_read_tsc", timing::read_tsc as *const u8);
        builder.symbol("causm_spin_pad", timing::spin_pad as *const u8);

        // Native Math Intrinsics (ported from Lirien)
        builder.symbol("causm_sin", crate::intrinsics::causm_sin as *const u8);
        builder.symbol("causm_cos", crate::intrinsics::causm_cos as *const u8);
        builder.symbol("causm_tan", crate::intrinsics::causm_tan as *const u8);
        builder.symbol("causm_asin", crate::intrinsics::causm_asin as *const u8);
        builder.symbol("causm_acos", crate::intrinsics::causm_acos as *const u8);
        builder.symbol("causm_atan", crate::intrinsics::causm_atan as *const u8);
        builder.symbol("causm_exp", crate::intrinsics::causm_exp as *const u8);
        builder.symbol("causm_log", crate::intrinsics::causm_log as *const u8);
        builder.symbol("causm_pow", crate::intrinsics::causm_pow as *const u8);
        builder.symbol("causm_floor", crate::intrinsics::causm_floor as *const u8);
        builder.symbol("causm_ceil", crate::intrinsics::causm_ceil as *const u8);
        builder.symbol("causm_round", crate::intrinsics::causm_round as *const u8);
        builder.symbol("causm_sqrt", crate::intrinsics::causm_sqrt as *const u8);

        // Native Heap Memory Operations (ported from Lirien)
        builder.symbol("causm_alloc", crate::intrinsics::causm_alloc as *const u8);
        builder.symbol(
            "causm_dealloc",
            crate::intrinsics::causm_dealloc as *const u8,
        );
        builder.symbol(
            "causm_array_new",
            crate::memory::causm_array_new as *const u8,
        );
        builder.symbol(
            "causm_array_push",
            crate::memory::causm_array_push as *const u8,
        );
        builder.symbol(
            "causm_array_get",
            crate::memory::causm_array_get as *const u8,
        );
        builder.symbol(
            "causm_array_set",
            crate::memory::causm_array_set as *const u8,
        );
        builder.symbol(
            "causm_array_len",
            crate::memory::causm_array_len as *const u8,
        );

        // Native String Operations (ported from Lirien)
        builder.symbol("causm_str_new", crate::strings::causm_str_new as *const u8);
        builder.symbol("causm_str_len", crate::strings::causm_str_len as *const u8);
        builder.symbol(
            "causm_str_concat",
            crate::strings::causm_str_concat as *const u8,
        );
        builder.symbol(
            "causm_str_compare",
            crate::strings::causm_str_compare as *const u8,
        );
        builder.symbol(
            "causm_str_slice",
            crate::strings::causm_str_slice as *const u8,
        );
        builder.symbol(
            "causm_str_index",
            crate::strings::causm_str_index as *const u8,
        );

        let module = JITModule::new(builder);
        Ok(Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
        })
    }

    pub fn compile_routine(
        &mut self,
        name: &str,
        routine: &IrRoutine,
    ) -> Result<*const u8, JitError> {
        let cfg = causm_ir::CFG::from_flat_instructions(&routine.instructions);
        let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
        let ssa_cfg = transformer.transform();
        self.compile_ssa(name, &ssa_cfg, routine.params.len())
    }

    pub fn compile_ssa(
        &mut self,
        name: &str,
        ssa_cfg: &causm_ir::ssa::SsaCFG,
        param_count: usize,
    ) -> Result<*const u8, JitError> {
        let hash = crate::cache::hash_ssa_cfg(name, ssa_cfg);
        if let Some(cached) = crate::cache::lookup_compiled_routine(hash) {
            return Ok(cached.code_ptr);
        }

        self.module.clear_context(&mut self.ctx);

        let mut sig = self.module.make_signature();
        for _ in 0..param_count {
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

        crate::ssa_lower::compile_ssa_cfg(
            &mut builder,
            &mut self.module,
            ssa_cfg,
            param_count,
        );

        builder.seal_all_blocks();
        let config = self.module.isa().frontend_config();
        builder.finalize(config);

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| JitError::FinalizeError(e.to_string()))?;

        self.module
            .finalize_definitions()
            .map_err(|e| JitError::FinalizeError(e.to_string()))?;

        let code = self.module.get_finalized_function(func_id);
        crate::cache::store_compiled_routine(
            hash,
            crate::cache::CompiledRoutine {
                code_ptr: code,
                param_count,
            },
        );
        Ok(code)
    }
}
