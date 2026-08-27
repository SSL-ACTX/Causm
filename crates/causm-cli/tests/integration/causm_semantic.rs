use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::{Arena, EntropicState, Payload};
use causm_frontend::parser;
use causm_runtime::vm::{TemporalError, Vm};

#[test]
fn test_causm_sized_primitive_types() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let mask: u8 = 255
        let count: u32 = 1000
        let temp: i16 = -50
        let ratio: f32 = 3.14
        let _mask = mask
        let _count = count
        let _temp = temp
        let _ratio = ratio
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn test_causm_type_casting_and_broadcasting() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x: int = 42
        let y: float = x as float
        let z: int = 3.14 as int
        let arr = [1, 2, 3] * 10
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let arr_reg = ir.symbols.get("arr").expect("arr not found").0;
    let arr_val = vm.root_timeline.arena.peek(arr_reg);
    match arr_val {
        Some(Payload::Array(vec)) => {
            assert_eq!(vec.len(), 3);
            assert_eq!(vec[0], Payload::Integer(10));
            assert_eq!(vec[1], Payload::Integer(20));
            assert_eq!(vec[2], Payload::Integer(30));
        }
        other => panic!("Expected broadcasted array [10, 20, 30], got {:?}", other),
    }

    Ok(())
}

#[test]
fn causm_semantic_arena_insert_overwrite_reclaims_previous_memory() {
    let mut arena = Arena::new(200);
    // Key register: 0
    // Payload "abc" weight: 3 + 24 = 27
    // EntropicState::Valid overhead: 16
    // Total: 27 + 16 = 43
    // In register VM, registers are fixed overhead, so only payload weight counts.
    // Let's see Arena::insert weight calculation:
    // potential_used = potential_used.saturating_sub(self.registers[idx].weight());
    // self.used = potential_used + state_weight;
    // Payload::String("abc") weight is 3 + 24 = 27.
    // EntropicState::Valid(Payload) weight is payload.weight() + 16 = 27 + 16 = 43.

    assert!(arena
        .insert(0, EntropicState::Valid(Payload::String("abc".into())))
        .is_ok());
    assert_eq!(arena.used, 43);

    // Payload "abcdefgh" weight: 8 + 24 = 32
    // EntropicState::Valid overhead: 16
    // Total: 32 + 16 = 48
    assert!(arena
        .insert(0, EntropicState::Valid(Payload::String("abcdefgh".into())))
        .is_ok());
    assert_eq!(arena.used, 48);
}

#[test]
fn causm_semantic_if_statement_integer_arith() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let a = 10
      let b = 20
      if (a < b) {
        let c = 1
      } else {
        let c = 0
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let c_reg = ir.symbols.get("c").expect("c not found").0;
    let c_val = vm.root_timeline.arena.peek(c_reg);
    match c_val {
        Some(Payload::Integer(v)) => assert_eq!(v, 1),
        _ => panic!("Expected c=1 in branch"),
    }

    Ok(())
}

#[test]
fn causm_semantic_type_system_assignment_mismatch() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x = 1
      let x = "oops"
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    assert!(analyzer.analyze_program(&program).is_err());

    Ok(())
}

#[test]
fn causm_semantic_type_system_if_condition_must_be_bool() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x = 1
      if (x) {
        let y = 2
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    assert!(analyzer.analyze_program(&program).is_err());

    Ok(())
}

#[test]
fn causm_semantic_type_annotation_assignment_matches() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x: int = 1
      let y: bool = false
      let z = x + 2
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    Ok(())
}

#[test]
fn causm_semantic_type_annotation_assignment_mismatch() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x: bool = 1
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    assert!(analyzer.analyze_program(&program).is_err());
    Ok(())
}

#[test]
fn causm_semantic_type_decl_and_custom_type_assignment() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      type Point = struct { x:int, y:int }
      let p: Point = struct { x = 3, y = 4 }
      let s = p.x + p.y
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    Ok(())
}

#[test]
fn causm_semantic_type_decl_assignment_mismatch() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      type Point = struct { x:int, y:int }
      let p: Point = struct { x = 3, z = 4 }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    assert!(analyzer.analyze_program(&program).is_err());
    Ok(())
}

