use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_sync_atomic_int_operations() -> anyhow::Result<()> {
    let source = r#"
    import "std/sync" as Sync

    @0ms: {
        let a = Sync.Atomic.new_int(10)
        let initial_val = Sync.Atomic.load_int(a)

        let add_res = Sync.Atomic.fetch_add(a, 5)
        let old_val = add_res.old
        let a_after_add = add_res.atomic
        let after_add_val = Sync.Atomic.load_int(a_after_add)

        let cas_res = Sync.Atomic.cas_int(a_after_add, 15, 100)
        let cas_ok = cas_res.success
        let a_final = cas_res.atomic
        let final_val = Sync.Atomic.load_int(a_final)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let initial_reg = ir
        .symbols
        .get("initial_val")
        .expect("initial_val not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(initial_reg),
        Some(Payload::Integer(10))
    );

    let old_reg = ir.symbols.get("old_val").expect("old_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(old_reg),
        Some(Payload::Integer(10))
    );

    let after_add_reg = ir
        .symbols
        .get("after_add_val")
        .expect("after_add_val not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(after_add_reg),
        Some(Payload::Integer(15))
    );

    let cas_ok_reg = ir.symbols.get("cas_ok").expect("cas_ok not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(cas_ok_reg),
        Some(Payload::Bool(true))
    );

    let final_reg = ir.symbols.get("final_val").expect("final_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(final_reg),
        Some(Payload::Integer(100))
    );

    Ok(())
}

#[test]
fn test_sync_atomic_bool_cas() -> anyhow::Result<()> {
    let source = r#"
    import "std/sync" as Sync

    @0ms: {
        let b = Sync.Atomic.new_bool(false)
        let init_val = Sync.Atomic.load_bool(b)

        let cas_fail = Sync.Atomic.cas_bool(b, true, true)
        let fail_ok = cas_fail.success
        let b_unmodified = cas_fail.atomic

        let cas_succ = Sync.Atomic.cas_bool(b_unmodified, false, true)
        let succ_ok = cas_succ.success
        let b_final = cas_succ.atomic
        let final_val = Sync.Atomic.load_bool(b_final)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let init_reg = ir.symbols.get("init_val").expect("init_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(init_reg),
        Some(Payload::Bool(false))
    );

    let fail_ok_reg = ir.symbols.get("fail_ok").expect("fail_ok not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(fail_ok_reg),
        Some(Payload::Bool(false))
    );

    let succ_ok_reg = ir.symbols.get("succ_ok").expect("succ_ok not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(succ_ok_reg),
        Some(Payload::Bool(true))
    );

    let final_reg = ir.symbols.get("final_val").expect("final_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(final_reg),
        Some(Payload::Bool(true))
    );

    Ok(())
}

#[test]
fn test_sync_mutex_lock_and_unlock() -> anyhow::Result<()> {
    let source = r#"
    import "std/sync" as Sync

    @0ms: {
        let m = Sync.Mutex.new()
        let is_init_locked = Sync.Mutex.is_locked(m)

        let lock_res = Sync.Mutex.try_lock(m, "worker_1")
        let acquired = lock_res.acquired
        let m_locked = lock_res.mutex
        let owner = Sync.Mutex.owner(m_locked)

        let second_try = Sync.Mutex.try_lock(m_locked, "worker_2")
        let second_acquired = second_try.acquired
        let m_still_locked = second_try.mutex

        let m_unlocked = Sync.Mutex.unlock(m_still_locked)
        let is_final_locked = Sync.Mutex.is_locked(m_unlocked)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let init_lock_reg = ir
        .symbols
        .get("is_init_locked")
        .expect("is_init_locked not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(init_lock_reg),
        Some(Payload::Bool(false))
    );

    let acq_reg = ir.symbols.get("acquired").expect("acquired not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(acq_reg),
        Some(Payload::Bool(true))
    );

    let owner_reg = ir.symbols.get("owner").expect("owner not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(owner_reg),
        Some(Payload::String("worker_1".to_string()))
    );

    let second_acq_reg = ir
        .symbols
        .get("second_acquired")
        .expect("second_acquired not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(second_acq_reg),
        Some(Payload::Bool(false))
    );

    let final_lock_reg = ir
        .symbols
        .get("is_final_locked")
        .expect("is_final_locked not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(final_lock_reg),
        Some(Payload::Bool(false))
    );

    Ok(())
}

#[test]
fn test_sync_channel_bounded_fifo() -> anyhow::Result<()> {
    let source = r#"
    import "std/sync" as Sync

    @0ms: {
        let ch = Sync.SyncChannel.new(4)
        let is_init_empty = Sync.SyncChannel.is_empty(ch)

        let s1 = Sync.SyncChannel.send(ch, 42)
        let ok1 = s1.ok
        let s2 = Sync.SyncChannel.send(s1.chan, 84)
        let ok2 = s2.ok

        let len_after_send = Sync.SyncChannel.len(s2.chan)

        let r1 = Sync.SyncChannel.recv(s2.chan)
        let val1 = r1.val
        let r2 = Sync.SyncChannel.recv(r1.chan)
        let val2 = r2.val

        let is_empty_after_recv = Sync.SyncChannel.is_empty(r2.chan)
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let init_empty_reg = ir
        .symbols
        .get("is_init_empty")
        .expect("is_init_empty not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(init_empty_reg),
        Some(Payload::Bool(true))
    );

    let ok1_reg = ir.symbols.get("ok1").expect("ok1 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(ok1_reg),
        Some(Payload::Bool(true))
    );

    let ok2_reg = ir.symbols.get("ok2").expect("ok2 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(ok2_reg),
        Some(Payload::Bool(true))
    );

    let len_reg = ir
        .symbols
        .get("len_after_send")
        .expect("len_after_send not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(len_reg),
        Some(Payload::Integer(2))
    );

    let val1_reg = ir.symbols.get("val1").expect("val1 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(val1_reg),
        Some(Payload::Integer(42))
    );

    let val2_reg = ir.symbols.get("val2").expect("val2 not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(val2_reg),
        Some(Payload::Integer(84))
    );

    let empty_reg = ir
        .symbols
        .get("is_empty_after_recv")
        .expect("is_empty_after_recv not found")
        .0;
    assert_eq!(
        vm.root_timeline.arena.peek(empty_reg),
        Some(Payload::Bool(true))
    );

    Ok(())
}

#[test]
fn test_sync_channel_cross_branch_merge_and_field_access() -> anyhow::Result<()> {
    let source = r#"
    import "std/sync" as Sync

    @0ms: {
        let ch = Sync.SyncChannel.new(4)
        split main into [producer, consumer]
    }

    @10ms: {
        @producer: {
            let send_res = Sync.SyncChannel.send(ch, 999)
            let send_ok = send_res.ok
        }
        @consumer: {
            let initial_empty = true
        }
    }

    @20ms: {
        merge [producer, consumer] into main
        let recv_res = Sync.SyncChannel.recv(send_res.chan)
        let recv_val = recv_res.val
        let recv_ok = recv_res.ok
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let val_reg = ir.symbols.get("recv_val").expect("recv_val not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(val_reg),
        Some(Payload::Integer(999))
    );

    let ok_reg = ir.symbols.get("recv_ok").expect("recv_ok not found").0;
    assert_eq!(
        vm.root_timeline.arena.peek(ok_reg),
        Some(Payload::Bool(true))
    );

    Ok(())
}
