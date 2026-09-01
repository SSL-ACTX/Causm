use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Cell<T> {
    sequence: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Sync for Cell<T> {}

/// Fixed-capacity bounded lock-free MPMC queue (Dvyukov bounded MPMC queue algorithm).
pub struct MpmcQueue<T, const N: usize> {
    buffer: [Cell<T>; N],
    enqueue_pos: AtomicUsize,
    dequeue_pos: AtomicUsize,
}

unsafe impl<T: Send, const N: usize> Send for MpmcQueue<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for MpmcQueue<T, N> {}

impl<T, const N: usize> MpmcQueue<T, N> {
    pub fn new() -> Self {
        assert!(
            N > 0 && (N & (N - 1)) == 0,
            "MpmcQueue capacity must be a non-zero power of 2"
        );
        let buffer = unsafe {
            let mut arr: [Cell<T>; N] = MaybeUninit::uninit().assume_init();
            for (i, elem) in arr.iter_mut().enumerate() {
                std::ptr::write(
                    elem,
                    Cell {
                        sequence: AtomicUsize::new(i),
                        value: UnsafeCell::new(MaybeUninit::uninit()),
                    },
                );
            }
            arr
        };

        Self {
            buffer,
            enqueue_pos: AtomicUsize::new(0),
            dequeue_pos: AtomicUsize::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        N
    }

    pub fn len(&self) -> usize {
        let deq = self.dequeue_pos.load(Ordering::Relaxed);
        let enq = self.enqueue_pos.load(Ordering::Relaxed);
        enq.saturating_sub(deq)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_full(&self) -> bool {
        self.len() >= N
    }

    pub fn try_push(&self, value: T) -> Result<(), T> {
        let mut pos = self.enqueue_pos.load(Ordering::Relaxed);
        loop {
            let cell = &self.buffer[pos & (N - 1)];
            let seq = cell.sequence.load(Ordering::Acquire);
            let diff = (seq as isize).wrapping_sub(pos as isize);

            if diff == 0 {
                match self.enqueue_pos.compare_exchange_weak(
                    pos,
                    pos + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        unsafe {
                            (*cell.value.get()).write(value);
                        }
                        cell.sequence.store(pos + 1, Ordering::Release);
                        return Ok(());
                    }
                    Err(actual) => {
                        pos = actual;
                    }
                }
            } else if diff < 0 {
                return Err(value);
            } else {
                pos = self.enqueue_pos.load(Ordering::Relaxed);
            }
        }
    }

    pub fn try_pop(&self) -> Option<T> {
        let mut pos = self.dequeue_pos.load(Ordering::Relaxed);
        loop {
            let cell = &self.buffer[pos & (N - 1)];
            let seq = cell.sequence.load(Ordering::Acquire);
            let diff = (seq as isize).wrapping_sub((pos + 1) as isize);

            if diff == 0 {
                match self.dequeue_pos.compare_exchange_weak(
                    pos,
                    pos + 1,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let value =
                            unsafe { (*cell.value.get()).assume_init_read() };
                        cell.sequence.store(pos + (N - 1) + 1, Ordering::Release);
                        return Some(value);
                    }
                    Err(actual) => {
                        pos = actual;
                    }
                }
            } else if diff < 0 {
                return None;
            } else {
                pos = self.dequeue_pos.load(Ordering::Relaxed);
            }
        }
    }
}

impl<T, const N: usize> Default for MpmcQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for MpmcQueue<T, N> {
    fn drop(&mut self) {
        while self.try_pop().is_some() {}
    }
}