#[test]
fn causm_semantic_routine_param_return_types() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      routine add(consume a:int, consume b:int) -> int taking _ {
        let sum = a + b
        yield sum
      }
      let result:int = add(10, 20)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let res_reg = ir.symbols.get("result").expect("result not found").0;
    let result_val = vm.root_timeline.arena.peek(res_reg);
    match result_val {
        Some(Payload::Integer(v)) => assert_eq!(v, 30),
        _ => panic!("Expected result=30"),
    }
    Ok(())
}

#[test]
fn causm_semantic_peek_borrow_does_not_consume() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let p = struct { a = "x", b = "y" }
      let x = &p.a
      let y = &p.b
      let z = p
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let z_reg = ir.symbols.get("z").expect("z not found").0;
    let z = vm.root_timeline.arena.peek(z_reg);
    assert!(z.is_some());
    Ok(())
}

#[test]
fn causm_semantic_if_reconcile_auto() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let v = 1
      if (v > 0) {
        let x = 5
      } else {
        let x = 10
      } reconcile auto
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let x_reg = ir.symbols.get("x").expect("x not found").0;
    let x_val = vm.root_timeline.arena.peek(x_reg);
    assert!(x_val.is_some());
    Ok(())
}

#[test]
fn causm_semantic_routine_taking_inferred() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      routine f(peek p) taking _ {
        let q = p
      }
      let s = "ok"
      let r = f(s)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let r_reg = ir.symbols.get("r").expect("r not found").0;
    let r_val = vm.root_timeline.arena.peek(r_reg);
    assert!(matches!(r_val, Some(Payload::String(_))));
    Ok(())
}

#[test]
fn causm_semantic_match_entropy_valid_branch() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let user = struct { id = "1", name = "Alice" }
      match entropy(user) {
        Valid(u):
          let out = u.id
        Consumed:
          let out = "consumed"
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out not found").0;
    let out_val = vm.root_timeline.arena.peek(out_reg);
    match out_val {
        Some(Payload::String(s)) => assert_eq!(s, "1"),
        _ => panic!("Expected out=1"),
    }

    Ok(())
}

#[test]
fn causm_semantic_routine_consume_non_identifier_fails_analyzer(
) -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      routine fn(consume token) taking 5ms {
        yield token
      }
      let result = fn("not_var", "x")
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    assert!(analyzer.analyze_program(&program).is_err());
    Ok(())
}

#[test]
fn causm_semantic_routine_yield_array_struct_return() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      routine make_res() taking 20ms {
        let a = [1,2,3]
        let s = struct { x = "hello", y = "world" }
        yield a
        yield s
      }
      let result1 = make_res()
      let result2 = make_res()
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let res1_reg = ir.symbols.get("result1").expect("result1 not found").0;
    let res2_reg = ir.symbols.get("result2").expect("result2 not found").0;
    assert!(vm.root_timeline.arena.peek(res1_reg).is_some());
    assert!(vm.root_timeline.arena.peek(res2_reg).is_some());
    Ok(())
}

#[test]
fn causm_semantic_if_requires_reconcile_for_crosspath_consume() -> anyhow::Result<()>
{
    let source = r#"
    @0ms: {
      let x = "foo"
      if (1 == 1) {
        let y = x
      } else {
        let z = "bar"
      }
    }
    "#; // no reconcile

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    assert!(analyzer.analyze_program(&program).is_err());

    let source_with_reconcile = r#"
    @0ms: {
      let x = "foo"
      if (1 == 1) {
        let y = x
      } else {
        let z = "bar"
      } reconcile (x=first_wins)
    }
    "#;

    let program = parser::parse_causm(source_with_reconcile)?;
    analyzer.analyze_program(&program)?;
    Ok(())
}

#[test]
fn causm_semantic_merge_resolution_first_wins() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      split main into [w1,w2]
    }
    @w1: {
      let v = "v1"
    }
    @w2: {
      let v = "v2"
    }
    @0ms: {
      merge [w1,w2] into main reconcile(v=w1)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let v_reg = ir.symbols.get("v").expect("v not found").0;
    let root_value = vm.root_timeline.arena.peek(v_reg);
    match root_value {
        Some(Payload::String(inner)) => assert_eq!(inner, "v1"),
        _ => panic!("Expected merged v in root timeline"),
    }

    Ok(())
}

