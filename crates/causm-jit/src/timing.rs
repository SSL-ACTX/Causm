//! Hardware timing and cycle accuracy primitives.

#[cfg(target_arch = "x86_64")]
pub extern "C" fn read_tsc() -> u64 {
    let mut aux: u32 = 0;
    unsafe { core::arch::x86_64::__rdtscp(&mut aux) }
}

#[cfg(target_arch = "aarch64")]
pub extern "C" fn read_tsc() -> u64 {
    let mut val: u64;
    unsafe {
        std::arch::asm!("mrs {}, cntvct_el0", out(reg) val);
    }
    val
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub extern "C" fn read_tsc() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub extern "C" fn spin_pad(target_cycles: u64) {
    let start = read_tsc();
    while read_tsc().saturating_sub(start) < target_cycles {
        std::hint::spin_loop();
    }
}
