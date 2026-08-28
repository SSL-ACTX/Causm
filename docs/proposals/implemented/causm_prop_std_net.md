# Proposal: Self-Hosted `std/net` Standard Library Module

## 1. Motivation
Causm previously provided standard file system (`std/fs`), environment (`std/env`), path (`std/path`), and temporal telemetry (`std/time`) self-hosted modules.

To enable native, deterministic socket I/O with entropic leases and automatic socket descriptor drops, this proposal specifies the self-hosted `std/net` module.

## 2. Module Hierarchy & Structure
Located at `crates/causm-stdlib/csm/std/net/`:
- **`types.csm`**: `SocketAddr`, `TcpStream` (auto_drop), `TcpListener` (auto_drop), `UdpSocket` (auto_drop), `Timeval`.
- **`ffi.csm`**: `socket`, `close`, `shutdown`, `bind`, `connect`, `listen`, `accept`, `send`, `recv`, `sendto`, `recvfrom`, `setsockopt`, `inet_addr`, `htons`, `fcntl`.
- **`ops.csm`**: Full high-level API (see §3 below).
- **`mod.csm`**: Re-exports all under `std/net`.

## 3. Full API Surface

### TCP
| Routine | Description |
|---|---|
| `tcp_listener(port)` | Bind + listen; returns `TcpListener` |
| `tcp_stream_connect(port, b0..b3)` | Connect via raw octets; returns `TcpStream` |
| `tcp_stream_connect_ip(ip, port)` | Connect via IP string `"127.0.0.1"`; returns `TcpStream` |
| `tcp_accept(fd)` | Accept incoming; returns client fd |
| `tcp_send(fd, buf, len)` | Send buffer; returns bytes sent |
| `tcp_recv(fd, buf, len)` | Receive into buffer; returns bytes received |
| `tcp_send_all(fd, buf, len)` | Full-buffer send wrapper |
| `tcp_recv_exact(fd, buf, len)` | Exact-length receive wrapper |
| `tcp_bind(fd, port)` | Raw bind to port |
| `tcp_listen(fd, backlog)` | Raw listen |
| `tcp_connect(fd, port, b0..b3)` | Raw connect |

### UDP
| Routine | Description |
|---|---|
| `udp_bind(port)` | Bind + reuseaddr; returns `UdpSocket` |
| `udp_send_to(fd, buf, len, port, b0..b3)` | Send datagram to address |
| `udp_recv_from(fd, buf, len)` | Receive datagram |

### Socket Control
| Routine | Description |
|---|---|
| `create_socket()` | Raw AF_INET/SOCK_STREAM fd |
| `create_udp_socket()` | Raw AF_INET/SOCK_DGRAM fd |
| `close_socket(fd)` | Close fd |
| `set_reuseaddr(fd)` | SO_REUSEADDR |
| `set_nonblocking(fd)` | O_NONBLOCK via fcntl |
| `set_blocking(fd)` | Remove O_NONBLOCK |
| `set_recv_timeout(fd, ms)` | SO_RCVTIMEO via struct timeval |
| `set_send_timeout(fd, ms)` | SO_SNDTIMEO via struct timeval |
| `shutdown_stream(stream, how)` | POSIX shutdown |
| `make_sockaddr(port, b0..b3)` | Build 16-byte sockaddr_in |
| `addr(ip, port)` | Build `SocketAddr` |

## 4. Example Usage

```csm
import "std/net" as Net

@10ms: {
    let listener = call Net.tcp_listener(19876)
    let listen_fd = listener.fd
}

@20ms: {
    let stream = call Net.tcp_stream_connect_ip("127.0.0.1", 19876)
    let stream_fd = stream.fd
    let _nb = call Net.set_nonblocking(stream_fd)
    let _rt = call Net.set_recv_timeout(stream_fd, 500)
    let client_fd = call Net.tcp_accept(listen_fd)
}

@30ms: {
    let payload = [72, 69, 76, 76, 79]
    let sent = call Net.tcp_send_all(stream_fd, payload, 5)
    let mut buf = [0, 0, 0, 0, 0]
    let recvd = call Net.tcp_recv_exact(client_fd, buf, 5)
}

@40ms: {
    let server = call Net.udp_bind(19899)
    let udp_client = call Net.create_udp_socket()
    let udp_sent = call Net.udp_send_to(udp_client, [85, 68, 80], 3, 19899, 127, 0, 0, 1)
    let mut udp_buf = [0, 0, 0]
    let udp_recvd = call Net.udp_recv_from(server.fd, udp_buf, 3)
}
```