#[test]
fn causm_semantic_analyzer_missing_capability_block() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate demo {
        enable cpu(10)
        let x = "hello"
        require System.IO(path="/tmp")
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let res = analyzer.analyze_program(&program);
    assert!(res.is_err(), "Missing capability should fail analysis");

    Ok(())
}

#[test]
fn causm_semantic_isolate_print_requires_system_log() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate demo {
        enable cpu(10)
        let msg = "hello"
        print(msg)
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    assert!(
        analyzer.analyze_program(&program).is_err(),
        "Print in isolate requires System.Log"
    );

    Ok(())
}

#[test]
fn causm_semantic_isolate_print_with_system_log() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate demo {
        enable cpu(10)
        require System.Log
        let msg = "hello"
        print(msg)
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_params| {
        Ok(causm_core::value::Payload::Null)
    });
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn causm_semantic_isolate_print_without_system_log_handler_fails(
) -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate demo {
        enable cpu(10)
        require System.Log
        let msg = "hello"
        print(msg)
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();

    let res = vm.execute_program(&ir);
    match res {
        Err(TemporalError::MissingCapability(path)) => {
            assert_eq!(path, "System.Log");
        }
        Err(e) => panic!("Unexpected runtime error: {e:?}"),
        Ok(_) => panic!("Expected missing capability runtime error"),
    }

    Ok(())
}

#[test]
fn causm_semantic_for_struct_iteration_source() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let raw = struct { a = "1", b = "2" }
      for item consume raw {
        let item_copy = clone(item)
        let key = item.key
        let value = item_copy.value
        let produced = struct { key = key, value = value }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;
    Ok(())
}

#[test]
fn causm_semantic_file_input_pipeline() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate demo {
        enable cpu(10)
        require System.Log(message="hello")
        let x = "hello"
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program_with_source(&program, source, "example.csm")?;

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_params| {
        Ok(causm_core::value::Payload::Null)
    });
    vm.execute_program(&ir)?;

    let x_reg = ir.symbols.get("x").expect("x not found").0;
    assert!(vm.root_timeline.arena.peek(x_reg).is_some());

    Ok(())
}

#[test]
fn causm_semantic_print_statement() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let msg = "hello"
      print(msg)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_| Ok(causm_core::value::Payload::Null));
    vm.execute_program(&ir)?;

    // print(msg) peeks the register and does not consume it, so it remains present in the arena.
    let msg_reg = ir.symbols.get("msg").expect("msg not found").0;
    assert!(vm.root_timeline.arena.peek(msg_reg).is_some());

    Ok(())
}

#[test]
fn causm_semantic_debug_log_non_consuming() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let v = "hello"
      debug(v)
      log(v)
      let x = clone(v)
      let y = clone(x)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_| Ok(causm_core::value::Payload::Null));
    vm.execute_program(&ir)?;

    let x_reg = ir.symbols.get("x").expect("x not found").0;
    // v must survive debug/log and be cloneable
    assert!(vm.root_timeline.arena.peek(x_reg).is_some());
    Ok(())
}

#[test]
fn causm_semantic_isolate_memory_limit_out_of_memory() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate lowmem {
        enable memory(1)
        let s = "too-large"
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    let exec = vm.execute_program(&ir);
    assert!(exec.is_err());

    Ok(())
}

#[test]
fn causm_semantic_clone_and_reuse_variable() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let a = "foo"
      let b = clone(a)
      let c = a
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let main = &vm.root_timeline;
    let a_reg = ir.symbols.get("a").expect("a not found").0;
    let b_reg = ir.symbols.get("b").expect("b not found").0;
    let c_reg = ir.symbols.get("c").expect("c not found").0;
    // `a` is consumed by c = a; b is cloned and remains available
    assert!(
        main.arena.peek(a_reg).is_none(),
        "`a` should have been consumed by c = a"
    );
    assert!(
        main.arena.peek(b_reg).is_some(),
        "clone result should remain available"
    );
    assert!(main.arena.peek(c_reg).is_some(), "c should exist");
    Ok(())
}

