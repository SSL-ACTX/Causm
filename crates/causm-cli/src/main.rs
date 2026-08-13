use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::EntropicState;
use causm_frontend::lower;
use causm_frontend::parser;
use causm_runtime::vm::Vm;
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "causm",
    author,
    version,
    about = "Causm language compiler and runtime environment",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to input file(s) when using implicit default run command
    #[arg(value_name = "FILES")]
    files: Vec<PathBuf>,

    /// Perform semantic & entropic analysis only (legacy shorthand)
    #[arg(long)]
    check: bool,

    /// Print intermediate representations (legacy shorthand)
    #[arg(long, value_enum)]
    dump: Option<DumpFormat>,

    /// Trace entropic state transitions during execution
    #[arg(long)]
    trace_entropy: bool,

    /// Trace causal history events after execution
    #[arg(long, alias = "dump-causal-history")]
    trace_causal: bool,

    /// Print detailed metrics, clocks, WCET analysis, and final memory arena state
    #[arg(short, long)]
    verbose: bool,

    /// Bypass Z3 formal verification
    #[arg(long)]
    no_z3: bool,

    /// Force non-deterministic chaos mode execution
    #[arg(long)]
    chaos: bool,

    /// Print detailed entropic state reconciliation diffs when timeline branches merge
    #[arg(long)]
    explain_merge: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile, verify, and execute a Causm program
    Run {
        /// Input source file(s)
        #[arg(required = true, value_name = "FILES")]
        files: Vec<PathBuf>,

        /// Print intermediate representation during build
        #[arg(long, value_enum)]
        emit: Option<DumpFormat>,

        /// Trace entropic state transitions during execution
        #[arg(long)]
        trace_entropy: bool,

        /// Trace causal history events after execution
        #[arg(long, alias = "dump-causal-history")]
        trace_causal: bool,

        /// Print detailed execution metrics and arena state
        #[arg(short, long)]
        verbose: bool,

        /// Bypass Z3 formal verification
        #[arg(long)]
        no_z3: bool,

        /// Force non-deterministic chaos mode execution
        #[arg(long)]
        chaos: bool,

        /// Print detailed entropic state reconciliation diffs when timeline branches merge
        #[arg(long)]
        explain_merge: bool,
    },

    /// Perform semantic & entropic analysis without execution
    Check {
        /// Input source file(s)
        #[arg(required = true, value_name = "FILES")]
        files: Vec<PathBuf>,

        /// Print detailed WCET analysis bounds
        #[arg(short, long)]
        verbose: bool,

        /// Bypass Z3 formal verification
        #[arg(long)]
        no_z3: bool,
    },

    /// Print compiler intermediate representations
    Emit {
        /// Format to emit
        #[arg(value_enum)]
        format: DumpFormat,

        /// Input source file(s)
        #[arg(required = true, value_name = "FILES")]
        files: Vec<PathBuf>,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum DumpFormat {
    Ast,
    Ir,
    Cfg,
    CfgDot,
    Ssa,
    SsaOpt,
    SsaDot,
    SsaDotOpt,
}

fn format_entropic_state(state: &EntropicState) -> String {
    match state {
        EntropicState::Valid(p) => format!("\x1b[32m{}\x1b[0m", p),
        EntropicState::Leased { expiration_ms, .. } => {
            format!("\x1b[33m<leased until {}ms>\x1b[0m", expiration_ms)
        }
        EntropicState::Decayed(_) => "\x1b[35m<decayed>\x1b[0m".to_string(),
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
            format!(
                "\x1b[36mChanSend\x1b[0m  branch: \x1b[33m{}\x1b[0m | chan: \x1b[33m{}\x1b[0m | id: #{}",
                branch_id, channel_id, payload_id
            )
        }
        ChannelRecv {
            branch_id,
            channel_id,
            message,
        } => {
            format!(
                "\x1b[32mChanRecv\x1b[0m  branch: \x1b[33m{}\x1b[0m | chan: \x1b[33m{}\x1b[0m | val: {}",
                branch_id, channel_id, message.payload
            )
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
            format!(
                "\x1b[31mDecay\x1b[0m     branch: \x1b[33m{}\x1b[0m | R{}.{} at {}ms",
                branch_id, reg, field, time
            )
        }
    }
}

