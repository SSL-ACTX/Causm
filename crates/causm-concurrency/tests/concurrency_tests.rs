use causm_concurrency::mailbox::{
    BoundedMailbox, MailboxOverflowAction, SaturationPolicy,
};
use causm_concurrency::queue::{MpmcQueue, SpscQueue};
use causm_concurrency::scheduler::{ActorPool, TimeSlice};
use causm_concurrency::sync::{AtomicBool, AtomicInt, BoundedChannel, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_spsc_queue_basic_operations() {
    let queue: SpscQueue<i32, 4> = SpscQueue::new();
    assert!(queue.is_empty());
    assert_eq!(queue.capacity(), 4);

    assert!(queue.try_push(10).is_ok());
    assert!(queue.try_push(20).is_ok());
    assert!(queue.try_push(30).is_ok());
    assert!(queue.try_push(40).is_ok());

    assert!(queue.is_full());
    assert_eq!(queue.len(), 4);

    // Overflow push fails
    assert_eq!(queue.try_push(50), Err(50));

    assert_eq!(queue.try_pop(), Some(10));
    assert_eq!(queue.try_pop(), Some(20));
    assert_eq!(queue.try_pop(), Some(30));
    assert_eq!(queue.try_pop(), Some(40));
    assert_eq!(queue.try_pop(), None);
    assert!(queue.is_empty());
}

#[test]
fn test_bounded_mailbox_fail_fast_policy() {
    let mut mb_fail_fast: BoundedMailbox<i32> =
        BoundedMailbox::new(2, SaturationPolicy::FailFast);
    assert!(mb_fail_fast.push(1).is_ok());
    assert!(mb_fail_fast.push(2).is_ok());

    let err = mb_fail_fast.push(3);
    assert!(matches!(
        err,
        Err(causm_concurrency::mailbox::MailboxError::Full(3))
    ));

    assert_eq!(mb_fail_fast.pop(), Some(1));
    assert_eq!(mb_fail_fast.pop(), Some(2));
    assert_eq!(mb_fail_fast.pop(), None);
}

#[test]
fn test_spsc_queue_threaded_producer_consumer() {
    let queue: SpscQueue<usize, 64> = SpscQueue::new();
    let (producer, consumer) = queue.split();

    let count = 1000;
    let p_handle = thread::spawn(move || {
        for i in 0..count {
            while producer.try_push(i).is_err() {
                thread::yield_now();
            }
        }
    });

    let c_handle = thread::spawn(move || {
        let mut received = Vec::new();
        while received.len() < count {
            if let Some(val) = consumer.try_pop() {
                received.push(val);
            } else {
                thread::yield_now();
            }
        }
        received
    });

    p_handle.join().unwrap();
    let res = c_handle.join().unwrap();
    assert_eq!(res.len(), count);
    for (i, &val) in res.iter().enumerate() {
        assert_eq!(i, val);
    }
}

#[test]
fn test_mpmc_queue_multi_threaded() {
    let queue = Arc::new(MpmcQueue::<usize, 64>::new());
    let total_items = 2000;
    let producers_count = 4;
    let consumers_count = 4;
    let items_per_producer = total_items / producers_count;

    let mut p_handles = Vec::new();
    for p in 0..producers_count {
        let q = Arc::clone(&queue);
        p_handles.push(thread::spawn(move || {
            for i in 0..items_per_producer {
                let val = p * items_per_producer + i;
                while q.try_push(val).is_err() {
                    thread::yield_now();
                }
            }
        }));
    }

    let consumed_sum = Arc::new(AtomicUsize::new(0));
    let consumed_count = Arc::new(AtomicUsize::new(0));
    let mut c_handles = Vec::new();

    for _ in 0..consumers_count {
        let q = Arc::clone(&queue);
        let sum = Arc::clone(&consumed_sum);
        let count = Arc::clone(&consumed_count);
        c_handles.push(thread::spawn(move || loop {
            if count.load(Ordering::Acquire) >= total_items {
                break;
            }
            if let Some(val) = q.try_pop() {
                sum.fetch_add(val, Ordering::Relaxed);
                count.fetch_add(1, Ordering::Release);
            } else {
                thread::yield_now();
            }
        }));
    }

    for h in p_handles {
        h.join().unwrap();
    }
    for h in c_handles {
        h.join().unwrap();
    }

    assert_eq!(consumed_count.load(Ordering::Acquire), total_items);
    let expected_sum: usize = (0..total_items).sum();
    assert_eq!(consumed_sum.load(Ordering::Acquire), expected_sum);
}

#[test]
fn test_bounded_mailbox_saturation_policies() {
    // 1. RingBuffer Policy
    let mut mb_ring: BoundedMailbox<i32> =
        BoundedMailbox::new(3, SaturationPolicy::RingBuffer);
    assert_eq!(mb_ring.push(1).unwrap(), None);
    assert_eq!(mb_ring.push(2).unwrap(), None);
    assert_eq!(mb_ring.push(3).unwrap(), None);
    assert!(mb_ring.is_full());

    // Overflow replaces oldest (1 evicted)
    assert_eq!(
        mb_ring.push(4).unwrap(),
        Some(MailboxOverflowAction::EvictedOldest)
    );
    assert_eq!(mb_ring.pop(), Some(2));
    assert_eq!(mb_ring.pop(), Some(3));
    assert_eq!(mb_ring.pop(), Some(4));
    assert_eq!(mb_ring.pop(), None);

    // 2. Throttle Policy
    let mut mb_throttle: BoundedMailbox<i32> =
        BoundedMailbox::new(2, SaturationPolicy::Throttle);
    assert!(mb_throttle.push(10).is_ok());
    assert!(mb_throttle.push(20).is_ok());
    assert!(mb_throttle.push(30).is_err());
    assert_eq!(mb_throttle.pop(), Some(10));
    assert_eq!(mb_throttle.pop(), Some(20));
}

#[test]
fn test_atomic_int_and_bool_primitives() {
    let a = AtomicInt::new(10);
    assert_eq!(a.load(), 10);
    assert_eq!(a.fetch_add(5), 10);
    assert_eq!(a.load(), 15);
    assert!(a.compare_exchange(15, 100));
    assert_eq!(a.load(), 100);

    let b = AtomicBool::new(false);
    assert!(!b.load());
    assert!(b.compare_exchange(false, true));
    assert!(b.load());
}

#[test]
fn test_mutex_and_channel_primitives() {
    let lock = Mutex::new();
    assert!(!lock.is_locked());
    assert!(lock.try_lock("worker_1"));
    assert!(lock.is_locked());
    assert_eq!(lock.owner(), Some("worker_1".to_string()));
    assert!(!lock.try_lock("worker_2"));
    assert!(lock.unlock());
    assert!(!lock.is_locked());

    let ch = BoundedChannel::new(2);
    assert!(ch.send(42));
    assert!(ch.send(84));
    assert!(ch.is_full());
    assert_eq!(ch.recv(), Some(42));
    assert_eq!(ch.recv(), Some(84));
    assert!(ch.is_empty());
    ch.close();
    assert!(ch.is_closed());
    assert_eq!(ch.recv(), None);
}

#[test]
fn test_scheduler_time_slice_tracking() {
    let mut slice = TimeSlice::from_millis(50);
    assert_eq!(slice.budget(), Duration::from_millis(50));
    assert!(!slice.is_expired());

    slice.start();
    thread::sleep(Duration::from_millis(10));
    slice.pause();

    assert!(slice.elapsed() >= Duration::from_millis(9));
    assert!(!slice.is_expired());
    assert!(slice.remaining() > Duration::ZERO);
}

#[test]
fn test_actor_pool_cooperative_dispatch() {
    let mut pool: ActorPool<String> = ActorPool::new();

    let mb1 = BoundedMailbox::new(8, SaturationPolicy::RingBuffer);
    let mb2 = BoundedMailbox::new(8, SaturationPolicy::RingBuffer);

    pool.register_actor("SensorA".to_string(), mb1, Duration::from_millis(5));
    pool.register_actor("ActuatorB".to_string(), mb2, Duration::from_millis(5));

    assert_eq!(pool.active_actors_count(), 2);

    pool.send_to("ActuatorB", "START_PUMP".to_string()).unwrap();

    let next_actor = pool.next_ready_actor().unwrap();
    assert_eq!(next_actor, "SensorA");

    let next_actor2 = pool.next_ready_actor().unwrap();
    assert_eq!(next_actor2, "ActuatorB");

    let actor = pool.get_actor_mut("ActuatorB").unwrap();
    assert_eq!(actor.mailbox.pop(), Some("START_PUMP".to_string()));
}
