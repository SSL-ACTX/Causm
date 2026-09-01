use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool as StdAtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};

#[derive(Debug, Clone)]
pub struct AtomicInt {
    inner: Arc<AtomicI64>,
}

impl AtomicInt {
    pub fn new(initial: i64) -> Self {
        Self {
            inner: Arc::new(AtomicI64::new(initial)),
        }
    }

    pub fn load(&self) -> i64 {
        self.inner.load(Ordering::SeqCst)
    }

    pub fn store(&self, value: i64) {
        self.inner.store(value, Ordering::SeqCst)
    }

    pub fn fetch_add(&self, delta: i64) -> i64 {
        self.inner.fetch_add(delta, Ordering::SeqCst)
    }

    pub fn compare_exchange(&self, expected: i64, desired: i64) -> bool {
        self.inner
            .compare_exchange(expected, desired, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

#[derive(Debug, Clone)]
pub struct AtomicBool {
    inner: Arc<StdAtomicBool>,
}

impl AtomicBool {
    pub fn new(initial: bool) -> Self {
        Self {
            inner: Arc::new(StdAtomicBool::new(initial)),
        }
    }

    pub fn load(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }

    pub fn store(&self, value: bool) {
        self.inner.store(value, Ordering::SeqCst)
    }

    pub fn compare_exchange(&self, expected: bool, desired: bool) -> bool {
        self.inner
            .compare_exchange(expected, desired, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Mutex {
    locked: Arc<StdAtomicBool>,
    owner: Arc<StdMutex<Option<String>>>,
}

impl Mutex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_lock(&self, owner: impl Into<String>) -> bool {
        let owner = owner.into();
        if self
            .locked
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            false
        } else {
            *self.owner.lock().unwrap() = Some(owner);
            true
        }
    }

    pub fn unlock(&self) -> bool {
        if self
            .locked
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            *self.owner.lock().unwrap() = None;
            true
        } else {
            false
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::SeqCst)
    }

    pub fn owner(&self) -> Option<String> {
        self.owner.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone)]
pub struct BoundedChannel<T> {
    capacity: usize,
    buffer: Arc<StdMutex<VecDeque<T>>>,
    closed: Arc<StdAtomicBool>,
    cond: Arc<Condvar>,
}

impl<T> BoundedChannel<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "channel capacity must be greater than zero");
        Self {
            capacity,
            buffer: Arc::new(StdMutex::new(VecDeque::new())),
            closed: Arc::new(StdAtomicBool::new(false)),
            cond: Arc::new(Condvar::new()),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.buffer.lock().unwrap().len() >= self.capacity
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub fn send(&self, value: T) -> bool {
        let mut queue = self.buffer.lock().unwrap();
        while queue.len() >= self.capacity && !self.closed.load(Ordering::SeqCst) {
            queue = self.cond.wait(queue).unwrap();
        }

        if self.closed.load(Ordering::SeqCst) {
            return false;
        }

        queue.push_back(value);
        self.cond.notify_one();
        true
    }

    pub fn recv(&self) -> Option<T> {
        let mut queue = self.buffer.lock().unwrap();
        loop {
            if let Some(value) = queue.pop_front() {
                self.cond.notify_one();
                return Some(value);
            }
            if self.closed.load(Ordering::SeqCst) {
                return None;
            }
            queue = self.cond.wait(queue).unwrap();
        }
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.cond.notify_all();
    }
}
