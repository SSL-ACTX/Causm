# Causm Pure Standard Library (`causm-stdlib`)

`causm-stdlib` provides the embedded, first-class standard library written natively in pure Causm (`.csm`) for systems programming, file I/O, process management, and memory safety.

---

## 1. Modules

### 1.1 `std/fs` (File System Module)
Located at `crates/causm-stdlib/csm/std/fs/`:
- **`types.csm`**: Core structures and enums (`File`, `FileMetadata`, `OpenMode`, `SeekFrom`, `FileResult`). The `File` type leverages Causm's `auto_drop("libc.so.6", "close", fd)` to guarantee file descriptor cleanup on scope exit or decay.
- **`ffi.csm`**: POSIX C bindings to `libc.so.6` (`open`, `creat`, `read`, `write`, `close`, `unlink`, `rename`, `access`, `lseek`, `fsync`, `ftruncate`, `mkdir`, `rmdir`).
- **`ops.csm`**: High-level file system routines:
  - `open_readonly(path)`
  - `create_file(path)`
  - `open_append(path)`
  - `write_all(file, data, len)`
  - `flush_file(file)`
  - `truncate_file(file, len)`
  - `seek_start(file, offset)`
  - `seek_end(file, offset)`
  - `file_exists(path)`
  - `remove_file(path)`
  - `rename_file(oldpath, newpath)`
  - `create_dir(path, mode)`
  - `read_to_string(path)`
  - `file_size(path)`
  - `remove_dir(path)`
- **`mod.csm`**: Aggregates and re-exports all file system types and routines.

### 1.2 `std/path` (Path Utilities Module)
Located at `crates/causm-stdlib/csm/std/path.csm`:
- `join(dir, file)`: Combines directory and file paths.
- `path_basename(path)`: Extracts filename from path via standard POSIX `basename`.
- `path_dirname(path)`: Extracts directory component from path via standard POSIX `dirname`.

### 1.3 `std/env` (Environment Module)
Located at `crates/causm-stdlib/csm/std/env.csm`:
- `get_pid()`
- `get_ppid()`
- `get_uid()`
- `get_cwd()`

---

## 2. Usage in Causm Programs

Standard library modules are embedded directly into the compiler binary and can be imported into any isolate without requiring relative path configuration:

```causm
@0ms: {
    isolate fs_worker {
        enable cpu(2000ms)
        enable memory(64KB)
        require System.FFI

        from "std/fs" import *

        let file = create_file("/tmp/log.txt")
        let written = write_all(file, "Hello, Causm!", 13)
        flush_file(file)
    }
}
```

Or with namespace aliasing:
```causm
@0ms: {
    isolate fs_worker {
        import "std/fs" as fs

        let file = fs.create_file("/tmp/log.txt")
    }
}
```

---

## 3. Extended Standard Library Modules

- **`std/json`**: Native high-performance JSON decoder, encoder, object manipulation, and ADT enum representations.
- **`std/time`**: High-precision monotonic timestamps (`Instant`, `Duration`), POSIX epoch clocks, and precision sleeping.
- **`std/net`**: Socket networking, `SocketAddr`, `TcpStream`, and `TcpListener`.
- **`std/http`**: HTTP/1.1 request formatting, client networking, and response parsing.
- **`std/encoding`**: Base64 encoding/decoding, binary serialization, and bitwise stream operations.
- **`std/collection`**: Array utilities, `RingBuffer`, `Stack`, `Queue`, and `BitSet`.
- **`std/process`**: Process spawning, execution, pipe control, and status checking.