#[test]
fn causm_semantic_gc_terminate_branch() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      split main into [w1]
    }
    @w1: {
      let v = "data"
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    assert!(vm.active_branches.contains_key("w1"));
    vm.terminate_branch("w1")?;
    assert!(!vm.active_branches.contains_key("w1"));
    Ok(())
}

#[test]
fn causm_semantic_gc_merge_collects_leaf_branches() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      split main into [w1,w2]
    }
    @w1: { let v1 = "x" }
    @w2: { let v2 = "y" }
    @0ms: { merge [w1,w2] into main reconcile(v1=w1,v2=w2) }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    assert!(!vm.active_branches.contains_key("w1"));
    assert!(!vm.active_branches.contains_key("w2"));

    Ok(())
}

#[test]
fn causm_semantic_capability_require_outbound_and_use() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate net {
        enable cpu(10)
        require Net.Outbound(rate="5/s", domain="api.example.com")
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.register_capability("Net.Outbound", |_params| {
        Ok(causm_core::value::Payload::Null)
    });
    vm.register_capability("System.Log", |_params| {
        Ok(causm_core::value::Payload::Null)
    });

    let res = vm.execute_program(&ir);
    assert!(res.is_ok());

    Ok(())
}

#[test]
fn causm_semantic_analyzer_unresolved_merge_collision() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      split main into [w1,w2]
      @w1: { let v = "v1" }
      @w2: { let v = "v2" }
      merge [w1,w2] into main
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_err(),
        "unresolved merge collisions should trigger analyzer error"
    );
    Ok(())
}

#[test]
fn causm_semantic_analyzer_use_after_consume() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x = "a"
      let y = x
      let z = x
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_err(),
        "use-after-consume should be rejected by analyzer"
    );
    Ok(())
}

#[test]
fn causm_semantic_merge_priority_resolves_to_priority_branch() -> anyhow::Result<()>
{
    let source = r#"
    @0ms: { split main into [w1,w2] }
    @w1: { let v = "v1" }
    @w2: { let v = "v2" }
    @0ms: { merge [w1,w2] into main reconcile(v=w2) }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let v_reg = ir.symbols.get("v").expect("v not found").0;
    let root_value = vm.root_timeline.arena.peek(v_reg);
    match root_value {
        Some(Payload::String(inner)) => assert_eq!(inner, "v2"),
        _ => panic!("Expected merged v from w2"),
    }

    Ok(())
}

#[test]
fn causm_semantic_split_map_collects_yields() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let data = [1, 2, 3]
      let sum = 0
      for item in data step 10ms {
        sum = sum + item
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let sum_reg = ir.symbols.get("sum").expect("sum not found").0;
    let sum = vm.root_timeline.arena.peek(sum_reg);
    assert_eq!(sum, Some(Payload::Integer(6)));
    Ok(())
}

#[test]
fn causm_semantic_capability_budget_enforcement() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate limited {
        require System.Log
        enable system_log(1)
        print("First")
        print("Second")
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_params| {
        Ok(causm_core::value::Payload::Null)
    });

    let result = vm.execute_program(&ir);
    match result {
        Err(TemporalError::CapabilityViolation(msg)) => {
            assert!(msg.contains("Capability budget exhausted"));
        }
        other => panic!("Expected CapabilityViolation, got {:?}", other),
    }

    Ok(())
}

#[test]
fn causm_semantic_match_entropy_optional_binding() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let user = struct { id = "1", name = "Alice" }
      match entropy(user) {
        Valid: {
          let out = user.id
        }
        Consumed: {
          let out = "consumed"
        }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out not found").0;
    let out_val = vm.root_timeline.arena.peek(out_reg);
    match out_val {
        Some(Payload::String(s)) => assert_eq!(s, "1"),
        _ => panic!("Expected out=1"),
    }

    Ok(())
}

#[test]
fn causm_semantic_use_after_consume_in_nonconsuming() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x = 10
      let y = x
      print(x)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("has been consumed or decayed"));

    Ok(())
}

