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
