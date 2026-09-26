// causm-kernel/bindings/causm-kernel-sys/tests/kernel_tests.rs
use causm_kernel_sys::{Arena, CellT, IsochronousTracker, KernelDiagnostic, RawInstr, VerifiedVM};

#[test]
fn test_kernel_read_valid_cell() {
    let cell = CellT {
        payload: 42,
        tag: 0,
        expire: 100,
    };
    assert_eq!(cell.payload, 42);
}

#[test]
fn test_kernel_arena_multi_register_workflow() {
    let mut arena = Arena::new(8);
    assert_eq!(arena.len(), 8);

    assert_eq!(arena.read(0), None);
    assert_eq!(arena.read(1), None);

    assert!(arena.init_valid(0, 100));
    assert!(arena.init_valid(1, 200));

    assert_eq!(arena.read(0), Some(100));
    assert_eq!(arena.read(1), Some(200));

    assert!(arena.lease(1, 50, 25));
    let cell1 = arena.get_cell(1).unwrap();
    assert_eq!(cell1.tag, 1);
    assert_eq!(cell1.expire, 75);

    assert!(arena.consume(0));
    assert_eq!(arena.read(0), None);
    assert!(!arena.consume(0));

    arena.tick_sweep(60);
    assert_eq!(arena.get_cell(1).unwrap().tag, 1);

    arena.tick_sweep(75);
    assert_eq!(arena.get_cell(1).unwrap().tag, 2);
    assert_eq!(arena.read(1), None);
}

#[test]
fn test_kernel_fine_grained_diagnostics() {
    let mut arena = Arena::new(4);
    assert_eq!(arena.check_access(0, 0), KernelDiagnostic::UseAfterConsume);
    assert_eq!(arena.check_consume(0), KernelDiagnostic::DoubleConsume);

    arena.init_valid(0, 42);
    assert_eq!(arena.check_access(0, 0), KernelDiagnostic::Ok);
    assert_eq!(arena.check_consume(0), KernelDiagnostic::Ok);

    assert!(arena.lease(0, 10, 20));
    assert_eq!(arena.check_access(0, 15), KernelDiagnostic::Ok);
    assert_eq!(arena.check_consume(0), KernelDiagnostic::ConsumeActiveLease);

    assert_eq!(arena.check_access(0, 30), KernelDiagnostic::LeaseExpired);

    arena.tick_sweep(30);
    assert_eq!(arena.check_access(0, 30), KernelDiagnostic::UseAfterDecay);
    assert_eq!(arena.check_consume(0), KernelDiagnostic::ConsumeDecayed);
}

#[test]
fn test_kernel_check_lease_hierarchical_subleasing() {
    let mut arena = Arena::new(4);
    assert_eq!(arena.check_lease(0, 10, 20), KernelDiagnostic::CannotLeaseNonValid);

    arena.init_valid(0, 100);
    assert_eq!(arena.check_lease(0, 10, 20), KernelDiagnostic::Ok);

    assert_eq!(
        arena.check_lease(0, u64::MAX - 5, 10),
        KernelDiagnostic::LeaseDurationOverflow
    );

    assert!(arena.lease(0, 10, 50));
    assert_eq!(arena.check_lease(0, 15, 10), KernelDiagnostic::CannotLeaseNonValid);
}

#[test]
fn test_kernel_wcet_zero_jitter_pacing() {
    let mut tracker = IsochronousTracker::new(100);
    assert!(tracker.try_pace_step(15, 25));
    assert_eq!(tracker.consumed(), 25);
    assert_eq!(tracker.remaining(), 75);

    assert!(tracker.try_pace_step(25, 25));
    assert_eq!(tracker.consumed(), 50);

    assert!(!tracker.try_pace_step(30, 60));
    assert_eq!(tracker.consumed(), 50);

    assert!(!tracker.try_pace_step(40, 30));
    assert_eq!(tracker.consumed(), 50);

    assert!(tracker.try_pace_step(35, 50));
    assert_eq!(tracker.consumed(), 100);
    assert_eq!(tracker.remaining(), 0);
}

