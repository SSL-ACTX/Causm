//! Native runtime intrinsics linked into the Cranelift JIT.

use std::alloc::{alloc, dealloc, Layout};

pub extern "C" fn causm_alloc(size: usize, align: usize) -> *mut u8 {
    let align = if align == 0 { 8 } else { align };
    let layout =
        Layout::from_size_align(size, align).unwrap_or(Layout::new::<u64>());
    unsafe { alloc(layout) }
}

/// # Safety
/// Caller must pass a pointer allocated by `causm_alloc` with matching size and alignment.
pub unsafe extern "C" fn causm_dealloc(ptr: *mut u8, size: usize, align: usize) {
    if !ptr.is_null() && size > 0 {
        let align = if align == 0 { 8 } else { align };
        let layout =
            Layout::from_size_align(size, align).unwrap_or(Layout::new::<u64>());
        dealloc(ptr, layout);
    }
}

pub extern "C" fn causm_sin(x: f64) -> f64 {
    x.sin()
}
pub extern "C" fn causm_cos(x: f64) -> f64 {
    x.cos()
}
pub extern "C" fn causm_tan(x: f64) -> f64 {
    x.tan()
}
pub extern "C" fn causm_asin(x: f64) -> f64 {
    x.asin()
}
pub extern "C" fn causm_acos(x: f64) -> f64 {
    x.acos()
}
pub extern "C" fn causm_atan(x: f64) -> f64 {
    x.atan()
}
pub extern "C" fn causm_exp(x: f64) -> f64 {
    x.exp()
}
pub extern "C" fn causm_log(x: f64) -> f64 {
    x.ln()
}
pub extern "C" fn causm_pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}
pub extern "C" fn causm_floor(x: f64) -> f64 {
    x.floor()
}
pub extern "C" fn causm_ceil(x: f64) -> f64 {
    x.ceil()
}
pub extern "C" fn causm_round(x: f64) -> f64 {
    x.round()
}
pub extern "C" fn causm_sqrt(x: f64) -> f64 {
    x.sqrt()
}
