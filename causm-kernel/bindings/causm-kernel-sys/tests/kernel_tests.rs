use causm_kernel_sys::{Arena, CellT};

#[test]
fn test_kernel_read_valid_cell() {
    let cell = CellT {
        payload: 42,
        tag: 0,
        expire: 100,
    };
    assert_eq!(cell.read_valid(), 42);
}

#[test]
fn test_kernel_consume_cell() {
    let mut cell = CellT {
        payload: 42,
        tag: 0,
        expire: 100,
    };
    cell.consume();
    assert_eq!(cell.tag, 3);
    assert_eq!(cell.payload, 0);
}

#[test]
fn test_kernel_tick_decay_cell() {
    let mut cell = CellT {
        payload: 42,
        tag: 1,
        expire: 10,
    };
    cell.tick_decay(10);
    assert_eq!(cell.tag, 2);
    assert_eq!(cell.payload, 0);
}

#[test]
fn test_kernel_lease_cell() {
    let mut cell = CellT {
        payload: 99,
        tag: 0,
        expire: 0,
    };
    cell.lease(100, 50);
    assert_eq!(cell.tag, 1);
    assert_eq!(cell.payload, 99);
    assert_eq!(cell.expire, 150);
}

#[test]
fn test_kernel_init_valid_cell() {
    let mut cell = CellT {
        payload: 0,
        tag: 3,
        expire: 0,
    };
    cell.init_valid(777);
    assert_eq!(cell.tag, 0);
    assert_eq!(cell.payload, 777);
    assert_eq!(cell.expire, 0);
}

#[test]
fn test_kernel_arena_multi_register_workflow() {
    let mut arena = Arena::new(8);
    assert_eq!(arena.len(), 8);

    // Initial slots are consumed (tag 3)
    assert_eq!(arena.read(0), None);
    assert_eq!(arena.read(1), None);

    // Initialize registers R0 and R1
    assert!(arena.init_valid(0, 100));
    assert!(arena.init_valid(1, 200));

    // Verify reading valid registers
    assert_eq!(arena.read(0), Some(100));
    assert_eq!(arena.read(1), Some(200));

    // Lease R1 at clock 50 with duration 25 (expires at 75)
    assert!(arena.lease(1, 50, 25));
    let cell1 = arena.get_cell(1).unwrap();
    assert_eq!(cell1.tag, 1);
    assert_eq!(cell1.expire, 75);

    // Consume R0
    assert!(arena.consume(0));
    assert_eq!(arena.read(0), None); // No longer readable
    assert!(!arena.consume(0)); // Double consume fails

    // Clock advances to 60: R1 should still be leased (not decayed)
    arena.tick_sweep(60);
    assert_eq!(arena.get_cell(1).unwrap().tag, 1);

    // Clock advances to 75: R1 decays
    arena.tick_sweep(75);
    assert_eq!(arena.get_cell(1).unwrap().tag, 2);
    assert_eq!(arena.read(1), None); // Decayed register unreadable
}

#[test]
fn test_kernel_cfg_lattice_meet_branch_convergence() {
    // Simulate CFG Diamond:
    //         [BB0: init R0=Valid, R1=Valid]
    //         /                            \
    // [BB1: R0 stays Valid]          [BB2: R0 consumed, R1 leased]
    //         \                            /
    //         [BB3: Join Block - meet merge]

    let mut bb1 = Arena::new(4);
    bb1.init_valid(0, 42);
    bb1.init_valid(1, 84);

    let mut bb2 = Arena::new(4);
    bb2.init_valid(0, 42);
    bb2.init_valid(1, 84);
    assert!(bb2.consume(0)); // R0 consumed on BB2 branch
    assert!(bb2.lease(1, 10, 20)); // R1 leased on BB2 branch

    // In BB1: R0=0(Valid), R1=0(Valid)
    // In BB2: R0=3(Consumed), R1=1(Leased)
    assert_eq!(bb1.get_cell(0).unwrap().tag, 0);
    assert_eq!(bb2.get_cell(0).unwrap().tag, 3);
    assert_eq!(bb1.get_cell(1).unwrap().tag, 0);
    assert_eq!(bb2.get_cell(1).unwrap().tag, 1);

    // Merge BB1 and BB2 into BB3:
    // R0: Valid (0) meet Consumed (3) => Consumed (3)
    // R1: Valid (0) meet Leased (1) => Leased (1)
    let mut bb3 = Arena::new(4);
    bb3.init_valid(0, 42);
    bb3.init_valid(1, 84);
    bb3.merge_from(&bb2);

    // Assert exact lattice meet properties on convergence block
    let r0 = bb3.get_cell(0).unwrap();
    assert_eq!(r0.tag, 3, "R0 must converge to Consumed because BB2 consumed it");
    assert_eq!(r0.payload, 0, "Consumed payload must be zeroed");
    assert_eq!(bb3.read(0), None, "Consumed register must be unreadable");

    let r1 = bb3.get_cell(1).unwrap();
    assert_eq!(r1.tag, 1, "R1 must converge to Leased because BB2 leased it");
    assert_eq!(r1.payload, 84, "Leased payload must remain readable before expiration");
    assert_eq!(bb3.read(1), Some(84));
}

#[test]
fn test_kernel_cfg_multi_predecessor_decay_meet() {
    // When one branch has decayed (tag 2) and another has leased (tag 1),
    // meet must converge to Decayed (tag 2), preventing any stale read.
    let mut bb_leased = Arena::new(2);
    bb_leased.init_valid(0, 123);
    assert!(bb_leased.lease(0, 0, 50));

    let mut bb_decayed = Arena::new(2);
    bb_decayed.init_valid(0, 123);
    assert!(bb_decayed.lease(0, 0, 10));
    bb_decayed.tick_sweep(15); // decayed at clock 15
    assert_eq!(bb_decayed.get_cell(0).unwrap().tag, 2);

    let mut bb_join = Arena::new(2);
    bb_join.init_valid(0, 123);
    assert!(bb_join.lease(0, 0, 50)); // start with leased state
    bb_join.merge_from(&bb_decayed);

    // Meet(Leased, Decayed) = Decayed
    let cell = bb_join.get_cell(0).unwrap();
    assert_eq!(cell.tag, 2, "Meet of Leased and Decayed must yield Decayed");
    assert_eq!(cell.payload, 0, "Decayed payload must be zeroed");
    assert_eq!(bb_join.read(0), None, "Decayed register must not be readable at join");
}

#[test]
fn test_kernel_cfg_loop_isochronous_clock_decay() {
    // Simulate an isochronous loop where a register has a lease of 30 ticks.
    // Loop steps in increments of 10 ticks.
    let mut arena = Arena::new(2);
    arena.init_valid(0, 555);
    assert!(arena.lease(0, 0, 30)); // expires at clock 30

    // Loop iter 1: clock = 10
    arena.tick_sweep(10);
    assert_eq!(arena.read_at(0, 10), Some(555));

    // Loop iter 2: clock = 20
    arena.tick_sweep(20);
    assert_eq!(arena.read_at(0, 20), Some(555));

    // Loop iter 3: clock = 30 (budget exhausted -> decay)
    arena.tick_sweep(30);
    assert_eq!(arena.get_cell(0).unwrap().tag, 2);
    assert_eq!(arena.read_at(0, 30), None);
}


