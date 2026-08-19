use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_collection_array_operations() -> anyhow::Result<()> {
    let source = r#"
    import "std/collection" as Collection

    @0ms: {
        let a = [1, 2, 3]
        let b = [4, 5]
        let joined = Collection.Array.concat(a, b)
        let joined_len = len(joined)

        let mut dest = [0; 5]
        let copied = Collection.Array.copy(a, 0, dest, 1, 3)

        let has_two = Collection.Array.contains(a, 2)
        let has_ten = Collection.Array.contains(a, 10)
        let idx_three = Collection.Array.find(a, 3)

        let rev = Collection.Array.reverse(a)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    // Dump SSA for debugging if needed
    for (i, block) in opt_ir.blocks.iter().enumerate() {
        let cfg = causm_ir::CFG::from_flat_instructions(&block.instructions);
        let ssa = causm_ir::ssa::SsaTransformer::new(cfg).transform();
        println!("--- Block {} SSA CFG ---\n{}", i, ssa);
    }

    let mut vm = Vm::new();
    vm.trace_entropy = true;
    let tracer = causm_devtools::Tracer::new(true);
    tracer.emit(
        0,
        "main",
        causm_devtools::TraceLayer::Runtime,
        None,
        "Running Collection Tests",
    );
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let joined_reg = ir.symbols.get("joined").expect("joined not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(joined_reg),
        Some(Payload::Array(vec![
            Payload::Integer(1),
            Payload::Integer(2),
            Payload::Integer(3),
            Payload::Integer(4),
            Payload::Integer(5),
        ]))
    );

    let joined_len_reg = ir
        .symbols
        .get("joined_len")
        .expect("joined_len not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(joined_len_reg),
        Some(Payload::Integer(5))
    );

    let copied_reg = ir.symbols.get("copied").expect("copied not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(copied_reg),
        Some(Payload::Array(vec![
            Payload::Integer(0),
            Payload::Integer(1),
            Payload::Integer(2),
            Payload::Integer(3),
            Payload::Integer(0),
        ]))
    );

    let has_two_reg = ir.symbols.get("has_two").expect("has_two not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_two_reg),
        Some(Payload::Bool(true))
    );

    let has_ten_reg = ir.symbols.get("has_ten").expect("has_ten not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_ten_reg),
        Some(Payload::Bool(false))
    );

    let idx_three_reg = ir.symbols.get("idx_three").expect("idx_three not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(idx_three_reg),
        Some(Payload::Integer(2))
    );

    let rev_reg = ir.symbols.get("rev").expect("rev not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(rev_reg),
        Some(Payload::Array(vec![
            Payload::Integer(3),
            Payload::Integer(2),
            Payload::Integer(1),
        ]))
    );

    Ok(())
}

#[test]
fn test_collection_dynamic_buffer() -> anyhow::Result<()> {
    let source = r#"
    import "std/collection" as Collection

    @0ms: {
        let mut buf = Collection.Buffer.new(4)
        let buf = Collection.Buffer.append(buf, 65)
        let buf = Collection.Buffer.append(buf, 66)
        let buf = Collection.Buffer.append_slice(buf, [67, 68, 69], 3)

        let slice = Collection.Buffer.as_slice(buf)
        let slice_len = len(slice)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    for (i, block) in opt_ir.blocks.iter().enumerate() {
        let cfg = causm_ir::CFG::from_flat_instructions(&block.instructions);
        let ssa = causm_ir::ssa::SsaTransformer::new(cfg).transform();
        println!("--- Block {} SSA CFG ---\n{}", i, ssa);
    }

    let mut vm = Vm::new();
    vm.trace_entropy = true;
    let tracer = causm_devtools::Tracer::new(true);
    tracer.emit(
        0,
        "main",
        causm_devtools::TraceLayer::Runtime,
        None,
        "Running Collection Buffer Tests",
    );
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let slice_reg = ir.symbols.get("slice").expect("slice not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(slice_reg),
        Some(Payload::Array(vec![
            Payload::Integer(65),
            Payload::Integer(66),
            Payload::Integer(67),
            Payload::Integer(68),
            Payload::Integer(69),
        ]))
    );

    let slice_len_reg = ir.symbols.get("slice_len").expect("slice_len not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(slice_len_reg),
        Some(Payload::Integer(5))
    );

    Ok(())
}
