use causm_kernel_sys::CellT;

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
