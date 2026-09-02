use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Fixed-capacity lock-free Single-Producer Single-Consumer (SPSC) queue.
pub struct SpscQueue<T, const N: usize> {
    buffer: [UnsafeCell<MaybeUninit<T>>; N],
    head: AtomicUsize,
    tail: AtomicUsize,
}

unsafe impl<T: Send, const N: usize> Send for SpscQueue<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for SpscQueue<T, N> {}

impl<T, const N: usize> SpscQueue<T, N> {
    pub fn new() -> Self {
        assert!(N > 0, "SpscQueue capacity must be greater than 0");
        // Initialize UnsafeCell array
        let buffer = unsafe {
            let mut arr: [UnsafeCell<MaybeUninit<T>>; N] =
                MaybeUninit::uninit().assume_init();
            for elem in &mut arr {
                std::ptr::write(elem, UnsafeCell::new(MaybeUninit::uninit()));
            }
            arr
        };

        Self {
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        N
    }

    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        tail.saturating_sub(head)
    }

    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        head >= tail
    }

    pub fn is_full(&self) -> bool {
        self.len() >= N
    }

    /// Try pushing an item to the queue. Returns Err(value) if full.
    pub fn try_push(&self, value: T) -> Result<(), T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail.saturating_sub(head) >= N {
            return Err(value);
        }

        let idx = tail % N;
        unsafe {
            let cell = self.buffer[idx].get();
            (*cell).write(value);
        }

        self.tail.store(tail + 1, Ordering::Release);
        Ok(())
    }

    /// Try popping an item from the queue. Returns None if empty.
    pub fn try_pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        if head >= tail {
            return None;
        }

        let idx = head % N;
        let value = unsafe {
            let cell = self.buffer[idx].get();
            (*cell).assume_init_read()
        };

        self.head.store(head + 1, Ordering::Release);
        Some(value)
    }

    /// Split into typed Producer and Consumer handles
    pub fn split(self) -> (SpscProducer<T, N>, SpscConsumer<T, N>) {
        let arc = Arc::new(self);
        (
            SpscProducer {
                queue: Arc::clone(&arc),
            },
            SpscConsumer { queue: arc },
        )
    }
}

impl<T, const N: usize> Default for SpscQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for SpscQueue<T, N> {
    fn drop(&mut self) {
        while self.try_pop().is_some() {}
    }
}

pub struct SpscProducer<T, const N: usize> {
    queue: Arc<SpscQueue<T, N>>,
}

impl<T, const N: usize> SpscProducer<T, N> {
    pub fn try_push(&self, value: T) -> Result<(), T> {
        self.queue.try_push(value)
    }

    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }

    pub fn is_full(&self) -> bool {
        self.queue.is_full()
    }
}

pub struct SpscConsumer<T, const N: usize> {
    queue: Arc<SpscQueue<T, N>>,
}

impl<T, const N: usize> SpscConsumer<T, N> {
    pub fn try_pop(&self) -> Option<T> {
        self.queue.try_pop()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
