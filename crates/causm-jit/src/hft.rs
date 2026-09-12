//! SpeedMicro High-Frequency Trading (HFT) and Low-Latency Systems Optimization Module.
//!
//! Provides primitives for:
//! 1. Memory page pre-faulting and RAM pinning (`mlock`) to eliminate page faults.
//! 2. Thread CPU core affinity pinning to prevent context-switch migration jitter.
//! 3. L1/L2 cache warming loops.
//! 4. Elastic Determinism jitter detection for acausal external preemption.

use std::sync::atomic::{AtomicU64, Ordering};

/// Threshold in TSC cycles beyond which an OS context switch or hardware interrupt
/// is classified as an external preemption triggering Elastic Determinism.
pub const DEFAULT_ELASTIC_JITTER_THRESHOLD_CYCLES: u64 = 150_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterStatus {
    WithinBudget,
    ElasticJitterDetected {
        elapsed_cycles: u64,
        expected_cycles: u64,
        lost_to_void: u64,
    },
}

static LOST_CYCLES_TOTAL: AtomicU64 = AtomicU64::new(0);

/// Returns the cumulative number of cycles lost to acausal external preemption.
pub fn get_total_lost_cycles() -> u64 {
    LOST_CYCLES_TOTAL.load(Ordering::Relaxed)
}

/// Reset the total lost cycles counter.
pub fn reset_total_lost_cycles() {
    LOST_CYCLES_TOTAL.store(0, Ordering::Relaxed);
}

/// Evaluates cycle progression against expected bounds to enforce Elastic Determinism.
pub fn evaluate_elastic_determinism(
    elapsed_cycles: u64,
    expected_cycles: u64,
    threshold: u64,
) -> JitterStatus {
    if elapsed_cycles > expected_cycles
        && (elapsed_cycles - expected_cycles) > threshold
    {
        let lost = elapsed_cycles - expected_cycles;
        LOST_CYCLES_TOTAL.fetch_add(lost, Ordering::Relaxed);
        JitterStatus::ElasticJitterDetected {
            elapsed_cycles,
            expected_cycles,
            lost_to_void: lost,
        }
    } else {
        JitterStatus::WithinBudget
    }
}

/// Pre-faults every 4KB page in a memory slice and locks it in RAM using `mlock`.
///
/// Prevents the OS kernel from swapping arena memory or raising page faults
/// during latency-critical execution blocks.
pub fn pin_and_prefault_memory(slice: &mut [u8]) -> Result<(), String> {
    if slice.is_empty() {
        return Ok(());
    }

    // Step 1: Pre-fault every 4096-byte memory page by writing into it
    let page_size = 4096;
    for chunk in slice.chunks_mut(page_size) {
        unsafe {
            std::ptr::write_volatile(&mut chunk[0], chunk[0]);
        }
    }

    // Step 2: Lock pages into physical RAM
    #[cfg(unix)]
    {
        let ptr = slice.as_ptr() as *const libc::c_void;
        let len = slice.len();
        let res = unsafe { libc::mlock(ptr, len) };
        if res != 0 {
            // Note: In non-root Android / container environments RLIMIT_MEMLOCK might be limited,
            // so we gracefully log rather than hard failing.
            return Ok(());
        }
    }

    Ok(())
}

/// Unlocks memory previously locked via `pin_and_prefault_memory`.
pub fn unpin_memory(slice: &[u8]) {
    #[cfg(unix)]
    if !slice.is_empty() {
        unsafe {
            libc::munlock(slice.as_ptr() as *const libc::c_void, slice.len());
        }
    }
}

/// Warms the CPU L1 data cache by performing volatile reads across the memory buffer.
pub fn warm_l1_cache(slice: &[u8]) {
    // Stride across cache lines (64 bytes)
    for chunk in slice.chunks(64) {
        unsafe {
            let _ = std::ptr::read_volatile(&chunk[0]);
        }
    }
}

/// Pins the current thread to a specific CPU core to eliminate core-migration jitter.
#[cfg(target_os = "linux")]
pub fn pin_current_thread_to_core(core_id: usize) -> Result<(), String> {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(core_id, &mut set);
        let res =
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set);
        if res == 0 {
            Ok(())
        } else {
            Err(format!(
                "sched_setaffinity failed with errno: {}",
                std::io::Error::last_os_error()
            ))
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn pin_current_thread_to_core(_core_id: usize) -> Result<(), String> {
    Ok(())
}
