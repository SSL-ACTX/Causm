use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::EntropicState;
use causm_frontend::lower;
use causm_frontend::parser;
use causm_runtime::vm::Vm;
use std::env;
use std::fs;
use std::path::PathBuf;

fn usage(program: &str) {
    eprintln!("\x1b[1;36mCausm Runtime Environment\x1b[0m v0.1.0");
    eprintln!(
        "\x1b[1;33mUsage:\x1b[0m {} [options] <file1.csm> [file2.csm ...]\n",
        program
    );
    eprintln!("\x1b[1mOptions:\x1b[0m");
    eprintln!("  \x1b[36m--check\x1b[0m                 Perform semantic & entropic analysis only");
    eprintln!("  \x1b[36m--run\x1b[0m                   Execute program after analysis (default)");
    eprintln!("  \x1b[36m--dump <format>\x1b[0m         Print compiler representation and continue");
    eprintln!("                          Formats: ast, ir, cfg, cfg-dot, ssa, ssa-opt, ssa-dot, ssa-dot-opt");
    eprintln!("  \x1b[36m--trace-entropy\x1b[0m         Trace entropic state transitions during execution");
    eprintln!("  \x1b[36m--dump-causal-history\x1b[0m   Print causal trace events after execution");
    eprintln!("  \x1b[36m--help\x1b[0m                  Display this help message");
}

fn format_entropic_state(state: &EntropicState) -> String {
    match state {
        EntropicState::Valid(p) => format!("\x1b[32m{}\x1b[0m", p),
        EntropicState::Leased { expiration_ms, .. } => {
            format!("\x1b[33m<leased until {}ms>\x1b[0m", expiration_ms)
        }
        EntropicState::Decayed(_) => "\x1b[90m<decayed>\x1b[0m".to_string(),
        EntropicState::Pending(_) => "\x1b[35m<pending>\x1b[0m".to_string(),
        EntropicState::Consumed => "\x1b[31m<consumed>\x1b[0m".to_string(),
    }
}

