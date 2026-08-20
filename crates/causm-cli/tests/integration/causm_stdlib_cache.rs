use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;
use causm_stdlib::archive::CsaArchive;

#[test]
fn test_csa_archive_roundtrip_serialization() -> anyhow::Result<()> {
    let archive = CsaArchive::build_standard_archive();
    assert!(archive.modules.contains_key("std/core"));
    assert!(archive.modules.contains_key("std/json"));
    assert!(archive.modules.contains_key("std/http"));
    assert!(archive.modules.contains_key("std/collection"));

    let serialized = archive.to_bytes()?;
    assert!(!serialized.is_empty());
    assert_eq!(&serialized[0..4], b"CSMA");

    let deserialized = CsaArchive::from_bytes(&serialized)?;
    assert_eq!(deserialized.magic, *b"CSMA");
    assert_eq!(deserialized.version, 2);
    assert_eq!(
        deserialized.get_module("std/core/types"),
        archive.get_module("std/core/types")
    );

    // Test inserting and retrieving pre-compiled IrRoutine binary bytecode
    let mut bytecode_archive = archive;
    let dummy_routine = causm_ir::IrRoutine {
        params: vec![],
        return_type: causm_core::types::Type::Integer,
        taking_ms: Some(10),
        foreign_binding: None,
        instructions: vec![
            causm_ir::Instruction::ConstInt {
                dest: causm_ir::Reg(0),
                value: 42,
            },
            causm_ir::Instruction::Return {
                src: Some(causm_ir::Reg(0)),
            },
        ],
        spans: vec![None, None],
    };
    bytecode_archive.insert_bytecode_routine(
        "std/core",
        "Core.constant_42",
        dummy_routine.clone(),
    );
    let bytecode_bytes = bytecode_archive.to_bytes()?;
    let loaded_bytecode_archive = CsaArchive::from_bytes(&bytecode_bytes)?;
    let routines = loaded_bytecode_archive
        .get_bytecode_routines("std/core")
        .expect("routines found");
    assert_eq!(routines.len(), 1);
    assert_eq!(routines[0].name, "Core.constant_42");
    assert_eq!(routines[0].routine, dummy_routine);

    Ok(())
}

#[test]
fn test_stdlib_ast_cache_ingestion_and_execution() -> anyhow::Result<()> {
    let source = r#"
    import "std/core" as Core
    import "std/json" as Json
    import "std/collection" as Collection

    @0ms: {
        let opt = Option::Some(42)
        let val = opt.unwrap_or(0)

        let parsed = Json.parse("{\"status\":\"ok\",\"code\":200}")
        let code = parsed.get_int("code", 0)

        let mut st = Collection.Stack.new(4)
        st = st.push(999)
        let top = st.peek()
    }
    "#;

    // First parse populates STDLIB_AST_CACHE
    let program1 = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer1 = EntropicAnalyzer::new();
    analyzer1.analyze_program(&program1)?;

    // Second parse hits STDLIB_AST_CACHE directly (microsecond latency)
    let program2 = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer2 = EntropicAnalyzer::new();
    analyzer2.analyze_program(&program2)?;

    let ir = causm_frontend::lower::lower_program(&program2);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let val_reg = ir.symbols.get("val").expect("val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(val_reg),
        Some(Payload::Integer(42))
    );

    let code_reg = ir.symbols.get("code").expect("code not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(code_reg),
        Some(Payload::Integer(200))
    );

    let top_reg = ir.symbols.get("top").expect("top not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(top_reg),
        Some(Payload::Integer(999))
    );

    Ok(())
}

#[test]
fn test_csa_archive_disk_cache_persistence_and_checksum_invalidation(
) -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join("causm_test_cache");
    let test_csa_path = temp_dir.join("test_std.csa");

    let archive = CsaArchive::build_standard_archive();
    let saved_path = archive.save_to_disk(Some(&test_csa_path))?;
    assert!(saved_path.exists());

    let loaded = CsaArchive::load_from_disk(Some(&test_csa_path))?;
    assert_eq!(loaded.magic, *b"CSMA");
    assert!(loaded.verify_checksums());
    assert_eq!(
        loaded.get_module("std/json"),
        archive.get_module("std/json")
    );

    // Verify corruption / invalidation detection
    let mut corrupt_archive = loaded.clone();
    if let Some(entry) = corrupt_archive.modules.get_mut("std/json") {
        entry.bytecode = vec![0xDE, 0xAD, 0xBE, 0xEF];
    }
    assert!(!corrupt_archive.verify_checksums());

    let _ = std::fs::remove_dir_all(temp_dir);
    Ok(())
}

#[test]
fn test_benchmark_stdlib_caching_speedup() -> anyhow::Result<()> {
    let source = r#"
    import "std/core" as Core
    import "std/json" as Json
    import "std/http" as Http
    import "std/collection" as Collection
    import "std/process" as Process

    @0ms: {
        let opt = Option::Some(100)
        let val = opt.unwrap_or(0)
    }
    "#;

    // Warm-up and Cold vs Warm timing
    let start_cold = std::time::Instant::now();
    let prog1 = parser::parse_causm_with_imports(source, None)?;
    let cold_duration = start_cold.elapsed();

    let start_warm = std::time::Instant::now();
    let prog2 = parser::parse_causm_with_imports(source, None)?;
    let warm_duration = start_warm.elapsed();

    println!("\n=== Stdlib Caching Empirical Benchmark ===");
    println!(
        "Cold Parse + Transitive Stdlib Expansions: {:?}",
        cold_duration
    );
    println!(
        "Warm Parse (Cache Hit):                   {:?}",
        warm_duration
    );

    // Multiple iterations benchmark
    let iterations = 10;
    let start_multi = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = parser::parse_causm_with_imports(source, None)?;
    }
    let avg_warm = start_multi.elapsed() / iterations;
    println!(
        "Average Cached Import Time ({} iters):     {:?}",
        iterations, avg_warm
    );

    assert_eq!(prog1.timelines.len(), prog2.timelines.len());
    println!("===========================================\n");

    Ok(())
}