#[test]
fn test_kernel_arena_state_partition_conservation() {
    let mut arena = Arena::new(10);
    let (v, l, d, c) = arena.state_partition();
    assert_eq!((v, l, d, c), (0, 0, 0, 10));

    for i in 0..4 {
        arena.init_valid(i, (i + 1) as u64 * 10);
    }
    let (v, l, d, c) = arena.state_partition();
    assert_eq!((v, l, d, c), (4, 0, 0, 6));

    assert!(arena.lease(1, 0, 20));
    assert!(arena.lease(2, 0, 30));
    let (v, l, d, c) = arena.state_partition();
    assert_eq!((v, l, d, c), (2, 2, 0, 6));

    arena.tick_sweep(25);
    let (v, l, d, c) = arena.state_partition();
    assert_eq!((v, l, d, c), (2, 1, 1, 6));

    assert!(arena.consume(0));
    let (v, l, d, c) = arena.state_partition();
    assert_eq!((v, l, d, c), (1, 1, 1, 7));
}

#[test]
fn test_kernel_entanglement_cascade_consumption() {
    let mut vm = VerifiedVM::new(4);
    assert!(vm.arena.init_valid(0, 100));
    assert!(vm.arena.init_valid(1, 200));
    assert!(vm.arena.init_valid(2, 300));
    assert!(vm.arena.init_valid(3, 400));

    // Entangle R0 <-> R1 and R1 <-> R2
    assert!(vm.entangle(0, 1));
    assert!(vm.entangle(1, 2));

    assert!(vm.is_entangled(0, 1));
    assert!(vm.is_entangled(1, 0));
    assert!(vm.is_entangled(1, 2));
    assert!(!vm.is_entangled(0, 3));

    // Consuming R0 must cascade and consume R1
    assert!(vm.consume_cascading(0));
    assert_eq!(vm.arena.read(0), None);
    assert_eq!(vm.arena.read(1), None);
    // R3 was not entangled, stays valid
    assert_eq!(vm.arena.read(3), Some(400));
}

#[test]
fn test_kernel_cfg_basic_block_step_execution() {
    let mut vm = VerifiedVM::new(4);

    // Instruction Block:
    // 0: LoadInt R0, 10
    // 1: LoadInt R1, 20
    // 2: Add R2, R0, R1 (R2 = 30)
    let body = vec![
        RawInstr { op: 0, arg1: 0, arg2: 0, arg3: 0, imm: 10 },
        RawInstr { op: 0, arg1: 1, arg2: 0, arg3: 0, imm: 20 },
        RawInstr { op: 1, arg1: 2, arg2: 0, arg3: 1, imm: 0 },
    ];

    assert!(vm.execute_basic_block(&body));
    assert_eq!(vm.arena.read(0), Some(10));
    assert_eq!(vm.arena.read(1), Some(20));
    assert_eq!(vm.arena.read(2), Some(30));
}

#[test]
fn test_kernel_typechecker_static_verification_and_rejection() {
    let mut vm = VerifiedVM::new(4);

    // Valid Block
    let valid_body = vec![
        RawInstr { op: 0, arg1: 0, arg2: 0, arg3: 0, imm: 50 },
        RawInstr { op: 0, arg1: 1, arg2: 0, arg3: 0, imm: 50 },
        RawInstr { op: 1, arg1: 2, arg2: 0, arg3: 1, imm: 0 },
    ];
    assert!(vm.verify_and_execute_basic_block(&valid_body));
    assert_eq!(vm.arena.read(2), Some(100));

    // Invalid Block: Consume R0 then attempt to use R0 in Add (Use-After-Consume)
    let invalid_body = vec![
        RawInstr { op: 2, arg1: 0, arg2: 0, arg3: 0, imm: 0 }, // Consume R0
        RawInstr { op: 1, arg1: 3, arg2: 0, arg3: 1, imm: 0 }, // Add R3, R0, R1 -> Rejected!
    ];
    assert!(!vm.verify_and_execute_basic_block(&invalid_body), "Kernel must reject Use-After-Consume statically");
}
