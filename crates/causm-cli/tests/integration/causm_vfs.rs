use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_vfs_in_memory_ramdisk_lifecycle() -> anyhow::Result<()> {
    let source = r#"
    from "std/fs" import *

    @0ms: {
        let path = "causm_vfs_native_temp.txt"
        let f = create_file(path)
        let written = write_all(f, "CAUSM_VFS_PAYLOAD", 17)
        let exists = file_exists(path)
        let size = file_size(path)
        let removed = remove_file(path)
        let exists_after = file_exists(path)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let written_reg = ir.symbols.get("written").expect("written not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(written_reg),
        Some(Payload::Integer(17))
    );

    let exists_reg = ir.symbols.get("exists").expect("exists not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(exists_reg),
        Some(Payload::Bool(true))
    );

    let size_reg = ir.symbols.get("size").expect("size not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(size_reg),
        Some(Payload::Integer(17))
    );

    let removed_reg = ir.symbols.get("removed").expect("removed not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(removed_reg),
        Some(Payload::Integer(0))
    );

    let exists_after_reg = ir
        .symbols
        .get("exists_after")
        .expect("exists_after not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(exists_after_reg),
        Some(Payload::Bool(false))
    );

    Ok(())
}

#[test]
fn test_vfs_isolate_sandboxed_zero_ffi_execution() -> anyhow::Result<()> {
    let source = r#"
    from "std/fs" import *

    @0ms: {
        isolate sandboxed_vfs {
            require System.WASI
            enable memory(64KB)
            enable cpu(500ms)

            let path = "causm_vfs_isolated_temp.txt"
            let f = create_file(path)
            let written = write_all(f, "ISOLATED_SANDBOX_VFS", 20)
            let exists = file_exists(path)
            let size = file_size(path)
            let removed = remove_file(path)
        }
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let written_reg = ir.symbols.get("written").expect("written not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(written_reg),
        Some(Payload::Integer(20))
    );

    let exists_reg = ir.symbols.get("exists").expect("exists not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(exists_reg),
        Some(Payload::Bool(true))
    );

    let size_reg = ir.symbols.get("size").expect("size not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(size_reg),
        Some(Payload::Integer(20))
    );

    let removed_reg = ir.symbols.get("removed").expect("removed not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(removed_reg),
        Some(Payload::Integer(0))
    );

    Ok(())
}