#[test]
fn causm_semantic_inactive_timeline_error() -> anyhow::Result<()> {
    let source = r#"
    @0ms: { split main into [w1, w2] }
    @w1: { let x = 1 }
    @w2: { let y = 2 }
    @0ms: { merge [w1, w2] into main reconcile auto }
    @w1: { let z = 3 }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("inactive, has been merged, or has not been split"));

    Ok(())
}

#[test]
fn causm_semantic_match_entropy_decayed_pattern_success() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let user = struct { id = "1", name = "Alice" }
      let _consumed_name = user.name
      let out = ""
      match entropy(user) {
        Decayed({ id = Valid, name = Consumed }): {
          out = user.id
        }
        Valid: {
          out = "valid"
        }
        Consumed: {
          out = "consumed"
        }
        Pending: {
          out = "pending"
        }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out not found").0;
    let out_val = vm.root_timeline.arena.peek(out_reg);
    match out_val {
        Some(Payload::String(s)) => assert_eq!(s, "1"),
        _ => panic!("Expected out=1"),
    }

    Ok(())
}

#[test]
fn causm_semantic_match_entropy_decayed_pattern_use_after_consume(
) -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let user = struct { id = "1", name = "Alice" }
      let _consumed_name = user.name
      match entropy(user) {
        Decayed({ id = Valid, name = Consumed }): {
          let test_use = user.name
        }
        Valid: {}
        Consumed: {}
        Pending: {}
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("user.name"));

    Ok(())
}

#[test]
fn causm_semantic_temporal_decay_and_decay_handler() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        type Account = struct decay_after 2ms { id: string, balance: int }
        let cleanup_executed = false
        on_decay(Account) {
            cleanup_executed = true
        }
        let act: Account = struct { id = "123", balance = 1000 }
        slice 15ms
        match entropy(act) {
            Decayed: {}
            Valid: {}
            Pending: {}
            Consumed: {}
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let cleanup_reg = ir
        .symbols
        .get("cleanup_executed")
        .expect("cleanup_executed not found")
        .0;
    let cleanup_val = vm.root_timeline.arena.peek(cleanup_reg);
    match cleanup_val {
        Some(Payload::Bool(b)) => assert!(
            b,
            "Expected decay_handler to run and set cleanup_executed=true"
        ),
        _ => panic!("Expected cleanup_executed to be true bool"),
    }

    Ok(())
}

#[test]
fn causm_semantic_valid_pattern_destructuring_literal() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let user = struct { id = "1", name = "Alice", balance = 100 }
        let out = ""
        match entropy(user) {
            Valid({ balance = 100 }): {
                out = "match_100"
            }
            Consumed: {}
            Pending: {}
            Decayed: {}
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out not found").0;
    let out_val = vm.root_timeline.arena.peek(out_reg);
    match out_val {
        Some(Payload::String(s)) => assert_eq!(s, "match_100"),
        _ => panic!("Expected out='match_100'"),
    }

    Ok(())
}

#[test]
fn test_syntax_print_variadic_arguments() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 42
        let y = "world"
        print("Hello", y, "val =", x)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let logged = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let logged_clone = logged.clone();

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(move |params| {
            if let Some(msg) = params.get("message") {
                logged_clone.lock().unwrap().push(msg.clone());
            }
            Ok(Payload::Null)
        }),
    );
    vm.execute_program(&ir)?;

    let logs = logged.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], "Hello world val = 42");

    Ok(())
}

#[test]
fn test_syntax_fstring_interpolation() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let name = "Alice"
        let age = 30
        let msg = f"User {name} is {age} years old"
        print(msg)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let logged = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let logged_clone = logged.clone();

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(move |params| {
            if let Some(msg) = params.get("message") {
                logged_clone.lock().unwrap().push(msg.clone());
            }
            Ok(Payload::Null)
        }),
    );
    vm.execute_program(&ir)?;

    let logs = logged.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], "User Alice is 30 years old");

    Ok(())
}

#[test]
fn test_syntax_fstring_with_expressions() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let a = 10
        let b = 25
        let result = f"{a} + {b} = {a + b}"
        print(result)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let logged = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let logged_clone = logged.clone();

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(move |params| {
            if let Some(msg) = params.get("message") {
                logged_clone.lock().unwrap().push(msg.clone());
            }
            Ok(Payload::Null)
        }),
    );
    vm.execute_program(&ir)?;

    let logs = logged.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], "10 + 25 = 35");

    Ok(())
}

