use crate::vm::error::TemporalError;
use causm_core::types::Type;
use causm_core::value::Payload;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::Mutex;

#[cfg(unix)]
type RawHandle = *mut libc::c_void;

#[cfg(not(unix))]
type RawHandle = *mut std::ffi::c_void;

pub struct ForeignLibraryManager {
    #[cfg_attr(not(unix), allow(dead_code))]
    handles: Mutex<HashMap<String, usize>>,
}

impl Default for ForeignLibraryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ForeignLibraryManager {
    pub fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(unix)]
    pub fn get_or_load_symbol(
        &self,
        lib_name: &str,
        symbol: &str,
    ) -> Result<*mut libc::c_void, TemporalError> {
        let mut map = self.handles.lock().unwrap();
        let handle = if let Some(&h) = map.get(lib_name) {
            h as RawHandle
        } else {
            // Attempt to load library using dlopen
            let c_lib = CString::new(lib_name).map_err(|_| {
                TemporalError::EvalError(format!(
                    "Invalid library path: {}",
                    lib_name
                ))
            })?;

            let mut h = unsafe { libc::dlopen(c_lib.as_ptr(), libc::RTLD_LAZY) };
            if h.is_null() {
                // If lib_name is "libc.so.6" and on Android/Termux, try "libc.so" or RTLD_DEFAULT
                if lib_name.contains("libc") {
                    let fallback_name = CString::new("libc.so").unwrap();
                    h = unsafe {
                        libc::dlopen(fallback_name.as_ptr(), libc::RTLD_LAZY)
                    };
                }
            }
            if h.is_null() {
                // Try RTLD_DEFAULT
                h = libc::RTLD_DEFAULT;
            }

            map.insert(lib_name.to_string(), h as usize);
            h
        };

        let c_sym = CString::new(symbol).map_err(|_| {
            TemporalError::EvalError(format!("Invalid symbol name: {}", symbol))
        })?;

        let sym_ptr = unsafe { libc::dlsym(handle, c_sym.as_ptr()) };
        if sym_ptr.is_null() {
            // Try RTLD_DEFAULT as fallback lookup
            let default_ptr =
                unsafe { libc::dlsym(libc::RTLD_DEFAULT, c_sym.as_ptr()) };
            if !default_ptr.is_null() {
                return Ok(default_ptr);
            }
            return Err(TemporalError::EvalError(format!(
                "FFI symbol '{}' not found in '{}'",
                symbol, lib_name
            )));
        }

        Ok(sym_ptr)
    }

    #[cfg(not(unix))]
    pub fn get_or_load_symbol(
        &self,
        lib_name: &str,
        symbol: &str,
    ) -> Result<RawHandle, TemporalError> {
        match symbol {
            "socket" => Ok(wasm_socket as RawHandle),
            "connect" => Ok(wasm_connect as RawHandle),
            "send" | "write" => Ok(wasm_vfs_write as RawHandle),
            "recv" | "read" => Ok(wasm_vfs_read as RawHandle),
            "close" => Ok(wasm_vfs_close as RawHandle),
            "inet_addr" => Ok(wasm_inet_addr as RawHandle),
            "open" | "creat" => Ok(wasm_vfs_open as RawHandle),
            "lseek" => Ok(wasm_vfs_lseek as RawHandle),
            "unlink" => Ok(wasm_vfs_unlink as RawHandle),
            "access" => Ok(wasm_vfs_access as RawHandle),
            "rename" => Ok(wasm_vfs_rename as RawHandle),
            "mkdir" => Ok(wasm_vfs_mkdir as RawHandle),
            "rmdir" => Ok(wasm_vfs_rmdir as RawHandle),
            "fsync" | "ftruncate" => Ok(wasm_vfs_close as RawHandle), // no-op success
            _ => Err(TemporalError::EvalError(format!(
                "Dynamic FFI dlopen not supported on non-unix platform for {}::{}",
                lib_name, symbol
            ))),
        }
    }
}

#[cfg(not(unix))]
static WASM_SOCKETS: std::sync::Mutex<Option<HashMap<i32, std::net::TcpStream>>> =
    std::sync::Mutex::new(None);