fn format_causal_event(event: &causm_runtime::vm::state::CausalEvent) -> String {
    use causm_runtime::vm::state::CausalEvent::*;
    match event {
        ChannelSend {
            branch_id,
            channel_id,
            payload_id,
        } => {
            format!("\x1b[36mChanSend\x1b[0m  branch: \x1b[33m{}\x1b[0m | chan: \x1b[33m{}\x1b[0m | id: #{}", branch_id, channel_id, payload_id)
        }
        ChannelRecv {
            branch_id,
            channel_id,
            message,
        } => {
            format!("\x1b[32mChanRecv\x1b[0m  branch: \x1b[33m{}\x1b[0m | chan: \x1b[33m{}\x1b[0m | val: {}", branch_id, channel_id, message.payload)
        }
        InterBranchMove {
            source_branch,
            target_branch,
            reg,
            ..
        } => {
            format!(
                "\x1b[35mMove\x1b[0m      {} ➔ {} | R{}",
                source_branch, target_branch, reg
            )
        }
        Decay {
            branch_id,
            reg,
            field,
            time,
        } => {
            format!("\x1b[31mDecay\x1b[0m     branch: \x1b[33m{}\x1b[0m | R{}.{} at {}ms", branch_id, reg, field, time)
        }
    }
}

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage(&env::args().next().unwrap_or_else(|| "causm".to_string()));
        std::process::exit(1);
    }

    let mut check_only = false;
    let mut run_program = false;
    let mut dump_ast = false;
    let mut dump_ir = false;
    let mut dump_cfg = false;
    let mut dump_cfg_dot = false;
    let mut dump_ssa = false;
    let mut dump_ssa_opt = false;
    let mut dump_ssa_dot = false;
    let mut dump_ssa_dot_opt = false;
    let mut trace_entropy = false;
    let mut dump_causal_history = false;

    while let Some(arg) = args.first() {
        if arg == "--help" || arg == "-h" {
            usage(&env::args().next().unwrap_or_else(|| "causm".to_string()));
            std::process::exit(0);
        }
        if arg == "--check" {
            check_only = true;
            args.remove(0);
            continue;
        }
        if arg == "--run" {
            run_program = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump" {
            args.remove(0);
            if let Some(format) = args.first() {
                match format.as_str() {
                    "ast" => dump_ast = true,
                    "ir" => dump_ir = true,
                    "cfg" => dump_cfg = true,
                    "cfg-dot" => dump_cfg_dot = true,
                    "ssa" => dump_ssa = true,
                    "ssa-opt" => dump_ssa_opt = true,
                    "ssa-dot" => dump_ssa_dot = true,
                    "ssa-dot-opt" => dump_ssa_dot_opt = true,
                    other => {
                        eprintln!("\x1b[1;31merror:\x1b[0m Unknown dump format '{}'. Valid formats are: ast, ir, cfg, cfg-dot, ssa, ssa-opt, ssa-dot, ssa-dot-opt.", other);
                        std::process::exit(1);
                    }
                }
                args.remove(0);
                continue;
            } else {
                eprintln!("\x1b[1;31merror:\x1b[0m --dump requires a format argument (ast, ir, cfg, cfg-dot, ssa, ssa-opt, ssa-dot, ssa-dot-opt).");
                std::process::exit(1);
            }
        }
        if arg == "--dump-ast" {
            dump_ast = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-ir" {
            dump_ir = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-cfg" {
            dump_cfg = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-cfg-dot" {
            dump_cfg_dot = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-ssa" {
            dump_ssa = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-ssa-opt" {
            dump_ssa_opt = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-ssa-dot" {
            dump_ssa_dot = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-ssa-dot-opt" {
            dump_ssa_dot_opt = true;
            args.remove(0);
            continue;
        }
        if arg == "--trace-entropy" {
            trace_entropy = true;
            args.remove(0);
            continue;
        }
        if arg == "--dump-causal-history" {
            dump_causal_history = true;
            args.remove(0);
            continue;
        }
        break;
    }

    if !check_only && !run_program {
        run_program = true;
    }

    if args.is_empty() {
        usage(&env::args().next().unwrap_or_else(|| "causm".to_string()));
        std::process::exit(1);
    }

    for file in args {
        let path = PathBuf::from(&file);
        let source = fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!("Failed reading {}: {}", path.display(), e)
        })?;

        let program = match parser::parse_causm(&source) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "\x1b[1;31merror: failed to parse {}\x1b[0m\n  \x1b[90m--> {}\x1b[0m\n      {}",
                    path.display(),
                    path.display(),
                    e
                );
                continue;
            }
        };

        if dump_ast {
            println!(
                "\x1b[1;35mAST for {}:\x1b[0m\n{:#?}",
                path.display(),
                program
            );
        }

        let mut analyzer = EntropicAnalyzer::new();
        if let Err(err) = analyzer.analyze_program_with_source(
            &program,
            &source,
            &path.display().to_string(),
        ) {
            let formatted = analyzer.format_semantic_error(&err);
            eprintln!("\x1b[1;31merror:\x1b[0m {}", formatted);
            continue;
        }

        println!("\x1b[1;32m{}: analysis ok\x1b[0m", path.display());

        let wcet_map = analyzer.analyzed_wcet.borrow();
        if !wcet_map.is_empty() {
            println!("\x1b[1;35mStatic Temporal Analysis (WCET bounds):\x1b[0m");
            let mut keys: Vec<&String> = wcet_map.keys().collect();
            keys.sort();
            for key in keys {
                // If it is a routine, check if it has a taking_ms budget
                let budget_str = if let Some(ref_info) = analyzer.routines.get(key) {
                    if ref_info.taking_ms > 0 {
                        format!(" [budget: {}ms]", ref_info.taking_ms)
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                };
                println!(
                    "  - \x1b[36m{}\x1b[0m: {}ms{}",
                    key, wcet_map[key], budget_str
                );
            }
        }

        if dump_ir {
            let ir_program = lower::lower_program(&program);
            println!(
                "\x1b[1;35mIR for {}:\x1b[0m\n{}",
                path.display(),
                ir_program
            );
        }

        if dump_cfg {
            let ir_program = lower::lower_program(&program);
            println!("\x1b[1;35mCFG for {}:\x1b[0m", path.display());
            for (name, routine) in &ir_program.routines {
                let cfg = causm_ir::cfg::CFG::from_flat_instructions(
                    &routine.instructions,
                );
                println!("  \x1b[1;33mRoutine {}\x1b[0m:", name);
                println!("{}", cfg);
            }
            for block in &ir_program.blocks {
                let cfg =
                    causm_ir::cfg::CFG::from_flat_instructions(&block.instructions);
                println!("  \x1b[1;33mBlock @{}\x1b[0m:", block.time);
                println!("{}", cfg);
            }
        }

        if dump_ssa {
            let ir_program = lower::lower_program(&program);
            println!("\x1b[1;35mSSA CFG for {}:\x1b[0m", path.display());
            for (name, routine) in &ir_program.routines {
                let cfg = causm_ir::cfg::CFG::from_flat_instructions(
                    &routine.instructions,
                );
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("  \x1b[1;33mRoutine {}\x1b[0m:", name);
                println!("{}", ssa_cfg);
            }
            for block in &ir_program.blocks {
                let cfg =
                    causm_ir::cfg::CFG::from_flat_instructions(&block.instructions);
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("  \x1b[1;33mBlock @{}\x1b[0m:", block.time);
                println!("{}", ssa_cfg);
            }
        }

        if dump_cfg_dot {
            let ir_program = lower::lower_program(&program);
            println!("\x1b[1;35mCFG DOT for {}:\x1b[0m", path.display());
            for (name, routine) in &ir_program.routines {
                let cfg = causm_ir::cfg::CFG::from_flat_instructions(
                    &routine.instructions,
                );
                println!("// Routine {}", name);
                println!("{}", cfg.to_dot());
            }
            for block in &ir_program.blocks {
                let cfg =
                    causm_ir::cfg::CFG::from_flat_instructions(&block.instructions);
                println!("// Block @{}", block.time);
                println!("{}", cfg.to_dot());
            }
        }

        if dump_ssa_opt {
            let mut ir_program = lower::lower_program(&program);
            ir_program = causm_ir::optimize::optimize_program(ir_program);
            println!("\x1b[1;35mOptimized SSA CFG for {}:\x1b[0m", path.display());
            for (name, routine) in &ir_program.routines {
                let cfg = causm_ir::cfg::CFG::from_flat_instructions(
                    &routine.instructions,
                );
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("  \x1b[1;33mRoutine {}\x1b[0m:", name);
                println!("{}", ssa_cfg);
            }
            for block in &ir_program.blocks {
                let cfg =
                    causm_ir::cfg::CFG::from_flat_instructions(&block.instructions);
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("  \x1b[1;33mBlock @{}\x1b[0m:", block.time);
                println!("{}", ssa_cfg);
            }
        }

        if dump_ssa_dot {
            let ir_program = lower::lower_program(&program);
            println!("\x1b[1;35mSSA CFG DOT for {}:\x1b[0m", path.display());
            for (name, routine) in &ir_program.routines {
                let cfg = causm_ir::cfg::CFG::from_flat_instructions(
                    &routine.instructions,
                );
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("// Routine {}", name);
                println!("{}", ssa_cfg.to_dot());
            }
            for block in &ir_program.blocks {
                let cfg =
                    causm_ir::cfg::CFG::from_flat_instructions(&block.instructions);
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("// Block @{}", block.time);
                println!("{}", ssa_cfg.to_dot());
            }
        }

        if dump_ssa_dot_opt {
            let mut ir_program = lower::lower_program(&program);
            ir_program = causm_ir::optimize::optimize_program(ir_program);
            println!(
                "\x1b[1;35mOptimized SSA CFG DOT for {}:\x1b[0m",
                path.display()
            );
            for (name, routine) in &ir_program.routines {
                let cfg = causm_ir::cfg::CFG::from_flat_instructions(
                    &routine.instructions,
                );
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("// Routine {}", name);
                println!("{}", ssa_cfg.to_dot());
            }
            for block in &ir_program.blocks {
                let cfg =
                    causm_ir::cfg::CFG::from_flat_instructions(&block.instructions);
                let transformer = causm_ir::ssa::SsaTransformer::new(cfg);
                let ssa_cfg = transformer.transform();
                println!("// Block @{}", block.time);
                println!("{}", ssa_cfg.to_dot());
            }
        }

        if run_program {
            let mut ir_program = lower::lower_program(&program);
            ir_program = causm_ir::optimize::optimize_program(ir_program);
            let mut vm = Vm::new();
            vm.trace_entropy = trace_entropy;
            vm.register_capability("System.Log", |params| {
                if let Some(msg) = params.get("message") {
                    println!("\x1b[1;34m[System.Log]\x1b[0m {}", msg);
                }
                Ok(())
            });
            vm.register_capability("System.NetworkFetch", |params| {
                if let Some(url) = params.get("url") {
                    println!(
                        "\x1b[1;34m[System.NetworkFetch]\x1b[0m Fetching {}",
                        url
                    );
                }
                Ok(())
            });

            if let Err(e) = vm.execute_program(&ir_program) {
                let location_info = if let Some(ref span) = vm.current_span {
                    let before = &source[..span.start];
                    let ln = before.lines().count() + 1;
                    let col = before
                        .lines()
                        .last()
                        .map(|line| line.len() + 1)
                        .unwrap_or(1);
                    format!(" at {}:{}:{}", path.display(), ln, col)
                } else {
                    format!(" in {}", path.display())
                };
                eprintln!(
                    "\x1b[1;31merror: runtime failure{}\x1b[0m\n  cause: {}",
                    location_info, e
                );
            }

            println!("\x1b[1;32m{}: run ok\x1b[0m", path.display());
            println!("\x1b[1;36mExecution Summary:\x1b[0m");
            println!(
                "  \x1b[90mGlobal clock:\x1b[0m     \x1b[1;33m{}\x1b[0m",
                vm.global_clock
            );
            println!(
                "  \x1b[90mMain local clock:\x1b[0m \x1b[1;33m{}\x1b[0m",
                vm.root_timeline.local_clock
            );
            println!(
                "  \x1b[90mArena memory:\x1b[0m     \x1b[1;32m{}/{} bytes used\x1b[0m",
                vm.root_timeline.arena.used,
                vm.root_timeline.arena.capacity
            );

            println!("\x1b[1;35mFinal Arena State:\x1b[0m");
            for (i, state) in vm.root_timeline.arena.registers.iter().enumerate() {
                if !matches!(state, EntropicState::Consumed) {
                    println!(
                        "  \x1b[1;33mR{: <10}\x1b[0m = {}",
                        i,
                        format_entropic_state(state)
                    );
                }
            }

            if dump_causal_history {
                println!("\x1b[1;35mCausal Trace History:\x1b[0m");
                for (i, event) in vm.causal_history.iter().enumerate() {
                    println!(
                        "  \x1b[1;30m[{:04}]\x1b[0m {}",
                        i,
                        format_causal_event(event)
                    );
                }
            }
        }
    }

    Ok(())
}
