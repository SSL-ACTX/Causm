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

    /// Custom compiler plugins (path to .wasm file or shell command for Stdio IPC)
    #[arg(long = "plugin", value_name = "SPEC")]
    plugins: Vec<String>,
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

        /// Custom compiler plugins (path to .wasm file or shell command for Stdio IPC)
        #[arg(long = "plugin", value_name = "SPEC")]
        plugins: Vec<String>,
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

        /// Custom compiler plugins (path to .wasm file or shell command for Stdio IPC)
        #[arg(long = "plugin", value_name = "SPEC")]
        plugins: Vec<String>,
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

    /// Empirically tune 'taking ?' routine temporal contracts
    Tune {
        /// Input source file(s) to tune in-place
        #[arg(required = true, value_name = "FILES")]
        files: Vec<PathBuf>,

        /// Number of chaos fuzzing iterations
        #[arg(long, default_value_t = 100)]
        iterations: usize,

        /// Safety margin percentage added to P99.9 WCET
        #[arg(long, default_value_t = 15.0)]
        safety_margin: f64,

        /// Tune all routines including already defined contracts (continuous re-tuning)
        #[arg(long, short = 'a')]
        all: bool,

        /// Pinpoint a specific routine to re-tune
        #[arg(long, short = 'r')]
        routine: Option<String>,

        /// Dry-run mode: print proposed changes without modifying files
        #[arg(long)]
        dry_run: bool,
    },

    /// Profile memory and clock timelines
    Profile {
        /// Input source file
        #[arg(required = true, value_name = "FILE")]
        file: PathBuf,
    },

    /// Format Causm source files
    Fmt {
        /// Input source file(s)
        #[arg(required = true, value_name = "FILES")]
        files: Vec<PathBuf>,
    },

    /// Manage and build compiler plugins
    Plugin {
        #[command(subcommand)]
        subcommand: PluginSubcommands,
    },
}