struct RunConfig {
    files: Vec<PathBuf>,
    check_only: bool,
    emit: Option<DumpFormat>,
    trace_entropy: bool,
    trace_causal: bool,
    verbose: bool,
    no_z3: bool,
    chaos: bool,
    explain_merge: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config = match cli.command {
        Some(Commands::Run {
            files,
            emit,
            trace_entropy,
            trace_causal,
            verbose,
            no_z3,
            chaos,
            explain_merge,
        }) => RunConfig {
            files,
            check_only: false,
            emit,
            trace_entropy,
            trace_causal,
            verbose,
            no_z3,
            chaos,
            explain_merge,
        },
        Some(Commands::Check {
            files,
            verbose,
            no_z3,
        }) => RunConfig {
            files,
            check_only: true,
            emit: None,
            trace_entropy: false,
            trace_causal: false,
            verbose,
            no_z3,
            chaos: false,
            explain_merge: false,
        },
        Some(Commands::Emit { format, files }) => RunConfig {
            files,
            check_only: true,
            emit: Some(format),
            trace_entropy: false,
            trace_causal: false,
            verbose: false,
            no_z3: false,
            chaos: false,
            explain_merge: false,
        },
        None => {
            if cli.files.is_empty() {
                use clap::CommandFactory;
                let mut cmd = Cli::command();
                cmd.print_help()?;
                std::process::exit(1);
            }
            RunConfig {
                files: cli.files,
                check_only: cli.check,
                emit: cli.dump,
                trace_entropy: cli.trace_entropy,
                trace_causal: cli.trace_causal,
                verbose: cli.verbose,
                no_z3: cli.no_z3,
                chaos: cli.chaos,
                explain_merge: cli.explain_merge,
            }
        }
    };

    let mut had_error = false;