#[cfg(not(unix))]
#[derive(Clone, Default)]
struct VirtualFileNode {
    data: Vec<u8>,
}

#[cfg(not(unix))]
#[derive(Clone)]
struct VirtualFileHandle {
    path: String,
    cursor: usize,
}

#[cfg(not(unix))]
static VIRTUAL_FS: std::sync::Mutex<Option<HashMap<String, VirtualFileNode>>> =
    std::sync::Mutex::new(None);

#[cfg(not(unix))]
static VIRTUAL_HANDLES: std::sync::Mutex<Option<HashMap<i32, VirtualFileHandle>>> =
    std::sync::Mutex::new(None);

#[cfg(not(unix))]
pub fn vfs_total_bytes() -> usize {
    let fs_guard = VIRTUAL_FS.lock().unwrap();
    if let Some(fs) = fs_guard.as_ref() {
        fs.values().map(|node| node.data.len()).sum()
    } else {
        0
    }
}

#[cfg(not(unix))]
pub fn vfs_reset() {
    let mut fs_guard = VIRTUAL_FS.lock().unwrap();
    if let Some(fs) = fs_guard.as_mut() {
        fs.clear();
    }
    let mut handles_guard = VIRTUAL_HANDLES.lock().unwrap();
    if let Some(handles) = handles_guard.as_mut() {
        handles.clear();
    }
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_open(
    path_ptr: *const std::ffi::c_char,
    flags: i32,
    _mode: i32,
) -> i32 {
    if path_ptr.is_null() {
        return -1;
    }
    let c_str = unsafe { CStr::from_ptr(path_ptr) };
    let path = match c_str.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };

    let mut fs_guard = VIRTUAL_FS.lock().unwrap();
    let fs = fs_guard.get_or_insert_with(HashMap::new);

    // If writing/creating and not exists, create entry
    let is_write = (flags & 512 != 0)
        || (flags & 1 != 0)
        || (flags & 2 != 0)
        || (flags & 577 != 0)
        || (flags & 1089 != 0);
    if is_write && !fs.contains_key(&path) {
        fs.insert(path.clone(), VirtualFileNode { data: Vec::new() });
    }

    if !fs.contains_key(&path) && !is_write {
        return -1;
    }

    let fd = NEXT_WASM_FD.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut handles_guard = VIRTUAL_HANDLES.lock().unwrap();
    let handles = handles_guard.get_or_insert_with(HashMap::new);
    let initial_cursor = if (flags & 1024 != 0) || (flags & 1089 != 0) {
        fs.get(&path).map(|f| f.data.len()).unwrap_or(0)
    } else {
        0
    };

    handles.insert(
        fd,
        VirtualFileHandle {
            path,
            cursor: initial_cursor,
        },
    );
    fd
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_read(fd: i32, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() {
        return -1;
    }
    let mut handles_guard = VIRTUAL_HANDLES.lock().unwrap();
    if let Some(handles) = handles_guard.as_mut() {
        if let Some(h) = handles.get_mut(&fd) {
            let fs_guard = VIRTUAL_FS.lock().unwrap();
            if let Some(fs) = fs_guard.as_ref() {
                if let Some(node) = fs.get(&h.path) {
                    if h.cursor >= node.data.len() {
                        return 0;
                    }
                    let avail = node.data.len() - h.cursor;
                    let to_read = std::cmp::min(avail, count);
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            node.data.as_ptr().add(h.cursor),
                            buf,
                            to_read,
                        );
                    }
                    h.cursor += to_read;
                    return to_read as isize;
                }
            }
        }
    }
    // Fall back to socket read if it was a socket
    wasm_recv(fd, buf, count, 0)
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_write(fd: i32, buf: *const u8, count: usize) -> isize {
    if buf.is_null() {
        return -1;
    }
    let mut handles_guard = VIRTUAL_HANDLES.lock().unwrap();
    if let Some(handles) = handles_guard.as_mut() {
        if let Some(h) = handles.get_mut(&fd) {
            let mut fs_guard = VIRTUAL_FS.lock().unwrap();
            let fs = fs_guard.get_or_insert_with(HashMap::new);
            let node = fs
                .entry(h.path.clone())
                .or_insert_with(|| VirtualFileNode { data: Vec::new() });
            let slice = unsafe { std::slice::from_raw_parts(buf, count) };
            if h.cursor + count > node.data.len() {
                node.data.resize(h.cursor + count, 0);
            }
            node.data[h.cursor..h.cursor + count].copy_from_slice(slice);
            h.cursor += count;
            return count as isize;
        }
    }
    // Fall back to socket send if it was a socket
    wasm_send(fd, buf, count, 0)
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_lseek(fd: i32, offset: i64, whence: i32) -> i64 {
    let mut handles_guard = VIRTUAL_HANDLES.lock().unwrap();
    if let Some(handles) = handles_guard.as_mut() {
        if let Some(h) = handles.get_mut(&fd) {
            let fs_guard = VIRTUAL_FS.lock().unwrap();
            if let Some(fs) = fs_guard.as_ref() {
                if let Some(node) = fs.get(&h.path) {
                    let new_cursor = match whence {
                        0 => offset as usize,                            // SEEK_SET
                        1 => (h.cursor as i64 + offset).max(0) as usize, // SEEK_CUR
                        2 => (node.data.len() as i64 + offset).max(0) as usize, // SEEK_END
                        _ => h.cursor,
                    };
                    h.cursor = new_cursor;
                    return new_cursor as i64;
                }
            }
        }
    }
    -1
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_close(fd: i32) -> i32 {
    let mut handles_guard = VIRTUAL_HANDLES.lock().unwrap();
    if let Some(handles) = handles_guard.as_mut() {
        if handles.remove(&fd).is_some() {
            return 0;
        }
    }
    wasm_close(fd)
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_unlink(path_ptr: *const std::ffi::c_char) -> i32 {
    if path_ptr.is_null() {
        return -1;
    }
    let c_str = unsafe { CStr::from_ptr(path_ptr) };
    if let Ok(path) = c_str.to_str() {
        let mut fs_guard = VIRTUAL_FS.lock().unwrap();
        if let Some(fs) = fs_guard.as_mut() {
            if fs.remove(path).is_some() {
                return 0;
            }
        }
    }
    -1
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_access(path_ptr: *const std::ffi::c_char, _mode: i32) -> i32 {
    if path_ptr.is_null() {
        return -1;
    }
    let c_str = unsafe { CStr::from_ptr(path_ptr) };
    if let Ok(path) = c_str.to_str() {
        let fs_guard = VIRTUAL_FS.lock().unwrap();
        if let Some(fs) = fs_guard.as_ref() {
            if fs.contains_key(path) {
                return 0;
            }
        }
    }
    -1
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_rename(
    old_ptr: *const std::ffi::c_char,
    new_ptr: *const std::ffi::c_char,
) -> i32 {
    if old_ptr.is_null() || new_ptr.is_null() {
        return -1;
    }
    let old_s = match unsafe { CStr::from_ptr(old_ptr) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    let new_s = match unsafe { CStr::from_ptr(new_ptr) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    let mut fs_guard = VIRTUAL_FS.lock().unwrap();
    if let Some(fs) = fs_guard.as_mut() {
        if let Some(node) = fs.remove(&old_s) {
            fs.insert(new_s, node);
            return 0;
        }
    }
    -1
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_mkdir(path_ptr: *const std::ffi::c_char, _mode: i32) -> i32 {
    if path_ptr.is_null() {
        return -1;
    }
    let path = match unsafe { CStr::from_ptr(path_ptr) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return -1,
    };
    let mut fs_guard = VIRTUAL_FS.lock().unwrap();
    let fs = fs_guard.get_or_insert_with(HashMap::new);
    fs.insert(path, VirtualFileNode { data: Vec::new() });
    0
}

#[cfg(not(unix))]
extern "C" fn wasm_vfs_rmdir(path_ptr: *const std::ffi::c_char) -> i32 {
    wasm_vfs_unlink(path_ptr)
}

#[cfg(not(unix))]
static NEXT_WASM_FD: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(100);

#[cfg(not(unix))]
extern "C" fn wasm_socket(_domain: i32, _typ: i32, _proto: i32) -> i32 {
    NEXT_WASM_FD.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

#[cfg(not(unix))]
extern "C" fn wasm_connect(fd: i32, addr_buf: *const u8, _addrlen: i32) -> i32 {
    if addr_buf.is_null() {
        return -1;
    }
    unsafe {
        let slice = std::slice::from_raw_parts(addr_buf, 16);
        let port = ((slice[2] as u16) << 8) | (slice[3] as u16);
        let ip = std::net::Ipv4Addr::new(slice[4], slice[5], slice[6], slice[7]);
        let addr = std::net::SocketAddr::V4(std::net::SocketAddrV4::new(ip, port));
        if let Ok(stream) = std::net::TcpStream::connect_timeout(
            &addr,
            std::time::Duration::from_secs(5),
        ) {
            let mut guard = WASM_SOCKETS.lock().unwrap();
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(fd, stream);
            0
        } else {
            -1
        }
    }
}

#[cfg(not(unix))]
extern "C" fn wasm_send(fd: i32, buf: *const u8, len: usize, _flags: i32) -> isize {
    use std::io::Write;
    if buf.is_null() {
        return -1;
    }
    let mut guard = WASM_SOCKETS.lock().unwrap();
    if let Some(map) = guard.as_mut() {
        if let Some(stream) = map.get_mut(&fd) {
            let slice = unsafe { std::slice::from_raw_parts(buf, len) };
            if let Ok(n) = stream.write(slice) {
                let _ = stream.flush();
                return n as isize;
            }
        }
    }
    -1
}

#[cfg(not(unix))]
extern "C" fn wasm_recv(fd: i32, buf: *mut u8, len: usize, _flags: i32) -> isize {
    use std::io::Read;
    if buf.is_null() {
        return -1;
    }
    let mut guard = WASM_SOCKETS.lock().unwrap();
    if let Some(map) = guard.as_mut() {
        if let Some(stream) = map.get_mut(&fd) {
            let slice = unsafe { std::slice::from_raw_parts_mut(buf, len) };
            if let Ok(n) = stream.read(slice) {
                return n as isize;
            }
        }
    }
    -1
}

#[cfg(not(unix))]
extern "C" fn wasm_close(fd: i32) -> i32 {
    let mut guard = WASM_SOCKETS.lock().unwrap();
    if let Some(map) = guard.as_mut() {
        map.remove(&fd);
    }
    0
}

#[cfg(not(unix))]
extern "C" fn wasm_inet_addr(ip_str: *const std::ffi::c_char) -> u32 {
    if ip_str.is_null() {
        return 0;
    }
    let c_str = unsafe { CStr::from_ptr(ip_str) };
    if let Ok(s) = c_str.to_str() {
        if let Ok(ip) = s.parse::<std::net::Ipv4Addr>() {
            return u32::from_ne_bytes(ip.octets());
        }
    }
    0
}

/// Invokes raw C function pointer with dynamically marshalled arguments.
///
/// # Safety
///
/// `sym_ptr` must be a valid, callable C function pointer matching the native ABI.
pub unsafe fn invoke_foreign_symbol(
    sym_ptr: *mut std::ffi::c_void,
    args: &mut [Payload],
    return_type: &Type,
) -> Result<Payload, TemporalError> {
    // Keep CString allocations alive for the duration of the native call
    let mut c_strings = Vec::new();
    let mut raw_args: Vec<usize> = Vec::new();

    #[cfg(not(unix))]
    {
        // On WASM, sym_ptr is a Rust function pointer matching the signature
        if sym_ptr == (wasm_socket as RawHandle) {
            let d = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(2);
            let t = args
                .get(1)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(1);
            let p = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            return Ok(Payload::Integer(wasm_socket(d, t, p) as i64));
        } else if sym_ptr == (wasm_connect as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            // If the argument is an array payload, get the direct pointer to its elements
            let sa_bytes: Vec<u8> =
                if let Some(Payload::Array(elements)) = args.get(1) {
                    elements
                        .iter()
                        .map(|el| match el {
                            Payload::Integer(v) => *v as u8,
                            _ => 0,
                        })
                        .collect()
                } else {
                    Vec::new()
                };
            let buf_ptr = if !sa_bytes.is_empty() {
                sa_bytes.as_ptr()
            } else {
                args.get(1)
                    .and_then(|p| match p {
                        Payload::Integer(i) => Some(*i as *const u8),
                        _ => None,
                    })
                    .unwrap_or(std::ptr::null())
            };
            let len = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(16);
            return Ok(Payload::Integer(wasm_connect(fd, buf_ptr, len) as i64));
        } else if sym_ptr == (wasm_send as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            let send_bytes: Vec<u8> =
                if let Some(Payload::Array(elements)) = args.get(1) {
                    elements
                        .iter()
                        .map(|el| match el {
                            Payload::Integer(v) => *v as u8,
                            _ => 0,
                        })
                        .collect()
                } else {
                    Vec::new()
                };
            let buf_ptr = if !send_bytes.is_empty() {
                send_bytes.as_ptr()
            } else {
                args.get(1)
                    .and_then(|p| match p {
                        Payload::Integer(i) => Some(*i as *const u8),
                        _ => None,
                    })
                    .unwrap_or(std::ptr::null())
            };
            let len = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as usize),
                    _ => None,
                })
                .unwrap_or(send_bytes.len());
            let flags = args
                .get(3)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            return Ok(Payload::Integer(wasm_send(fd, buf_ptr, len, flags) as i64));
        } else if sym_ptr == (wasm_recv as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            let len = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as usize),
                    _ => None,
                })
                .unwrap_or(512);
            let flags = args
                .get(3)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            let mut recv_buf = vec![0u8; len];
            let res = wasm_recv(fd, recv_buf.as_mut_ptr(), len, flags);
            if res > 0 {
                let recvd_count = res as usize;
                if let Some(Payload::Array(ref mut elements)) = args.get_mut(1) {
                    for i in 0..recvd_count {
                        if i < elements.len() {
                            elements[i] = Payload::Integer(recv_buf[i] as i64);
                        } else {
                            elements.push(Payload::Integer(recv_buf[i] as i64));
                        }
                    }
                }
            }
            return Ok(Payload::Integer(res as i64));
        } else if sym_ptr == (wasm_vfs_open as RawHandle) {
            let path_cs = args.get(0).and_then(|p| match p {
                Payload::String(s) => CString::new(s.as_str()).ok(),
                _ => None,
            });
            let path_ptr = path_cs
                .as_ref()
                .map(|cs| cs.as_ptr())
                .unwrap_or(std::ptr::null());
            let flags = args
                .get(1)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            let mode = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            return Ok(
                Payload::Integer(wasm_vfs_open(path_ptr, flags, mode) as i64),
            );
        } else if sym_ptr == (wasm_vfs_write as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            let write_bytes: Vec<u8> = if let Some(Payload::String(s)) = args.get(1)
            {
                s.as_bytes().to_vec()
            } else if let Some(Payload::Array(elements)) = args.get(1) {
                elements
                    .iter()
                    .map(|el| match el {
                        Payload::Integer(v) => *v as u8,
                        _ => 0,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let count = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as usize),
                    _ => None,
                })
                .unwrap_or(write_bytes.len());
            return Ok(Payload::Integer(wasm_vfs_write(
                fd,
                write_bytes.as_ptr(),
                count,
            ) as i64));
        } else if sym_ptr == (wasm_vfs_read as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            let count = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as usize),
                    _ => None,
                })
                .unwrap_or(512);
            let mut read_buf = vec![0u8; count];
            let res = wasm_vfs_read(fd, read_buf.as_mut_ptr(), count);
            if res > 0 {
                let recvd_count = res as usize;
                if let Some(Payload::Array(ref mut elements)) = args.get_mut(1) {
                    for i in 0..recvd_count {
                        if i < elements.len() {
                            elements[i] = Payload::Integer(read_buf[i] as i64);
                        } else {
                            elements.push(Payload::Integer(read_buf[i] as i64));
                        }
                    }
                }
            }
            return Ok(Payload::Integer(res as i64));
        } else if sym_ptr == (wasm_vfs_lseek as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            let offset = args
                .get(1)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i),
                    _ => None,
                })
                .unwrap_or(0);
            let whence = args
                .get(2)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            return Ok(Payload::Integer(wasm_vfs_lseek(fd, offset, whence)));
        } else if sym_ptr == (wasm_vfs_close as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            return Ok(Payload::Integer(wasm_vfs_close(fd) as i64));
        } else if sym_ptr == (wasm_vfs_unlink as RawHandle) {
            let path_cs = args.get(0).and_then(|p| match p {
                Payload::String(s) => CString::new(s.as_str()).ok(),
                _ => None,
            });
            let ptr = path_cs
                .as_ref()
                .map(|cs| cs.as_ptr())
                .unwrap_or(std::ptr::null());
            return Ok(Payload::Integer(wasm_vfs_unlink(ptr) as i64));
        } else if sym_ptr == (wasm_vfs_access as RawHandle) {
            let path_cs = args.get(0).and_then(|p| match p {
                Payload::String(s) => CString::new(s.as_str()).ok(),
                _ => None,
            });
            let ptr = path_cs
                .as_ref()
                .map(|cs| cs.as_ptr())
                .unwrap_or(std::ptr::null());
            let mode = args
                .get(1)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            return Ok(Payload::Integer(wasm_vfs_access(ptr, mode) as i64));
        } else if sym_ptr == (wasm_vfs_rename as RawHandle) {
            let old_cs = args.get(0).and_then(|p| match p {
                Payload::String(s) => CString::new(s.as_str()).ok(),
                _ => None,
            });
            let new_cs = args.get(1).and_then(|p| match p {
                Payload::String(s) => CString::new(s.as_str()).ok(),
                _ => None,
            });
            let old_ptr = old_cs
                .as_ref()
                .map(|cs| cs.as_ptr())
                .unwrap_or(std::ptr::null());
            let new_ptr = new_cs
                .as_ref()
                .map(|cs| cs.as_ptr())
                .unwrap_or(std::ptr::null());
            return Ok(Payload::Integer(wasm_vfs_rename(old_ptr, new_ptr) as i64));
        } else if sym_ptr == (wasm_vfs_mkdir as RawHandle) {
            let path_cs = args.get(0).and_then(|p| match p {
                Payload::String(s) => CString::new(s.as_str()).ok(),
                _ => None,
            });
            let ptr = path_cs
                .as_ref()
                .map(|cs| cs.as_ptr())
                .unwrap_or(std::ptr::null());
            let mode = args
                .get(1)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(0);
            return Ok(Payload::Integer(wasm_vfs_mkdir(ptr, mode) as i64));
        } else if sym_ptr == (wasm_close as RawHandle) {
            let fd = args
                .get(0)
                .and_then(|p| match p {
                    Payload::Integer(i) => Some(*i as i32),
                    _ => None,
                })
                .unwrap_or(-1);
            return Ok(Payload::Integer(wasm_close(fd) as i64));
        } else if sym_ptr == (wasm_inet_addr as RawHandle) {
            let ip_str = args.get(0).and_then(|p| match p {
                Payload::String(s) => CString::new(s.as_str()).ok(),
                _ => None,
            });
            let ptr = ip_str
                .as_ref()
                .map(|cs| cs.as_ptr())
                .unwrap_or(std::ptr::null());
            return Ok(Payload::Integer(wasm_inet_addr(ptr) as i64));
        }
    }

    // Struct buffers allocated on the heap: (arg_index, sorted_keys, buffer)
    let mut struct_buffers: Vec<(usize, Vec<String>, Vec<u8>)> = Vec::new();
    // Array buffers allocated on the heap: (arg_index, buffer, is_byte_array)
    let mut array_buffers: Vec<(usize, Vec<u8>, bool)> = Vec::new();

    for (arg_idx, arg) in args.iter().enumerate() {
        match arg {
            Payload::Integer(i) => {
                raw_args.push(*i as usize);
            }
            Payload::Float(bits) => {
                raw_args.push(*bits as usize);
            }
            Payload::Bool(b) => {
                raw_args.push(if *b { 1 } else { 0 });
            }
            Payload::String(s) => {
                let c_str = CString::new(s.as_str()).map_err(|_| {
                    TemporalError::EvalError("String contains null byte".to_string())
                })?;
                raw_args.push(c_str.as_ptr() as usize);
                c_strings.push(c_str);
            }
            Payload::Null => {
                raw_args.push(0);
            }
            Payload::Struct(map) => {
                // Marshall 64-bit integer fields consecutively into a C-compatible memory buffer.
                // If the struct has POSIX timespec fields, place tv_sec first, then tv_nsec.
                let mut buf = Vec::with_capacity(map.len() * 8);
                let sorted_keys: Vec<String> = if map.contains_key("tv_sec")
                    && map.contains_key("tv_nsec")
                {
                    let mut keys = vec!["tv_sec".to_string(), "tv_nsec".to_string()];
                    for k in map.keys() {
                        if k != "tv_sec" && k != "tv_nsec" {
                            keys.push(k.clone());
                        }
                    }
                    keys
                } else if map.contains_key("tv_sec") && map.contains_key("tv_usec") {
                    let mut keys = vec!["tv_sec".to_string(), "tv_usec".to_string()];
                    for k in map.keys() {
                        if k != "tv_sec" && k != "tv_usec" {
                            keys.push(k.clone());
                        }
                    }
                    keys
                } else {
                    let mut keys: Vec<String> = map.keys().cloned().collect();
                    keys.sort();
                    keys
                };
                for k in &sorted_keys {
                    let val_i64 = match map.get(k) {
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Integer(i),
                        )) => *i,
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Float(bits),
                        )) => *bits as i64,
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Bool(true),
                        )) => 1,
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Bool(false),
                        )) => 0,
                        _ => 0i64,
                    };
                    buf.extend_from_slice(&val_i64.to_ne_bytes());
                }
                let ptr = buf.as_mut_ptr() as usize;
                struct_buffers.push((arg_idx, sorted_keys, buf));
                raw_args.push(ptr);
            }
            Payload::Array(elements) => {
                // Determine if this is an array of byte/u8 integers (<256) or 64-bit integers
                let is_u8 = elements.iter().all(|e| match e {
                    Payload::Integer(i) => *i >= 0 && *i <= 255,
                    _ => false,
                });

                if is_u8 {
                    let mut buf: Vec<u8> = elements
                        .iter()
                        .map(|e| match e {
                            Payload::Integer(i) => *i as u8,
                            _ => 0u8,
                        })
                        .collect();
                    let ptr = buf.as_mut_ptr() as usize;
                    array_buffers.push((arg_idx, buf, true));
                    raw_args.push(ptr);
                } else {
                    let mut buf = Vec::with_capacity(elements.len() * 8);
                    for elem in elements {
                        let val_i64 = match elem {
                            Payload::Integer(i) => *i,
                            Payload::Float(bits) => *bits as i64,
                            Payload::Bool(b) => i64::from(*b),
                            _ => 0i64,
                        };
                        buf.extend_from_slice(&val_i64.to_ne_bytes());
                    }
                    let ptr = buf.as_mut_ptr() as usize;
                    array_buffers.push((arg_idx, buf, false));
                    raw_args.push(ptr);
                }
            }
            Payload::Topology(_) => {
                return Err(TemporalError::EvalError(
                    "Passing complex topology types directly to raw C FFI is unsupported"
                        .to_string(),
                ));
            }
            Payload::Tuple(_) => {
                return Err(TemporalError::EvalError(
                    "Passing tuple types directly to raw C FFI is unsupported"
                        .to_string(),
                ));
            }
            Payload::Range(_, _) => {
                return Err(TemporalError::EvalError(
                    "Passing range types directly to raw C FFI is unsupported"
                        .to_string(),
                ));
            }
        }
    }

    // Call function pointer based on arity
    let result_raw: usize = match raw_args.len() {
        0 => {
            // For routines returning String with 0 args (e.g. getcwd(NULL, 0)), pass (NULL, 0) or call directly
            if let Type::String = return_type {
                let func: extern "C" fn(usize, usize) -> usize =
                    unsafe { std::mem::transmute(sym_ptr) };
                func(0, 0)
            } else {
                let func: extern "C" fn() -> usize =
                    unsafe { std::mem::transmute(sym_ptr) };
                func()
            }
        }
        1 => {
            let func: extern "C" fn(usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0])
        }
        2 => {
            let func: extern "C" fn(usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0], raw_args[1])
        }
        3 => {
            let func: extern "C" fn(usize, usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0], raw_args[1], raw_args[2])
        }
        4 => {
            let func: extern "C" fn(usize, usize, usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0], raw_args[1], raw_args[2], raw_args[3])
        }
        5 => {
            let func: extern "C" fn(usize, usize, usize, usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(
                raw_args[0],
                raw_args[1],
                raw_args[2],
                raw_args[3],
                raw_args[4],
            )
        }
        6 => {
            let func: extern "C" fn(
                usize,
                usize,
                usize,
                usize,
                usize,
                usize,
            ) -> usize = unsafe { std::mem::transmute(sym_ptr) };
            func(
                raw_args[0],
                raw_args[1],
                raw_args[2],
                raw_args[3],
                raw_args[4],
                raw_args[5],
            )
        }
        _ => {
            return Err(TemporalError::EvalError(format!(
                "Foreign function calls with > 6 arguments ({}) not supported",
                raw_args.len()
            )));
        }
    };

    // Write back any modified bytes from C struct buffers into the Causm arguments
    for (arg_idx, sorted_keys, buf) in struct_buffers {
        if let Payload::Struct(ref mut map) = args[arg_idx] {
            for (i, k) in sorted_keys.into_iter().enumerate() {
                let offset = i * 8;
                if offset + 8 <= buf.len() {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&buf[offset..offset + 8]);
                    let val_i64 = i64::from_ne_bytes(bytes);
                    map.insert(
                        k,
                        causm_core::value::EntropicState::Valid(Payload::Integer(
                            val_i64,
                        )),
                    );
                }
            }
        }
    }

    // Write back any modified bytes from C array/buffer pointers into the Causm array arguments
    for (arg_idx, buf, is_u8) in array_buffers {
        if let Payload::Array(ref mut elements) = args[arg_idx] {
            if is_u8 {
                for (i, &byte) in buf.iter().enumerate() {
                    if i < elements.len() {
                        elements[i] = Payload::Integer(byte as i64);
                    } else {
                        elements.push(Payload::Integer(byte as i64));
                    }
                }
            } else {
                for (i, chunk) in buf.chunks_exact(8).enumerate() {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(chunk);
                    let val_i64 = i64::from_ne_bytes(bytes);
                    if i < elements.len() {
                        elements[i] = Payload::Integer(val_i64);
                    } else {
                        elements.push(Payload::Integer(val_i64));
                    }
                }
            }
        }
    }
    match return_type {
        Type::Integer
        | Type::I8
        | Type::I16
        | Type::I32
        | Type::I64
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::Unknown => Ok(Payload::Integer(result_raw as i64)),
        Type::Bool => Ok(Payload::Bool(result_raw != 0)),
        Type::String => {
            if result_raw == 0 {
                Ok(Payload::Null)
            } else {
                let c_str =
                    unsafe { CStr::from_ptr(result_raw as *const std::ffi::c_char) };
                let rust_str = c_str.to_string_lossy().to_string();
                Ok(Payload::String(rust_str))
            }
        }
        Type::Float | Type::F32 | Type::F64 => {
            let f = f64::from_bits(result_raw as u64);
            Ok(Payload::Float(f.to_bits()))
        }
        _ => Ok(Payload::Integer(result_raw as i64)),
    }
}