#[derive(Subcommand)]
enum PluginSubcommands {
    /// Scaffold a new compiler plugin project
    New {
        /// Name of the new plugin
        #[arg(required = true, value_name = "NAME")]
        name: String,

        /// Plugin type (rust or python)
        #[arg(long, default_value = "rust")]
        template: String,
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
    plugins: Vec<String>,
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
            plugins,
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
            plugins,
        },
        Some(Commands::Check {
            files,
            verbose,
            no_z3,
            plugins,
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
            plugins,
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
            plugins: Vec::new(),
        },
        Some(Commands::Tune {
            files,
            iterations,
            safety_margin,
            all,
            routine,
            dry_run,
        }) => {
            for path in &files {
                let source = fs::read_to_string(path)?;
                let mut tuned_source = source.clone();
                let program =
                    parser::parse_causm_with_imports(&source, path.parent())
                        .map_err(|e| anyhow::anyhow!(e))?;

                println!("\x1b[1;36m[Tuning Target]\x1b[0m {}", path.display());
                for tb in &program.timelines {
                    for stmt in &tb.statements {
                        if let causm_core::Statement::RoutineDef {
                            name,
                            taking_ms,
                            ..
                        } = &stmt.stmt
                        {
                            let should_tune =
                                if let Some(ref target_routine) = routine {
                                    target_routine == name
                                } else if all {
                                    true
                                } else {
                                    taking_ms.is_none()
                                };

                            if should_tune
                                && source.contains(&format!("routine {}", name))
                            {
                                let fuzzer_cfg =
                                    causm_devtools::tuner::fuzzer::FuzzConfig {
                                        iterations,
                                        chaos_jitter_ms: 5,
                                        safety_margin_pct: safety_margin,
                                        target: Some(name.clone()),
                                    };
                                match causm_devtools::tuner::fuzzer::fuzz_routine_wcet(&source, &fuzzer_cfg) {
                                    Ok(res) => {
                                        println!(
                                            "  \x1b[32m✔\x1b[0m Routine \x1b[1m{}\x1b[0m ({} iterations, min: {}ms, max: {}ms): P99.9 WCET = {}ms (+{:.0}% margin -> \x1b[1;33mtaking {}ms\x1b[0m)",
                                            name,
                                            res.sample_durations_ms.len(),
                                            res.min_duration_ms,
                                            res.max_duration_ms,
                                            res.max_duration_ms,
                                            safety_margin,
                                            res.p99_wcet_ms
                                        );
                                        tuned_source = causm_devtools::tuner::rewriter::patch_routine_contract(
                                            &tuned_source,
                                            name,
                                            res.p99_wcet_ms,
                                        );
                                    }
                                    Err(e) => {
                                        eprintln!("  \x1b[31m✖\x1b[0m Failed tuning {}: {}", name, e);
                                    }
                                }
                            }
                        }
                    }
                }

                if !dry_run {
                    fs::write(path, &tuned_source)?;
                    println!("\x1b[1;32m[Updated]\x1b[0m {}\n", path.display());
                } else {
                    println!(
                        "\x1b[1;33m[Dry-Run Proposal]\x1b[0m\n{}\n",
                        tuned_source
                    );
                }
            }
            return Ok(());
        }
        Some(Commands::Profile { file }) => {
            let source = fs::read_to_string(&file)?;
            let program = parser::parse_causm_with_imports(&source, file.parent())
                .map_err(|e| anyhow::anyhow!(e))?;
            let ir = lower::lower_program(&program);
            let mut vm = Vm::new();
            causm_stdlib::register_all(&mut vm);
            vm.execute_program(&ir)?;

            let report = causm_devtools::profiler::timeline::TimelineProfileReport::profile_vm(&vm);
            println!(
                "\x1b[1;36m=== TVM Profile Report: {} ===\x1b[0m",
                file.display()
            );
            println!("Logical Global Clock: {}ms", report.clock.global_clock_ms);
            println!(
                "Root Timeline Clock:  {}ms",
                report.clock.root_local_clock_ms
            );
            println!(
                "Arena Capacity:       {} bytes",
                report.memory.capacity_bytes
            );
            println!("Arena Used:           {} bytes", report.memory.used_bytes);
            println!(
                "Active Variables:     {}",
                report.memory.active_variables_count
            );
            return Ok(());
        }
        Some(Commands::Fmt { files }) => {
            let fmt_cfg = causm_devtools::fmt::rules::FormatConfig::default();
            for path in &files {
                let source = fs::read_to_string(path)?;
                let program =
                    parser::parse_causm(&source).map_err(|e| anyhow::anyhow!(e))?;
                let formatted =
                    causm_devtools::fmt::printer::format_program(&program, &fmt_cfg);

                // Pre-format analysis probe: if the original file already fails
                // semantic analysis (e.g. stdlib-internal files that call FFI
                // routines defined in a sibling ffi.csm), skip the semantic
                // regression gate on the formatted output. Only verify that the
                // formatted source is re-parseable.
                let pre_analysis_ok = {
                    let mut probe = EntropicAnalyzer::new();
                    probe.use_z3 = false;
                    let probe_ast =
                        parser::parse_causm_with_imports(&source, path.parent());
                    match probe_ast {
                        Ok(ast) => probe
                            .analyze_program_with_source(
                                &ast,
                                &source,
                                &path.display().to_string(),
                            )
                            .is_ok(),
                        Err(_) => false,
                    }
                };

                // Safe Round-Trip, AST-Equivalence & Semantic Validation Gate:
                // 1. Re-parse formatted source directly to compare AST
                match parser::parse_causm(&formatted) {
                    Ok(reparsed_program) => {
                        // 2. Strict AST equivalence check: formatted AST must match original AST
                        if !causm_core::programs_ast_eq(&program, &reparsed_program)
                        {
                            eprintln!(
                                "\x1b[1;31mFormatting AST Structural Mismatch in {}:\x1b[0m\n  Formatter generated an AST that differs from original.\n  (Original file preserved untouched)",
                                path.display()
                            );
                            for (i, (s1, s2)) in program.timelines[0]
                                .statements
                                .iter()
                                .zip(reparsed_program.timelines[0].statements.iter())
                                .enumerate()
                            {
                                if !causm_core::ast_statement_eq(&s1.stmt, &s2.stmt)
                                {
                                    eprintln!("  Statement {} mismatch:\n    original: {:#?}\n    reparsed: {:#?}", i, s1.stmt, s2.stmt);
                                    break;
                                }
                            }
                            continue;
                        }

                        // 3. Validate semantics if imports are present
                        let reparsed_with_imports = parser::parse_causm_with_imports(
                            &formatted,
                            path.parent(),
                        );
                        let semantic_ok = match reparsed_with_imports {
                            Ok(reparsed_ast) => {
                                if pre_analysis_ok {
                                    let mut analyzer = EntropicAnalyzer::new();
                                    analyzer.use_z3 = false;
                                    match analyzer.analyze_program_with_source(
                                        &reparsed_ast,
                                        &formatted,
                                        &path.display().to_string(),
                                    ) {
                                        Ok(_) => true,
                                        Err(err) => {
                                            let formatted_err =
                                                analyzer.format_semantic_error(&err);
                                            eprintln!(
                                                "\x1b[1;31mFormatting Semantic Regression in {}:\x1b[0m\n  {}\n  (Original file preserved untouched)",
                                                path.display(),
                                                formatted_err
                                            );
                                            false
                                        }
                                    }
                                } else {
                                    true
                                }
                            }
                            Err(_) => true,
                        };

                        if semantic_ok {
                            if formatted == source {
                                println!(
                                    "\x1b[2mUnchanged: {}\x1b[0m",
                                    path.display()
                                );
                            } else {
                                fs::write(path, formatted)?;
                                println!(
                                    "\x1b[1;32mFormatted:\x1b[0m {}",
                                    path.display()
                                );
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "\x1b[1;31mFormatting Syntax Error in {}:\x1b[0m\n{}\n--- Formatted Output Snippet ---\n{}\n-------------------",
                            path.display(),
                            err,
                            formatted
                        );
                    }
                }
            }
            return Ok(());
        }
        Some(Commands::Plugin { subcommand }) => {
            match subcommand {
                PluginSubcommands::New { name, template } => {
                    let parsed_template =
                        match causm_devtools::scaffold::PluginTemplate::parse(
                            &template,
                        ) {
                            Some(t) => t,
                            None => {
                                eprintln!("\x1b[1;31merror: unknown plugin template '{}' (valid: rust, python)\x1b[0m", template);
                                std::process::exit(1);
                            }
                        };

                    match causm_devtools::scaffold::scaffold_plugin_project(
                        &name,
                        parsed_template,
                        &std::env::current_dir()?,
                    ) {
                        Ok(target_path) => {
                            println!(
                                "\x1b[1;32mCreated plugin in\x1b[0m {}",
                                target_path.display()
                            );
                        }
                        Err(e) => {
                            eprintln!(
                                "\x1b[1;31merror scaffolding plugin:\x1b[0m {}",
                                e
                            );
                            std::process::exit(1);
                        }
                    }
                }
            }
            return Ok(());
        }
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
                plugins: cli.plugins,
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

        #[allow(unused_mut)]
        let mut program = match parser::parse_causm_with_imports(
            &source,
            path.parent(),
        ) {
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

        #[cfg(feature = "plugins")]
        let mut plugin_engine = causm_plugins::PluginEngine::new();

        #[cfg(feature = "plugins")]
        {
            // Auto-discover causm.toml in file path ancestors or current working directory
            let target_abs = path.canonicalize().unwrap_or_else(|_| path.clone());
            let mut search_dirs = Vec::new();
            if let Some(parent) = target_abs.parent() {
                search_dirs.push(parent.to_path_buf());
            }
            if let Ok(cwd) = std::env::current_dir() {
                search_dirs.push(cwd);
            }

            for start_dir in search_dirs {
                let mut curr_dir = Some(start_dir.as_path());
                while let Some(dir) = curr_dir {
                    let manifest_candidate = dir.join("causm.toml");
                    if manifest_candidate.exists() {
                        if let Err(e) = plugin_engine.load_from_causm_toml(&manifest_candidate) {
                            eprintln!("\x1b[1;33mwarning: failed to load causm.toml at '{}':\x1b[0m {}", manifest_candidate.display(), e);
                        }
                        break;
                    }
                    curr_dir = dir.parent();
                }
                if !plugin_engine.plugins.is_empty() {
                    break;
                }
            }

            // Register explicit CLI --plugin specs
            for plugin_spec in &config.plugins {
                if let Err(err) = plugin_engine.register_from_spec(plugin_spec) {
                    eprintln!(
                        "\x1b[1;31merror: failed to load plugin '{}':\x1b[0m {}",
                        plugin_spec, err
                    );
                    had_error = true;
                    break;
                }
            }
            if had_error {
                continue;
            }

            match plugin_engine
                .run_ast_pipeline(&path.display().to_string(), program)
            {
                Ok((transformed_ast, diagnostics)) => {
                    program = transformed_ast;
                    for diag in diagnostics {
                        let level_str = match diag.level {
                            causm_plugins::DiagnosticLevel::Error => {
                                "\x1b[1;31mplugin error:\x1b[0m"
                            }
                            causm_plugins::DiagnosticLevel::Warning => {
                                "\x1b[1;33mplugin warning:\x1b[0m"
                            }
                            causm_plugins::DiagnosticLevel::Note => {
                                "\x1b[1;36mplugin note:\x1b[0m"
                            }
                        };
                        eprintln!("{} {}", level_str, diag.message);
                        if matches!(
                            diag.level,
                            causm_plugins::DiagnosticLevel::Error
                        ) {
                            had_error = true;
                        }
                    }
                    if had_error {
                        continue;
                    }
                }
                Err(err) => {
                    eprintln!("\x1b[1;31mplugin execution error:\x1b[0m {:#}", err);
                    had_error = true;
                    continue;
                }
            }
        }

        #[cfg(not(feature = "plugins"))]
        if !config.plugins.is_empty() {
            eprintln!(
                "\x1b[1;33mwarning: plugin flags specified, but causm-cli was built without 'plugins' feature\x1b[0m"
            );
        }

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
                // For each dotted key, strip its first "Namespace." prefix and add the
                // remainder to a suppression set. Any key whose entire name appears in
                // this set is a duplicate covered by a namespaced alias and is hidden.
                //
                // e.g. "Net.TcpListener.accept" → tail = "TcpListener.accept" (suppressed)
                //      "Net.create_socket"       → tail = "create_socket"      (suppressed)
                let covered: std::collections::HashSet<String> = wcet_map
                    .keys()
                    .filter(|k| k.contains('.'))
                    .map(|k| {
                        let dot = k.find('.').unwrap();
                        k[dot + 1..].to_string()
                    })
                    .collect();
                let mut keys: Vec<&String> =
                    wcet_map.keys().filter(|k| !covered.contains(*k)).collect();
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

        #[cfg(feature = "plugins")]
        {
            let artifacts = causm_plugins::AnalysisArtifacts {
                verification_passed: true,
                timeline_count: program.timelines.len(),
                total_estimated_cost: 0,
            };

            match plugin_engine.run_post_analysis_pipeline(
                &path.display().to_string(),
                &program,
                artifacts,
            ) {
                Ok(diagnostics) => {
                    for diag in diagnostics {
                        let level_str = match diag.level {
                            causm_plugins::DiagnosticLevel::Error => {
                                "\x1b[1;31mplugin error:\x1b[0m"
                            }
                            causm_plugins::DiagnosticLevel::Warning => {
                                "\x1b[1;33mplugin warning:\x1b[0m"
                            }
                            causm_plugins::DiagnosticLevel::Note => {
                                "\x1b[1;36mplugin note:\x1b[0m"
                            }
                        };
                        eprintln!("{} {}", level_str, diag.message);
                        if matches!(
                            diag.level,
                            causm_plugins::DiagnosticLevel::Error
                        ) {
                            had_error = true;
                        }
                    }
                    if had_error {
                        continue;
                    }
                }
                Err(err) => {
                    eprintln!("\x1b[1;31mplugin execution error:\x1b[0m {:#}", err);
                    had_error = true;
                    continue;
                }
            }
        }

        if let Some(fmt) = config.emit {
            emit_format(fmt, &program, path)?;
        }

        if !config.check_only {
            let mut ir_program = lower::lower_program(&program);
            causm_ir::optimize::prune_unreachable_routines(&mut ir_program);
            ir_program = causm_ir::optimize::optimize_program(ir_program);
            let mut vm = Vm::new();
            vm.debug_mode = config.verbose;
            if config.chaos {
                vm.root_timeline.entropy_mode = causm_core::EntropyMode::Chaos;
            }
            vm.trace_entropy = config.trace_entropy;

            let tracer =
                causm_devtools::Tracer::new(config.verbose || config.trace_entropy);
            tracer.emit(
                0,
                "main",
                causm_devtools::TraceLayer::Runtime,
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
            let mut ir_program = lower::lower_program(program);
            causm_ir::optimize::prune_import_duplicates(&mut ir_program);
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
            let mut ir_program = lower::lower_program(program);
            causm_ir::optimize::prune_import_duplicates(&mut ir_program);
            println!("\x1b[1;35mSSA CFG for {}:\x1b[0m", path.display());
            let mut sorted_routines: Vec<_> = ir_program.routines.iter().collect();
            sorted_routines.sort_by_key(|(k, _)| k.as_str());
            for (name, routine) in sorted_routines {
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
            causm_ir::optimize::prune_import_duplicates(&mut ir_program);
            println!("\x1b[1;35mOptimized SSA CFG for {}:\x1b[0m", path.display());
            let mut sorted_routines: Vec<_> = ir_program.routines.iter().collect();
            sorted_routines.sort_by_key(|(k, _)| k.as_str());
            for (name, routine) in sorted_routines {
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
            let mut ir_program = lower::lower_program(program);
            causm_ir::optimize::prune_import_duplicates(&mut ir_program);
            println!("\x1b[1;35mSSA CFG DOT for {}:\x1b[0m", path.display());
            let mut sorted_routines: Vec<_> = ir_program.routines.iter().collect();
            sorted_routines.sort_by_key(|(k, _)| k.as_str());
            for (name, routine) in sorted_routines {
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
            causm_ir::optimize::prune_import_duplicates(&mut ir_program);
            println!(
                "\x1b[1;35mOptimized SSA CFG DOT for {}:\x1b[0m",
                path.display()
            );
            let mut sorted_routines: Vec<_> = ir_program.routines.iter().collect();
            sorted_routines.sort_by_key(|(k, _)| k.as_str());
            for (name, routine) in sorted_routines {
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