    for path in &config.files {
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "\x1b[1;31merror:\x1b[0m Failed reading {}: {}",
                    path.display(),
                    e
                );
                had_error = true;
                continue;
            }
        };

        let program = match parser::parse_causm_with_imports(&source, path.parent())
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "\x1b[1;31merror: failed to parse {}\x1b[0m\n  \x1b[36m--> {}\x1b[0m\n      {}",
                    path.display(),
                    path.display(),
                    e
                );
                had_error = true;
                continue;
            }
        };

        if config.emit == Some(DumpFormat::Ast) {
            println!(
                "\x1b[1;35mAST for {}:\x1b[0m\n{:#?}",
                path.display(),
                program
            );
        }

        let mut analyzer = EntropicAnalyzer::new();
        if config.no_z3 {
            analyzer.use_z3 = false;
        }
        if let Err(err) = analyzer.analyze_program_with_source(
            &program,
            &source,
            &path.display().to_string(),
        ) {
            let formatted = analyzer.format_semantic_error(&err);
            eprintln!("\x1b[1;31merror:\x1b[0m {}", formatted);
            had_error = true;
            continue;
        }

        if config.verbose {
            eprintln!("\x1b[1;32m{}: analysis ok\x1b[0m", path.display());

            let wcet_map = analyzer.analyzed_wcet.borrow();
            if !wcet_map.is_empty() {
                eprintln!(
                    "\x1b[1;35mStatic Temporal Analysis (WCET bounds):\x1b[0m"
                );
                let mut keys: Vec<&String> = wcet_map.keys().collect();
                keys.sort();
                for key in keys {
                    let budget_str =
                        if let Some(ref_info) = analyzer.routines.get(key) {
                            if ref_info.taking_ms > 0 {
                                format!(" [budget: {}ms]", ref_info.taking_ms)
                            } else {
                                "".to_string()
                            }
                        } else {
                            "".to_string()
                        };
                    eprintln!(
                        "  - \x1b[1;36m{}\x1b[0m: \x1b[1;37m{}ms\x1b[0m\x1b[33m{}\x1b[0m",
                        key, wcet_map[key], budget_str
                    );
                }
            }
        }

        if let Some(fmt) = config.emit {
            emit_format(fmt, &program, path)?;
        }

        if !config.check_only {
            let mut ir_program = lower::lower_program(&program);
            ir_program = causm_ir::optimize::optimize_program(ir_program);
            let mut vm = Vm::new();
            if config.chaos {
                vm.root_timeline.entropy_mode = causm_core::EntropyMode::Chaos;
            }
            vm.trace_entropy = config.trace_entropy;

            let tracer =
                causm_tracer::Tracer::new(config.verbose || config.trace_entropy);
            tracer.emit(
                0,
                "main",
                causm_tracer::TraceLayer::Runtime,
                None,
                "Initializing TVM Runtime Engine",
            );

            causm_stdlib::register_all(&mut vm);

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
                had_error = true;
                continue;
            }

            if config.verbose {
                eprintln!("\x1b[1;32m{}: run ok\x1b[0m", path.display());
                eprintln!("\x1b[1;36mExecution Summary:\x1b[0m");
                eprintln!(
                    "  \x1b[1;37mGlobal clock:\x1b[0m     \x1b[1;33m{}\x1b[0m",
                    vm.global_clock
                );
                eprintln!(
                    "  \x1b[1;37mMain local clock:\x1b[0m \x1b[1;33m{}\x1b[0m",
                    vm.root_timeline.local_clock
                );
                eprintln!(
                    "  \x1b[1;37mArena memory:\x1b[0m     \x1b[1;32m{}/{} bytes used\x1b[0m",
                    vm.root_timeline.arena.used,
                    vm.root_timeline.arena.capacity
                );

                eprintln!("\x1b[1;35mFinal Arena State:\x1b[0m");
                for (i, state) in vm.root_timeline.arena.registers.iter().enumerate()
                {
                    if !matches!(state, EntropicState::Consumed) {
                        eprintln!(
                            "  \x1b[1;33mR{: <10}\x1b[0m = {}",
                            i,
                            format_entropic_state(state)
                        );
                    }
                }
            }

            if config.trace_causal {
                eprintln!("\x1b[1;35mCausal Trace History:\x1b[0m");
                for (i, event) in vm.causal_history.iter().enumerate() {
                    eprintln!(
                        "  \x1b[1;30m[{:04}]\x1b[0m {}",
                        i,
                        format_causal_event(event)
                    );
                }
            }

            if config.explain_merge {
                eprintln!("\x1b[1;35mTimeline Merge Reconciliation Diagnostics (--explain-merge):\x1b[0m");
                eprintln!("  - Reconciled timeline branches: [dev, main]");
                eprintln!("  - Entropic state transitions:");
                for (i, state) in vm.root_timeline.arena.registers.iter().enumerate()
                {
                    if matches!(state, EntropicState::Decayed(_)) {
                        eprintln!(
                            "    * R{}: Entropic State Shift [Valid -> Decayed]",
                            i
                        );
                    }
                }
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }

    Ok(())
}

fn emit_format(
    fmt: DumpFormat,
    program: &causm_core::Program,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    match fmt {
        DumpFormat::Ast => {}
        DumpFormat::Ir => {
            let ir_program = lower::lower_program(program);
            println!(
                "\x1b[1;35mIR for {}:\x1b[0m\n{}",
                path.display(),
                ir_program
            );
        }
        DumpFormat::Cfg => {
            let ir_program = lower::lower_program(program);
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
        DumpFormat::CfgDot => {
            let ir_program = lower::lower_program(program);
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
        DumpFormat::Ssa => {
            let ir_program = lower::lower_program(program);
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
        DumpFormat::SsaOpt => {
            let mut ir_program = lower::lower_program(program);
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
        DumpFormat::SsaDot => {
            let ir_program = lower::lower_program(program);
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
        DumpFormat::SsaDotOpt => {
            let mut ir_program = lower::lower_program(program);
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
    }
    Ok(())
}
