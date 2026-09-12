//! Native heap array and aggregate memory layout operations.

#[repr(C)]
pub struct CausmArray {
    pub len: usize,
    pub cap: usize,
    pub data: *mut i64,
}

pub extern "C" fn causm_array_new(initial_cap: usize) -> *mut CausmArray {
    let cap = if initial_cap == 0 { 8 } else { initial_cap };
    let data = unsafe {
        let layout = std::alloc::Layout::array::<i64>(cap).unwrap();
        std::alloc::alloc(layout) as *mut i64
    };
    Box::into_raw(Box::new(CausmArray { len: 0, cap, data }))
}

/// # Safety
/// Caller must ensure `arr_ptr` is either null or a valid pointer to `CausmArray`.
pub unsafe extern "C" fn causm_array_push(
    arr_ptr: *mut CausmArray,
    val: i64,
) -> *mut CausmArray {
    if arr_ptr.is_null() {
        return arr_ptr;
    }
    let arr = &mut *arr_ptr;
    if arr.len >= arr.cap {
        let new_cap = arr.cap * 2;
        let old_layout = std::alloc::Layout::array::<i64>(arr.cap).unwrap();
        let new_data = std::alloc::realloc(
            arr.data as *mut u8,
            old_layout,
            new_cap * std::mem::size_of::<i64>(),
        ) as *mut i64;
        arr.data = new_data;
        arr.cap = new_cap;
    }
    *arr.data.add(arr.len) = val;
    arr.len += 1;
    arr_ptr
}

/// # Safety
/// Caller must ensure `arr_ptr` is either null or a valid pointer to `CausmArray`.
pub unsafe extern "C" fn causm_array_get(
    arr_ptr: *const CausmArray,
    index: usize,
) -> i64 {
    if arr_ptr.is_null() {
        return 0;
    }
    let arr = &*arr_ptr;
    if index < arr.len {
        *arr.data.add(index)
    } else {
        0
    }
}

/// # Safety
/// Caller must ensure `arr_ptr` is either null or a valid pointer to `CausmArray`.
pub unsafe extern "C" fn causm_array_set(
    arr_ptr: *mut CausmArray,
    index: usize,
    val: i64,
) {
    if !arr_ptr.is_null() {
        let arr = &mut *arr_ptr;
        if index < arr.len {
            *arr.data.add(index) = val;
        }
    }
}

/// # Safety
/// Caller must ensure `arr_ptr` is either null or a valid pointer to `CausmArray`.
pub unsafe extern "C" fn causm_array_len(arr_ptr: *const CausmArray) -> usize {
    if arr_ptr.is_null() {
        0
    } else {
        (*arr_ptr).len
    }
}
