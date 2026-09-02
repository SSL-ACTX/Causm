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

#[test]
fn test_collection_stack_operations() -> anyhow::Result<()> {
    let source = r#"
    import "std/collection" as Collection

    @0ms: {
        let mut s = Collection.Stack.new(2)
        let s = Collection.Stack.push(s, 10)
        let s = Collection.Stack.push(s, 20)
        let s = Collection.Stack.push(s, 30)

        let top_val = Collection.Stack.peek(s)
        let stack_len = Collection.Stack.len(s)
        let is_empty_false = Collection.Stack.is_empty(s)

        let s = Collection.Stack.pop(s)
        let popped_top = Collection.Stack.peek(s)
        let stack_arr = Collection.Stack.to_array(s)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let top_val_reg = ir.symbols.get("top_val").expect("top_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(top_val_reg),
        Some(Payload::Integer(30))
    );

    let stack_len_reg = ir.symbols.get("stack_len").expect("stack_len not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(stack_len_reg),
        Some(Payload::Integer(3))
    );

    let is_empty_reg = ir
        .symbols
        .get("is_empty_false")
        .expect("is_empty_false not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_empty_reg),
        Some(Payload::Bool(false))
    );

    let popped_top_reg = ir
        .symbols
        .get("popped_top")
        .expect("popped_top not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(popped_top_reg),
        Some(Payload::Integer(20))
    );

    let stack_arr_reg = ir.symbols.get("stack_arr").expect("stack_arr not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(stack_arr_reg),
        Some(Payload::Array(vec![
            Payload::Integer(10),
            Payload::Integer(20),
        ]))
    );

    Ok(())
}

#[test]
fn test_collection_queue_fifo() -> anyhow::Result<()> {
    let source = r#"
    import "std/collection" as Collection

    @0ms: {
        let mut q = Collection.Queue.new(2)
        let q = Collection.Queue.push(q, 100)
        let q = Collection.Queue.push(q, 200)
        let q = Collection.Queue.push(q, 300)

        let head_val = Collection.Queue.peek(q)
        let queue_len = Collection.Queue.len(q)

        let q = Collection.Queue.pop(q)
        let new_head = Collection.Queue.peek(q)
        let queue_arr = Collection.Queue.to_array(q)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let head_val_reg = ir.symbols.get("head_val").expect("head_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(head_val_reg),
        Some(Payload::Integer(100))
    );

    let queue_len_reg = ir.symbols.get("queue_len").expect("queue_len not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(queue_len_reg),
        Some(Payload::Integer(3))
    );

    let new_head_reg = ir.symbols.get("new_head").expect("new_head not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(new_head_reg),
        Some(Payload::Integer(200))
    );

    let queue_arr_reg = ir.symbols.get("queue_arr").expect("queue_arr not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(queue_arr_reg),
        Some(Payload::Array(vec![
            Payload::Integer(200),
            Payload::Integer(300),
        ]))
    );

    Ok(())
}

#[test]
fn test_collection_ring_buffer_circular() -> anyhow::Result<()> {
    let source = r#"
    import "std/collection" as Collection

    @0ms: {
        let mut rb = Collection.RingBuffer.new(3)
        let rb = Collection.RingBuffer.push(rb, 1)
        let rb = Collection.RingBuffer.push(rb, 2)
        let rb = Collection.RingBuffer.push(rb, 3)
        let is_full_true = Collection.RingBuffer.is_full(rb)

        // Overwrite oldest item (1) with 4
        let rb = Collection.RingBuffer.push(rb, 4)
        let rb_arr = Collection.RingBuffer.to_array(rb)
        let head_val = Collection.RingBuffer.peek(rb)
        let rb_len = Collection.RingBuffer.len(rb)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let is_full_reg = ir
        .symbols
        .get("is_full_true")
        .expect("is_full_true not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(is_full_reg),
        Some(Payload::Bool(true))
    );

    let head_val_reg = ir.symbols.get("head_val").expect("head_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(head_val_reg),
        Some(Payload::Integer(2))
    );

    let rb_len_reg = ir.symbols.get("rb_len").expect("rb_len not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(rb_len_reg),
        Some(Payload::Integer(3))
    );

    let rb_arr_reg = ir.symbols.get("rb_arr").expect("rb_arr not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(rb_arr_reg),
        Some(Payload::Array(vec![
            Payload::Integer(2),
            Payload::Integer(3),
            Payload::Integer(4),
        ]))
    );

    Ok(())
}

#[test]
fn test_collection_bitset_operations() -> anyhow::Result<()> {
    let source = r#"
    import "std/collection" as Collection

    @0ms: {
        let mut bs = Collection.BitSet.new(128)
        let bs = Collection.BitSet.set(bs, 0)
        let bs = Collection.BitSet.set(bs, 5)
        let bs = Collection.BitSet.set(bs, 70)

        let has_zero = Collection.BitSet.get(bs, 0)
        let has_five = Collection.BitSet.get(bs, 5)
        let has_ten = Collection.BitSet.get(bs, 10)
        let has_seventy = Collection.BitSet.get(bs, 70)

        let total_set = Collection.BitSet.count(bs)

        let bs = Collection.BitSet.clear(bs, 5)
        let has_five_after_clear = Collection.BitSet.get(bs, 5)
        let total_after_clear = Collection.BitSet.count(bs)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let opt_ir = causm_ir::optimize::optimize_program(ir.clone());

    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&opt_ir)?;

    let has_zero_reg = ir.symbols.get("has_zero").expect("has_zero not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_zero_reg),
        Some(Payload::Bool(true))
    );

    let has_five_reg = ir.symbols.get("has_five").expect("has_five not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_five_reg),
        Some(Payload::Bool(true))
    );

    let has_ten_reg = ir.symbols.get("has_ten").expect("has_ten not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_ten_reg),
        Some(Payload::Bool(false))
    );

    let has_seventy_reg = ir
        .symbols
        .get("has_seventy")
        .expect("has_seventy not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_seventy_reg),
        Some(Payload::Bool(true))
    );

    let total_set_reg = ir.symbols.get("total_set").expect("total_set not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(total_set_reg),
        Some(Payload::Integer(3))
    );

    let has_five_cleared_reg = ir
        .symbols
        .get("has_five_after_clear")
        .expect("has_five_after_clear not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(has_five_cleared_reg),
        Some(Payload::Bool(false))
    );

    let total_cleared_reg = ir
        .symbols
        .get("total_after_clear")
        .expect("total_after_clear not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(total_cleared_reg),
        Some(Payload::Integer(2))
    );

    Ok(())
}