#[test]
fn test_syntax_string_escape_sequences() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let text = "Line1\nLine2\tTabbed"
        let val = 42
        let ftext = f"Val:\t{val}\nDone."
        print(text)
        print(ftext)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let logged = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let logged_clone = logged.clone();

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(move |params| {
            if let Some(msg) = params.get("message") {
                logged_clone.lock().unwrap().push(msg.clone());
            }
            Ok(Payload::Null)
        }),
    );
    vm.execute_program(&ir)?;

    let logs = logged.lock().unwrap();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0], "Line1\nLine2\tTabbed");
    assert_eq!(logs[1], "Val:\t42\nDone.");

    Ok(())
}

#[test]
fn test_capability_routine_requirement_allowed() -> anyhow::Result<()> {
    let source = r#"
    routine log_message(msg: string) require System.Log taking 5ms -> () {
        print(msg)
    }

    @0ms: {
        isolate demo {
            require System.Log
            log_message("Hello from secured routine")
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let logged = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let logged_clone = logged.clone();

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(move |params| {
            if let Some(msg) = params.get("message") {
                logged_clone.lock().unwrap().push(msg.clone());
            }
            Ok(Payload::Null)
        }),
    );
    vm.execute_program(&ir)?;

    let logs = logged.lock().unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], "Hello from secured routine");

    Ok(())
}

#[test]
fn test_capability_routine_requirement_missing_rejected() -> anyhow::Result<()> {
    let source = r#"
    routine log_message(msg: string) require System.Log taking 5ms -> () {
        print(msg)
    }

    @0ms: {
        isolate demo {
            require System.Entropy
            log_message("Unauthorized call")
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let res = analyzer.analyze_program(&program);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.to_string().contains("Missing capability")
            || err.to_string().contains("System.Log"),
        "Unexpected error: {:?}",
        err
    );

    Ok(())
}

#[test]
fn test_capability_runtime_introspection_check() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let has_log = capability(System.Log)
        let has_net = capability(Net.Http)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.capability_handlers
        .insert("System.Log".to_string(), Box::new(|_| Ok(Payload::Null)));
    vm.execute_program(&ir)?;

    let has_log_reg = ir.symbols.get("has_log").unwrap().0;
    let has_net_reg = ir.symbols.get("has_net").unwrap().0;

    assert_eq!(
        vm.root_timeline.arena.peek(has_log_reg),
        Some(Payload::Bool(true))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(has_net_reg),
        Some(Payload::Bool(false))
    );

    Ok(())
}

#[test]
fn test_syntax_generic_turbofish_call() -> anyhow::Result<()> {
    let source = r#"
    routine identity(val: int) -> int taking _ => val

    @0ms: {
        let x = identity::<int>(42)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let x_reg = ir.symbols.get("x").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(x_reg),
        Some(Payload::Integer(42))
    );

    Ok(())
}

#[test]
fn test_syntax_generic_static_call() -> anyhow::Result<()> {
    let source = r#"
    routine Buffer.new(capacity: int) -> int taking _ => capacity

    @0ms: {
        let cap = Buffer<u8>::new(1024)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let cap_reg = ir.symbols.get("cap").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(cap_reg),
        Some(Payload::Integer(1024))
    );

    Ok(())
}

