#![allow(clippy::missing_safety_doc)]

#[repr(C)]
#[derive(Debug)]
pub struct CausmString {
    pub data: *const u8,
    pub len: usize,
}

impl CausmString {
    pub fn as_str(&self) -> &str {
        if self.len == 0 || self.data.is_null() {
            ""
        } else {
            unsafe {
                let slice = std::slice::from_raw_parts(self.data, self.len);
                std::str::from_utf8(slice).unwrap_or("")
            }
        }
    }
}

pub unsafe extern "C" fn causm_str_new(
    data: *const u8,
    len: usize,
) -> *mut CausmString {
    let boxed_data = if len > 0 && !data.is_null() {
        let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
        let ptr = std::alloc::alloc(layout);
        if ptr.is_null() {
            panic!("Out of memory in causm_str_new");
        }
        std::ptr::copy_nonoverlapping(data, ptr, len);
        ptr as *const u8
    } else {
        std::ptr::null()
    };

    Box::into_raw(Box::new(CausmString {
        data: boxed_data,
        len,
    }))
}

pub unsafe extern "C" fn causm_str_len(s: *const CausmString) -> usize {
    if s.is_null() {
        0
    } else {
        (*s).len
    }
}

pub unsafe extern "C" fn causm_str_concat(
    lhs: *const CausmString,
    rhs: *const CausmString,
) -> *mut CausmString {
    if lhs.is_null() && rhs.is_null() {
        return causm_str_new(std::ptr::null(), 0);
    }
    if lhs.is_null() {
        return causm_str_new((*rhs).data, (*rhs).len);
    }
    if rhs.is_null() {
        return causm_str_new((*lhs).data, (*lhs).len);
    }

    let lhs_ref = &*lhs;
    let rhs_ref = &*rhs;
    let new_len = lhs_ref.len + rhs_ref.len;

    let new_data = if new_len > 0 {
        let layout = std::alloc::Layout::from_size_align(new_len, 1).unwrap();
        let ptr = std::alloc::alloc(layout);
        if ptr.is_null() {
            panic!("Out of memory in causm_str_concat");
        }
        if lhs_ref.len > 0 && !lhs_ref.data.is_null() {
            std::ptr::copy_nonoverlapping(lhs_ref.data, ptr, lhs_ref.len);
        }
        if rhs_ref.len > 0 && !rhs_ref.data.is_null() {
            std::ptr::copy_nonoverlapping(
                rhs_ref.data,
                ptr.add(lhs_ref.len),
                rhs_ref.len,
            );
        }
        ptr as *const u8
    } else {
        std::ptr::null()
    };

    Box::into_raw(Box::new(CausmString {
        data: new_data,
        len: new_len,
    }))
}

pub unsafe extern "C" fn causm_str_compare(
    lhs: *const CausmString,
    rhs: *const CausmString,
) -> i64 {
    if lhs == rhs {
        return 1;
    }
    if lhs.is_null() || rhs.is_null() {
        return 0;
    }
    let lhs_ref = &*lhs;
    let rhs_ref = &*rhs;
    if lhs_ref.len != rhs_ref.len {
        return 0;
    }
    if lhs_ref.len == 0 {
        return 1;
    }
    let slice_lhs = std::slice::from_raw_parts(lhs_ref.data, lhs_ref.len);
    let slice_rhs = std::slice::from_raw_parts(rhs_ref.data, rhs_ref.len);
    if slice_lhs == slice_rhs {
        1
    } else {
        0
    }
}

pub unsafe extern "C" fn causm_str_slice(
    s: *const CausmString,
    start: isize,
    end: isize,
) -> *mut CausmString {
    if s.is_null() || (*s).len == 0 || (*s).data.is_null() {
        return causm_str_new(std::ptr::null(), 0);
    }
    let s_ref = &*s;
    let len = s_ref.len as isize;
    let mut start = if start < 0 { start + len } else { start };
    let mut end = if end < 0 { end + len } else { end };
    start = start.clamp(0, len);
    end = end.clamp(0, len);
    if end < start {
        end = start;
    }
    let slice_len = (end - start) as usize;
    if slice_len == 0 {
        return causm_str_new(std::ptr::null(), 0);
    }
    let ptr = s_ref.data.add(start as usize);
    causm_str_new(ptr, slice_len)
}

pub unsafe extern "C" fn causm_str_index(
    s: *const CausmString,
    index: usize,
) -> *mut CausmString {
    if s.is_null() {
        panic!("Null pointer in causm_str_index");
    }
    let s_ref = &*s;
    if index >= s_ref.len {
        panic!(
            "Index out of bounds in causm_str_index: {} >= {}",
            index, s_ref.len
        );
    }
    let byte_val = *s_ref.data.add(index);
    let layout = std::alloc::Layout::from_size_align(1, 1).unwrap();
    let ptr = std::alloc::alloc(layout);
    if ptr.is_null() {
        panic!("Out of memory in causm_str_index");
    }
    *ptr = byte_val;
    Box::into_raw(Box::new(CausmString {
        data: ptr as *const u8,
        len: 1,
    }))
}