#[test]
fn test_chaining_monadic_try_operator() -> anyhow::Result<()> {
    let source = r#"
    routine get_val(flag: bool) -> int taking _ {
        if flag {
            let res = 99
            yield res
        } else {
            let n = null
            yield n
        } reconcile auto
    }

    routine add_one(x: int) -> int taking _ => x + 1

    routine chain_success() -> int taking _ {
        let v = get_val(true)?
        let res = add_one(v)
        yield res
    }

    routine chain_short_circuit() -> int taking _ {
        let v = get_val(false)?
        let res = add_one(v)
        yield res
    }

    @0ms: {
        let success_val = chain_success()
        let short_circuit_val = chain_short_circuit()
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let succ_reg = ir.symbols.get("success_val").unwrap().0;
    let sc_reg = ir.symbols.get("short_circuit_val").unwrap().0;

    assert_eq!(
        vm.root_timeline.arena.peek(succ_reg),
        Some(Payload::Integer(100))
    );
    assert_eq!(vm.root_timeline.arena.peek(sc_reg), Some(Payload::Null));

    Ok(())
}

#[test]
fn test_capability_routine_with_bracketed_syntax() -> anyhow::Result<()> {
    let source = r#"
    routine logged_fetch(url: string) -> string with [System.NetworkFetch, System.Log] {
        yield url
    }

    @0ms: {
        isolate secure_zone {
            require System.NetworkFetch
            require System.Log
            let data = logged_fetch("https://causm.org")
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.register_capability("System.NetworkFetch", |_| Ok(Payload::Null));
    vm.register_capability("System.Log", |_| Ok(Payload::Null));
    vm.execute_program(&ir)?;
    Ok(())
}

#[test]
fn test_capability_routine_requires_bracketed_missing_cap_error(
) -> anyhow::Result<()> {
    let source = r#"
    routine dangerous_op() -> bool requires [System.IO, System.NetworkFetch] {
        yield true
    }

    @0ms: {
        isolate sandbox {
            require System.IO
            let ok = dangerous_op()
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let res = analyzer.analyze_program(&program);
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("Missing capability")
            || err_str.contains("System.NetworkFetch")
    );
    Ok(())
}

#[test]
fn test_pure_path_utilities_in_zero_cap_isolate() -> anyhow::Result<()> {
    let source = r#"
    from "std/path" import join, path_basename, path_dirname, extension

    @0ms: {
        isolate pure_sandbox {
            enable memory(64KB)
            enable cpu(100ms)

            let p = join("/var/log", "app.log")
            let b = path_basename(p)
            let d = path_dirname(p)
            let ext = extension(p)
        }
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let p_reg = ir.symbols.get("p").unwrap().0;
    let b_reg = ir.symbols.get("b").unwrap().0;
    let d_reg = ir.symbols.get("d").unwrap().0;
    let ext_reg = ir.symbols.get("ext").unwrap().0;

    assert_eq!(
        vm.root_timeline.arena.peek(p_reg),
        Some(Payload::String("/var/log/app.log".to_string()))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(b_reg),
        Some(Payload::String("app.log".to_string()))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(d_reg),
        Some(Payload::String("/var/log".to_string()))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(ext_reg),
        Some(Payload::String(".log".to_string()))
    );

    Ok(())
}

#[test]
fn test_tiered_stdlib_imports_in_zero_cap_isolate() -> anyhow::Result<()> {
    let source = r#"
    from "std/time" import now, unix_timestamp
    from "std/fs" import open_readonly, file_exists
    from "std/process" import pid, parent_pid
    from "std/net" import create_socket
    from "std/env" import current_pid, current_dir

    @0ms: {
        isolate unprivileged {
            enable memory(64KB)
            enable cpu(100ms)

            let t = now()
            let u = unix_timestamp()
            let f = open_readonly("/etc/passwd")
            let exists = file_exists("/etc/passwd")
            let p = pid()
            let pp = parent_pid()
            let sock = create_socket()
            let env_p = current_pid()
            let env_d = current_dir()
        }
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let u_reg = ir.symbols.get("u").unwrap().0;
    let exists_reg = ir.symbols.get("exists").unwrap().0;
    let p_reg = ir.symbols.get("p").unwrap().0;
    let sock_reg = ir.symbols.get("sock").unwrap().0;
    let env_p_reg = ir.symbols.get("env_p").unwrap().0;
    let env_d_reg = ir.symbols.get("env_d").unwrap().0;

    assert_eq!(
        vm.root_timeline.arena.peek(u_reg),
        Some(Payload::Integer(0))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(exists_reg),
        Some(Payload::Bool(false))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(p_reg),
        Some(Payload::Integer(1))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(sock_reg),
        Some(Payload::Integer(-1))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(env_p_reg),
        Some(Payload::Integer(1))
    );
    assert_eq!(
        vm.root_timeline.arena.peek(env_d_reg),
        Some(Payload::String("/".to_string()))
    );

    Ok(())
}
