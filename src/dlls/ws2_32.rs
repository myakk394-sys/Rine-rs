#![allow(non_snake_case)]

pub const AF_INET: i32 = 2;
pub const SOCK_STREAM: i32 = 1;
pub const IPPROTO_TCP: i32 = 6;
pub const INVALID_SOCKET: usize = !0;
pub const SOCKET_ERROR: i32 = -1;

#[repr(C)]
pub struct WsaData {
    pub version: u16,
    pub high_version: u16,
    pub max_sockets: u16,
    pub max_udp_dg: u16,
    pub vendor_info: *mut u8,
    pub description: [u8; 257],
    pub system_status: [u8; 129],
    pub _padding: [u8; 6],
}

pub unsafe extern "win64" fn WSAStartup(requested_version: u16, data: *mut WsaData) -> i32 {
    if !data.is_null() {
        (*data).version = requested_version;
        (*data).high_version = 0x0202; // Winsock 2.2
        (*data).max_sockets = 32767;
        (*data).max_udp_dg = 65535;
        (*data).vendor_info = std::ptr::null_mut();
        // Clear description and system_status strings
        (*data).description.fill(0);
        (*data).system_status.fill(0);
    }
    println!(
        "[WS2_32] WSAStartup: Requested Version=0x{:04X} -> initialized Win64 WSADATA layout",
        requested_version
    );
    0 // Success
}

pub unsafe extern "win64" fn WSACleanup() -> i32 {
    println!("[WS2_32] WSACleanup called");
    0
}

pub unsafe extern "win64" fn socket(domain: i32, socket_type: i32, protocol: i32) -> usize {
    let fd = libc::syscall(
        libc::SYS_socket,
        domain as libc::c_long,
        socket_type as libc::c_long,
        protocol as libc::c_long,
    ) as i32;
    if fd < 0 {
        println!(
            "[WS2_32] socket failed with errno {}",
            *libc::__errno_location()
        );
        INVALID_SOCKET
    } else {
        println!("[WS2_32] Created socket FD={}", fd);
        fd as usize
    }
}

pub unsafe extern "win64" fn closesocket(sock: usize) -> i32 {
    if sock != INVALID_SOCKET {
        libc::close(sock as i32);
    }
    0
}

pub unsafe extern "win64" fn WSAGetLastError() -> i32 {
    crate::dlls::kernel32::GetLastError() as i32
}

pub unsafe extern "win64" fn WSASetLastError(i_error: i32) {
    crate::dlls::kernel32::SetLastError(i_error as u32);
}

pub unsafe extern "win64" fn htons(hostshort: u16) -> u16 {
    hostshort.to_be()
}

pub unsafe extern "win64" fn ntohs(netshort: u16) -> u16 {
    u16::from_be(netshort)
}

pub unsafe extern "win64" fn htonl(hostlong: u32) -> u32 {
    hostlong.to_be()
}

pub unsafe extern "win64" fn ntohl(netlong: u32) -> u32 {
    u32::from_be(netlong)
}

pub unsafe extern "win64" fn inet_addr(cp: *const libc::c_char) -> u32 {
    if cp.is_null() {
        return 0xFFFFFFFF;
    }
    let cstr = std::ffi::CStr::from_ptr(cp);
    if let Ok(s) = cstr.to_str() {
        if let Ok(ip) = s.parse::<std::net::Ipv4Addr>() {
            return u32::from_ne_bytes(ip.octets());
        }
    }
    0xFFFFFFFF
}

pub unsafe extern "win64" fn inet_ntoa(in_addr: u32) -> *const libc::c_char {
    static mut BUFFER: [libc::c_char; 32] = [0; 32];
    let bytes = in_addr.to_ne_bytes();
    let s = format!("{}.{}.{}.{}\0", bytes[0], bytes[1], bytes[2], bytes[3]);
    let ptr = &raw mut BUFFER as *mut libc::c_char;
    std::ptr::copy_nonoverlapping(s.as_ptr() as *const libc::c_char, ptr, s.len());
    ptr
}

pub unsafe extern "win64" fn ioctlsocket(s: usize, cmd: i32, argp: *mut u32) -> i32 {
    if s == INVALID_SOCKET || argp.is_null() {
        return SOCKET_ERROR;
    }
    libc::ioctl(s as i32, cmd as libc::c_ulong, argp)
}

pub unsafe extern "win64" fn setsockopt(
    s: usize,
    level: i32,
    optname: i32,
    optval: *const libc::c_char,
    optlen: i32,
) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::setsockopt(
        s as i32,
        level,
        optname,
        optval as *const libc::c_void,
        optlen as libc::socklen_t,
    )
}

pub unsafe extern "win64" fn getsockopt(
    s: usize,
    level: i32,
    optname: i32,
    optval: *mut libc::c_char,
    optlen: *mut i32,
) -> i32 {
    if s == INVALID_SOCKET || optlen.is_null() {
        return SOCKET_ERROR;
    }
    let mut len = *optlen as libc::socklen_t;
    let ret = libc::getsockopt(
        s as i32,
        level,
        optname,
        optval as *mut libc::c_void,
        &mut len,
    );
    *optlen = len as i32;
    ret
}

pub unsafe extern "win64" fn bind(s: usize, name: *const libc::c_void, namelen: i32) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::bind(
        s as i32,
        name as *const libc::sockaddr,
        namelen as libc::socklen_t,
    )
}

pub unsafe extern "win64" fn connect(s: usize, name: *const libc::c_void, namelen: i32) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::connect(
        s as i32,
        name as *const libc::sockaddr,
        namelen as libc::socklen_t,
    )
}

pub unsafe extern "win64" fn listen(s: usize, backlog: i32) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::listen(s as i32, backlog)
}

pub unsafe extern "win64" fn accept(s: usize, addr: *mut libc::c_void, addrlen: *mut i32) -> usize {
    if s == INVALID_SOCKET {
        return INVALID_SOCKET;
    }
    let mut len = if !addrlen.is_null() {
        *addrlen as libc::socklen_t
    } else {
        0
    };
    let ret = libc::accept(
        s as i32,
        addr as *mut libc::sockaddr,
        if addrlen.is_null() {
            std::ptr::null_mut()
        } else {
            &mut len
        },
    );
    if ret < 0 {
        INVALID_SOCKET
    } else {
        if !addrlen.is_null() {
            *addrlen = len as i32;
        }
        ret as usize
    }
}

pub unsafe extern "win64" fn send(s: usize, buf: *const libc::c_char, len: i32, flags: i32) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::send(s as i32, buf as *const libc::c_void, len as usize, flags) as i32
}

pub unsafe extern "win64" fn recv(s: usize, buf: *mut libc::c_char, len: i32, flags: i32) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::recv(s as i32, buf as *mut libc::c_void, len as usize, flags) as i32
}

pub unsafe extern "win64" fn sendto(
    s: usize,
    buf: *const libc::c_char,
    len: i32,
    flags: i32,
    to: *const libc::c_void,
    tolen: i32,
) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::sendto(
        s as i32,
        buf as *const libc::c_void,
        len as usize,
        flags,
        to as *const libc::sockaddr,
        tolen as libc::socklen_t,
    ) as i32
}

pub unsafe extern "win64" fn recvfrom(
    s: usize,
    buf: *mut libc::c_char,
    len: i32,
    flags: i32,
    from: *mut libc::c_void,
    fromlen: *mut i32,
) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    let mut len_val = if !fromlen.is_null() {
        *fromlen as libc::socklen_t
    } else {
        0
    };
    let ret = libc::recvfrom(
        s as i32,
        buf as *mut libc::c_void,
        len as usize,
        flags,
        from as *mut libc::sockaddr,
        if fromlen.is_null() {
            std::ptr::null_mut()
        } else {
            &mut len_val
        },
    ) as i32;
    if !fromlen.is_null() {
        *fromlen = len_val as i32;
    }
    ret
}

pub unsafe extern "win64" fn shutdown(s: usize, how: i32) -> i32 {
    if s == INVALID_SOCKET {
        return SOCKET_ERROR;
    }
    libc::shutdown(s as i32, how)
}

pub unsafe extern "win64" fn gethostname(name: *mut libc::c_char, namelen: i32) -> i32 {
    if name.is_null() || namelen <= 0 {
        return SOCKET_ERROR;
    }
    libc::gethostname(name, namelen as usize)
}

pub unsafe extern "win64" fn __WSAFDIsSet(fd: usize, set: *mut libc::fd_set) -> i32 {
    if set.is_null() {
        return 0;
    }
    if libc::FD_ISSET(fd as i32, &*set) {
        1
    } else {
        0
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Winsock2 Event Objects & Socket Extensions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[repr(C)]
pub struct WsaBuf {
    pub len: u32,
    pub buf: *mut u8,
}

#[repr(C)]
pub struct WinAddrInfoA {
    pub ai_flags: i32,
    pub ai_family: i32,
    pub ai_socktype: i32,
    pub ai_protocol: i32,
    pub ai_addrlen: usize,
    pub ai_canonname: *mut libc::c_char,
    pub ai_addr: *mut libc::sockaddr,
    pub ai_next: *mut WinAddrInfoA,
}

#[repr(C)]
pub struct WinAddrInfoW {
    pub ai_flags: i32,
    pub ai_family: i32,
    pub ai_socktype: i32,
    pub ai_protocol: i32,
    pub ai_addrlen: usize,
    pub ai_canonname: *mut u16,
    pub ai_addr: *mut libc::sockaddr,
    pub ai_next: *mut WinAddrInfoW,
}

mod native {
    extern "C" {
        pub fn inet_ntop(
            af: libc::c_int,
            cp: *const libc::c_void,
            buf: *mut libc::c_char,
            len: libc::socklen_t,
        ) -> *const libc::c_char;
        pub fn inet_pton(
            af: libc::c_int,
            cp: *const libc::c_char,
            buf: *mut libc::c_void,
        ) -> libc::c_int;
        pub fn gethostbyname(name: *const libc::c_char) -> *mut libc::hostent;
        pub fn gethostbyaddr(
            addr: *const libc::c_void,
            len: libc::socklen_t,
            addrtype: libc::c_int,
        ) -> *mut libc::hostent;
        pub fn getprotobyname(name: *const libc::c_char) -> *mut libc::protoent;
        pub fn getprotobynumber(number: libc::c_int) -> *mut libc::protoent;
        pub fn getservbyname(name: *const libc::c_char, proto: *const libc::c_char) -> *mut libc::servent;
        pub fn getservbyport(port: libc::c_int, proto: *const libc::c_char) -> *mut libc::servent;
    }
}

unsafe fn read_wide_str_ptr(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *ptr.add(len) != 0 && len < 2048 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

pub unsafe extern "win64" fn WSACreateEvent() -> usize {
    let handle = crate::dlls::kernel32::CreateEventW(0, 1, 0, 0);
    println!("[WS2_32] WSACreateEvent -> handle=0x{:X}", handle);
    handle
}

pub unsafe extern "win64" fn WSACloseEvent(handle: usize) -> i32 {
    println!("[WS2_32] WSACloseEvent(0x{:X})", handle);
    crate::dlls::kernel32::CloseHandle(handle as u64)
}

pub unsafe extern "win64" fn WSASetEvent(handle: usize) -> i32 {
    crate::dlls::kernel32::SetEvent(handle as u64)
}

pub unsafe extern "win64" fn WSAResetEvent(handle: usize) -> i32 {
    crate::dlls::kernel32::ResetEvent(handle as u64)
}

pub unsafe extern "win64" fn WSAWaitForMultipleEvents(
    count: u32,
    lph_events: *const usize,
    wait_all: i32,
    timeout: u32,
    _alertable: i32,
) -> u32 {
    crate::dlls::kernel32::WaitForMultipleObjects(count, lph_events as *const u64, wait_all, timeout)
}

pub unsafe extern "win64" fn WSASocketA(
    af: i32,
    socket_type: i32,
    protocol: i32,
    _protocol_info: *const std::ffi::c_void,
    _g: u32,
    _flags: u32,
) -> usize {
    socket(af, socket_type, protocol)
}

pub unsafe extern "win64" fn WSASocketW(
    af: i32,
    socket_type: i32,
    protocol: i32,
    _protocol_info: *const std::ffi::c_void,
    _g: u32,
    _flags: u32,
) -> usize {
    socket(af, socket_type, protocol)
}

pub unsafe extern "win64" fn WSAEventSelect(
    s: usize,
    h_event: usize,
    l_network_events: i64,
) -> i32 {
    println!(
        "[WS2_32] WSAEventSelect(s={}, event=0x{:X}, events=0x{:X})",
        s, h_event, l_network_events
    );
    let mut non_blocking: libc::c_ulong = 1;
    if s != INVALID_SOCKET {
        libc::ioctl(s as i32, libc::FIONBIO, &mut non_blocking);
    }
    0
}

pub unsafe extern "win64" fn WSAEnumNetworkEvents(
    s: usize,
    h_event: usize,
    lp_network_events: *mut std::ffi::c_void,
) -> i32 {
    println!("[WS2_32] WSAEnumNetworkEvents(s={}, event=0x{:X})", s, h_event);
    if !lp_network_events.is_null() {
        std::ptr::write_bytes(lp_network_events as *mut u8, 0, 48);
    }
    if h_event != 0 {
        crate::dlls::kernel32::ResetEvent(h_event as u64);
    }
    0
}

pub unsafe extern "win64" fn WSAIoctl(
    s: usize,
    dw_io_control_code: u32,
    lpv_in_buffer: *const std::ffi::c_void,
    _cb_in_buffer: u32,
    _lpv_out_buffer: *mut std::ffi::c_void,
    _cb_out_buffer: u32,
    lpcb_bytes_returned: *mut u32,
    _lp_overlapped: *mut std::ffi::c_void,
    _lp_completion_routine: *mut std::ffi::c_void,
) -> i32 {
    const FIONBIO_CMD: u32 = 0x8004667E;
    const SIO_GET_EXTENSION_FUNCTION_POINTER: u32 = 0xC8000006;

    if dw_io_control_code == FIONBIO_CMD && !lpv_in_buffer.is_null() {
        let mode = *(lpv_in_buffer as *const u32);
        let mut non_blocking: libc::c_ulong = mode as libc::c_ulong;
        let ret = libc::ioctl(s as i32, libc::FIONBIO, &mut non_blocking);
        if !lpcb_bytes_returned.is_null() {
            *lpcb_bytes_returned = 0;
        }
        return ret;
    }

    if dw_io_control_code == SIO_GET_EXTENSION_FUNCTION_POINTER {
        crate::dlls::kernel32::SetLastError(10022); // WSAEINVAL
        return SOCKET_ERROR;
    }

    if !lpcb_bytes_returned.is_null() {
        *lpcb_bytes_returned = 0;
    }
    0
}

pub unsafe extern "win64" fn WSASend(
    s: usize,
    lp_buffers: *const WsaBuf,
    dw_buffer_count: u32,
    lp_number_of_bytes_sent: *mut u32,
    dw_flags: u32,
    _lp_overlapped: *mut std::ffi::c_void,
    _lp_completion_routine: *mut std::ffi::c_void,
) -> i32 {
    if s == INVALID_SOCKET || lp_buffers.is_null() || dw_buffer_count == 0 {
        crate::dlls::kernel32::SetLastError(10022);
        return SOCKET_ERROR;
    }
    let mut total_sent: usize = 0;
    for i in 0..dw_buffer_count as usize {
        let buf = &*lp_buffers.add(i);
        if buf.len > 0 && !buf.buf.is_null() {
            let res = libc::send(
                s as i32,
                buf.buf as *const libc::c_void,
                buf.len as usize,
                dw_flags as i32,
            );
            if res < 0 {
                let err = *libc::__errno_location();
                crate::dlls::kernel32::SetLastError(match err {
                    libc::EAGAIN => 10035, // WSAEWOULDBLOCK
                    libc::ECONNRESET => 10054, // WSAECONNRESET
                    _ => 10022,
                });
                return SOCKET_ERROR;
            }
            total_sent += res as usize;
        }
    }
    if !lp_number_of_bytes_sent.is_null() {
        *lp_number_of_bytes_sent = total_sent as u32;
    }
    0
}

pub unsafe extern "win64" fn WSARecv(
    s: usize,
    lp_buffers: *mut WsaBuf,
    dw_buffer_count: u32,
    lp_number_of_bytes_recvd: *mut u32,
    lp_flags: *mut u32,
    _lp_overlapped: *mut std::ffi::c_void,
    _lp_completion_routine: *mut std::ffi::c_void,
) -> i32 {
    if s == INVALID_SOCKET || lp_buffers.is_null() || dw_buffer_count == 0 {
        crate::dlls::kernel32::SetLastError(10022);
        return SOCKET_ERROR;
    }
    let flags = if !lp_flags.is_null() {
        *lp_flags as i32
    } else {
        0
    };
    let mut total_recvd: usize = 0;
    for i in 0..dw_buffer_count as usize {
        let buf = &*lp_buffers.add(i);
        if buf.len > 0 && !buf.buf.is_null() {
            let res = libc::recv(
                s as i32,
                buf.buf as *mut libc::c_void,
                buf.len as usize,
                flags,
            );
            if res < 0 {
                let err = *libc::__errno_location();
                crate::dlls::kernel32::SetLastError(match err {
                    libc::EAGAIN => 10035, // WSAEWOULDBLOCK
                    libc::ECONNRESET => 10054, // WSAECONNRESET
                    _ => 10022,
                });
                return SOCKET_ERROR;
            }
            total_recvd += res as usize;
            if (res as u32) < buf.len {
                break;
            }
        }
    }
    if !lp_number_of_bytes_recvd.is_null() {
        *lp_number_of_bytes_recvd = total_recvd as u32;
    }
    0
}

pub unsafe extern "win64" fn WSASendDisconnect(
    s: usize,
    _lp_outbound_disconnect_data: *const std::ffi::c_void,
) -> i32 {
    shutdown(s, 1) // SD_SEND = 1
}

pub unsafe extern "win64" fn select(
    nfds: i32,
    readfds: *mut libc::fd_set,
    writefds: *mut libc::fd_set,
    exceptfds: *mut libc::fd_set,
    timeout: *mut libc::timeval,
) -> i32 {
    libc::select(nfds, readfds, writefds, exceptfds, timeout)
}

pub unsafe extern "win64" fn getpeername(
    s: usize,
    name: *mut libc::c_void,
    namelen: *mut i32,
) -> i32 {
    if s == INVALID_SOCKET || namelen.is_null() {
        return SOCKET_ERROR;
    }
    let mut len = *namelen as libc::socklen_t;
    let ret = libc::getpeername(s as i32, name as *mut libc::sockaddr, &mut len);
    *namelen = len as i32;
    ret
}

pub unsafe extern "win64" fn getsockname(
    s: usize,
    name: *mut libc::c_void,
    namelen: *mut i32,
) -> i32 {
    if s == INVALID_SOCKET || namelen.is_null() {
        return SOCKET_ERROR;
    }
    let mut len = *namelen as libc::socklen_t;
    let ret = libc::getsockname(s as i32, name as *mut libc::sockaddr, &mut len);
    *namelen = len as i32;
    ret
}

pub unsafe extern "win64" fn inet_ntop(
    af: i32,
    src: *const std::ffi::c_void,
    dst: *mut libc::c_char,
    size: usize,
) -> *const libc::c_char {
    native::inet_ntop(af, src, dst, size as libc::socklen_t)
}

pub unsafe extern "win64" fn inet_pton(
    af: i32,
    src: *const libc::c_char,
    dst: *mut std::ffi::c_void,
) -> i32 {
    native::inet_pton(af, src, dst)
}

pub unsafe extern "win64" fn getaddrinfo(
    nodename: *const libc::c_char,
    servname: *const libc::c_char,
    hints: *const WinAddrInfoA,
    res: *mut *mut WinAddrInfoA,
) -> i32 {
    if res.is_null() {
        return 10022; // WSAEINVAL
    }
    *res = std::ptr::null_mut();

    let mut native_hints: libc::addrinfo = std::mem::zeroed();
    let p_native_hints = if !hints.is_null() {
        native_hints.ai_flags = (*hints).ai_flags;
        native_hints.ai_family = (*hints).ai_family;
        native_hints.ai_socktype = (*hints).ai_socktype;
        native_hints.ai_protocol = (*hints).ai_protocol;
        &native_hints as *const libc::addrinfo
    } else {
        std::ptr::null()
    };

    let mut native_res: *mut libc::addrinfo = std::ptr::null_mut();
    let ret = libc::getaddrinfo(nodename, servname, p_native_hints, &mut native_res);
    if ret != 0 {
        return match ret {
            libc::EAI_NONAME => 11001, // WSAHOST_NOT_FOUND
            libc::EAI_AGAIN => 11002,  // WSATRY_AGAIN
            libc::EAI_FAIL => 11003,   // WSANO_RECOVERY
            libc::EAI_FAMILY => 10047, // WSAEAFNOSUPPORT
            libc::EAI_SOCKTYPE => 10044, // WSAESOCKTNOSUPPORT
            _ => 10022,
        };
    }

    let mut head: *mut WinAddrInfoA = std::ptr::null_mut();
    let mut tail: *mut WinAddrInfoA = std::ptr::null_mut();

    let mut curr = native_res;
    while !curr.is_null() {
        let n = &*curr;
        let win_ai = libc::malloc(std::mem::size_of::<WinAddrInfoA>()) as *mut WinAddrInfoA;
        if win_ai.is_null() {
            break;
        }
        (*win_ai).ai_flags = n.ai_flags;
        (*win_ai).ai_family = n.ai_family;
        (*win_ai).ai_socktype = n.ai_socktype;
        (*win_ai).ai_protocol = n.ai_protocol;
        (*win_ai).ai_addrlen = n.ai_addrlen as usize;
        (*win_ai).ai_next = std::ptr::null_mut();

        if !n.ai_canonname.is_null() {
            (*win_ai).ai_canonname = libc::strdup(n.ai_canonname);
        } else {
            (*win_ai).ai_canonname = std::ptr::null_mut();
        }

        if !n.ai_addr.is_null() && n.ai_addrlen > 0 {
            let addr_copy = libc::malloc(n.ai_addrlen as usize) as *mut libc::sockaddr;
            if !addr_copy.is_null() {
                std::ptr::copy_nonoverlapping(
                    n.ai_addr as *const u8,
                    addr_copy as *mut u8,
                    n.ai_addrlen as usize,
                );
            }
            (*win_ai).ai_addr = addr_copy;
        } else {
            (*win_ai).ai_addr = std::ptr::null_mut();
        }

        if head.is_null() {
            head = win_ai;
            tail = win_ai;
        } else {
            (*tail).ai_next = win_ai;
            tail = win_ai;
        }

        curr = n.ai_next;
    }

    libc::freeaddrinfo(native_res);
    *res = head;
    0
}

pub unsafe extern "win64" fn freeaddrinfo(res: *mut WinAddrInfoA) {
    let mut curr = res;
    while !curr.is_null() {
        let next = (*curr).ai_next;
        if !(*curr).ai_canonname.is_null() {
            libc::free((*curr).ai_canonname as *mut libc::c_void);
        }
        if !(*curr).ai_addr.is_null() {
            libc::free((*curr).ai_addr as *mut libc::c_void);
        }
        libc::free(curr as *mut libc::c_void);
        curr = next;
    }
}

pub unsafe extern "win64" fn GetAddrInfoW(
    nodename: *const u16,
    servname: *const u16,
    hints: *const WinAddrInfoW,
    res: *mut *mut WinAddrInfoW,
) -> i32 {
    if res.is_null() {
        return 10022;
    }
    *res = std::ptr::null_mut();

    let node_c = if !nodename.is_null() {
        let s = read_wide_str_ptr(nodename);
        Some(std::ffi::CString::new(s).unwrap_or_default())
    } else {
        None
    };

    let serv_c = if !servname.is_null() {
        let s = read_wide_str_ptr(servname);
        Some(std::ffi::CString::new(s).unwrap_or_default())
    } else {
        None
    };

    let mut hints_a: WinAddrInfoA = std::mem::zeroed();
    let p_hints_a = if !hints.is_null() {
        hints_a.ai_flags = (*hints).ai_flags;
        hints_a.ai_family = (*hints).ai_family;
        hints_a.ai_socktype = (*hints).ai_socktype;
        hints_a.ai_protocol = (*hints).ai_protocol;
        &hints_a as *const WinAddrInfoA
    } else {
        std::ptr::null()
    };

    let mut res_a: *mut WinAddrInfoA = std::ptr::null_mut();
    let node_ptr = node_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
    let serv_ptr = serv_c.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());

    let ret = getaddrinfo(node_ptr, serv_ptr, p_hints_a, &mut res_a);
    if ret != 0 {
        return ret;
    }

    let mut head: *mut WinAddrInfoW = std::ptr::null_mut();
    let mut tail: *mut WinAddrInfoW = std::ptr::null_mut();

    let mut curr = res_a;
    while !curr.is_null() {
        let a = &*curr;
        let w_ai = libc::malloc(std::mem::size_of::<WinAddrInfoW>()) as *mut WinAddrInfoW;
        if w_ai.is_null() {
            break;
        }
        (*w_ai).ai_flags = a.ai_flags;
        (*w_ai).ai_family = a.ai_family;
        (*w_ai).ai_socktype = a.ai_socktype;
        (*w_ai).ai_protocol = a.ai_protocol;
        (*w_ai).ai_addrlen = a.ai_addrlen;
        (*w_ai).ai_next = std::ptr::null_mut();
        (*w_ai).ai_canonname = std::ptr::null_mut();
        (*w_ai).ai_addr = a.ai_addr;
        // Take ownership of ai_addr so freeaddrinfo won't free it
        (*curr).ai_addr = std::ptr::null_mut();

        if head.is_null() {
            head = w_ai;
            tail = w_ai;
        } else {
            (*tail).ai_next = w_ai;
            tail = w_ai;
        }

        curr = a.ai_next;
    }

    freeaddrinfo(res_a);
    *res = head;
    0
}

pub unsafe extern "win64" fn FreeAddrInfoW(res: *mut WinAddrInfoW) {
    let mut curr = res;
    while !curr.is_null() {
        let next = (*curr).ai_next;
        if !(*curr).ai_canonname.is_null() {
            libc::free((*curr).ai_canonname as *mut libc::c_void);
        }
        if !(*curr).ai_addr.is_null() {
            libc::free((*curr).ai_addr as *mut libc::c_void);
        }
        libc::free(curr as *mut libc::c_void);
        curr = next;
    }
}

pub unsafe extern "win64" fn GetAddrInfoExW(
    p_name: *const u16,
    p_service_name: *const u16,
    _dw_name_space: u32,
    _lp_nsprovider_id: *const std::ffi::c_void,
    hints: *const WinAddrInfoW,
    pp_result: *mut *mut WinAddrInfoW,
    _timeout: *const std::ffi::c_void,
    _lp_overlapped: *mut std::ffi::c_void,
    _lp_completion_routine: *mut std::ffi::c_void,
    _lp_name_handle: *mut usize,
) -> i32 {
    GetAddrInfoW(p_name, p_service_name, hints, pp_result)
}

pub unsafe extern "win64" fn FreeAddrInfoExW(res: *mut WinAddrInfoW) {
    FreeAddrInfoW(res);
}

pub unsafe extern "win64" fn GetAddrInfoExCancel(_lp_handle: usize) -> i32 {
    0
}

pub unsafe extern "win64" fn getnameinfo(
    sa: *const libc::sockaddr,
    salen: i32,
    host: *mut libc::c_char,
    hostlen: u32,
    serv: *mut libc::c_char,
    servlen: u32,
    flags: i32,
) -> i32 {
    libc::getnameinfo(
        sa,
        salen as libc::socklen_t,
        host,
        hostlen as libc::socklen_t,
        serv,
        servlen as libc::socklen_t,
        flags,
    )
}

pub unsafe extern "win64" fn GetNameInfoW(
    sa: *const libc::sockaddr,
    salen: i32,
    host: *mut u16,
    hostlen: u32,
    serv: *mut u16,
    servlen: u32,
    flags: i32,
) -> i32 {
    let mut host_a = vec![0u8; hostlen.max(64) as usize];
    let mut serv_a = vec![0u8; servlen.max(64) as usize];
    let ret = libc::getnameinfo(
        sa,
        salen as libc::socklen_t,
        if host.is_null() {
            std::ptr::null_mut()
        } else {
            host_a.as_mut_ptr() as *mut libc::c_char
        },
        hostlen as libc::socklen_t,
        if serv.is_null() {
            std::ptr::null_mut()
        } else {
            serv_a.as_mut_ptr() as *mut libc::c_char
        },
        servlen as libc::socklen_t,
        flags,
    );
    if ret != 0 {
        return ret;
    }
    if !host.is_null() && hostlen > 0 {
        let s = std::ffi::CStr::from_ptr(host_a.as_ptr() as *const libc::c_char).to_string_lossy();
        let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let copy_len = wide.len().min(hostlen as usize);
        std::ptr::copy_nonoverlapping(wide.as_ptr(), host, copy_len);
    }
    if !serv.is_null() && servlen > 0 {
        let s = std::ffi::CStr::from_ptr(serv_a.as_ptr() as *const libc::c_char).to_string_lossy();
        let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let copy_len = wide.len().min(servlen as usize);
        std::ptr::copy_nonoverlapping(wide.as_ptr(), serv, copy_len);
    }
    0
}

pub unsafe extern "win64" fn InetNtopW(
    family: i32,
    p_addr: *const std::ffi::c_void,
    p_string_buf: *mut u16,
    string_buf_size: usize,
) -> *const u16 {
    if p_addr.is_null() || p_string_buf.is_null() || string_buf_size == 0 {
        return std::ptr::null();
    }
    let mut buf = [0u8; 64];
    let res = native::inet_ntop(
        family,
        p_addr,
        buf.as_mut_ptr() as *mut libc::c_char,
        buf.len() as libc::socklen_t,
    );
    if res.is_null() {
        return std::ptr::null();
    }
    let s = std::ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char).to_string_lossy();
    let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    let copy_len = wide.len().min(string_buf_size);
    std::ptr::copy_nonoverlapping(wide.as_ptr(), p_string_buf, copy_len);
    p_string_buf
}

pub unsafe extern "win64" fn InetPtonW(
    family: i32,
    p_string_buf: *const u16,
    p_addr_buf: *mut std::ffi::c_void,
) -> i32 {
    if p_string_buf.is_null() || p_addr_buf.is_null() {
        return -1;
    }
    let s = read_wide_str_ptr(p_string_buf);
    let c_str = match std::ffi::CString::new(s) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    native::inet_pton(family, c_str.as_ptr(), p_addr_buf)
}

pub unsafe extern "win64" fn gethostbyname(name: *const libc::c_char) -> *mut libc::hostent {
    if name.is_null() {
        crate::dlls::kernel32::SetLastError(10022);
        return std::ptr::null_mut();
    }
    native::gethostbyname(name)
}

pub unsafe extern "win64" fn gethostbyaddr(
    addr: *const libc::c_void,
    len: i32,
    addr_type: i32,
) -> *mut libc::hostent {
    native::gethostbyaddr(addr, len as libc::socklen_t, addr_type)
}

pub unsafe extern "win64" fn getprotobyname(name: *const libc::c_char) -> *mut libc::protoent {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    native::getprotobyname(name)
}

pub unsafe extern "win64" fn getprotobynumber(number: i32) -> *mut libc::protoent {
    native::getprotobynumber(number)
}

pub unsafe extern "win64" fn getservbyname(
    name: *const libc::c_char,
    proto: *const libc::c_char,
) -> *mut libc::servent {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    native::getservbyname(name, proto)
}

pub unsafe extern "win64" fn getservbyport(
    port: i32,
    proto: *const libc::c_char,
) -> *mut libc::servent {
    native::getservbyport(port, proto)
}

#[no_mangle]
pub unsafe extern "win64" fn GetHostNameW(name: *mut u16, namelen: i32) -> i32 {
    if name.is_null() || namelen <= 0 {
        return -1;
    }
    let mut buf = [0u8; 256];
    let res = libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len());
    let host = if res == 0 {
        std::ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char)
            .to_string_lossy()
            .into_owned()
    } else {
        "rine-win64".to_string()
    };
    let utf16: Vec<u16> = host.encode_utf16().chain(std::iter::once(0)).collect();
    if utf16.len() > namelen as usize {
        return -1;
    }
    std::ptr::copy_nonoverlapping(utf16.as_ptr(), name, utf16.len());
    0
}

#[no_mangle]
pub unsafe extern "win64" fn WSALookupServiceBeginW(
    _lpqs_restrictions: *const u8,
    _dw_control_flags: u32,
    lph_lookup: *mut usize,
) -> i32 {
    if !lph_lookup.is_null() {
        *lph_lookup = 0x5001;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn WSALookupServiceBeginA(
    _lpqs_restrictions: *const u8,
    _dw_control_flags: u32,
    lph_lookup: *mut usize,
) -> i32 {
    if !lph_lookup.is_null() {
        *lph_lookup = 0x5001;
    }
    0
}

#[no_mangle]
pub unsafe extern "win64" fn WSALookupServiceNextW(
    _h_lookup: usize,
    _dw_control_flags: u32,
    _lpdw_buffer_length: *mut u32,
    _lpqs_results: *mut u8,
) -> i32 {
    WSASetLastError(10110); // WSA_E_NO_MORE
    -1
}

#[no_mangle]
pub unsafe extern "win64" fn WSALookupServiceNextA(
    _h_lookup: usize,
    _dw_control_flags: u32,
    _lpdw_buffer_length: *mut u32,
    _lpqs_results: *mut u8,
) -> i32 {
    WSASetLastError(10110); // WSA_E_NO_MORE
    -1
}

#[no_mangle]
pub unsafe extern "win64" fn WSALookupServiceEnd(_h_lookup: usize) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "win64" fn WSANSPIoctl(
    _h_lookup: usize,
    _dw_control_code: u32,
    _lpv_in_buffer: *const u8,
    _cb_in_buffer: u32,
    _lpv_out_buffer: *mut u8,
    _cb_out_buffer: u32,
    _lpcb_bytes_returned: *mut u32,
    _lp_completion: *mut u8,
) -> i32 {
    WSASetLastError(10045); // WSAEOPNOTSUPP
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_winsock2_initialization_and_layout() {
        assert_eq!(
            std::mem::size_of::<WsaData>(),
            408,
            "Win64 WSADATA must be exactly 408 bytes"
        );

        let mut data = WsaData {
            version: 0,
            high_version: 0,
            max_sockets: 0,
            max_udp_dg: 0,
            vendor_info: std::ptr::null_mut(),
            description: [0; 257],
            system_status: [0; 129],
            _padding: [0; 6],
        };

        let data_ptr = &mut data as *mut WsaData;

        // Verify Win64 struct field offsets
        unsafe {
            let base = data_ptr as usize;
            assert_eq!((&raw mut (*data_ptr).version) as usize - base, 0x00);
            assert_eq!((&raw mut (*data_ptr).high_version) as usize - base, 0x02);
            assert_eq!((&raw mut (*data_ptr).max_sockets) as usize - base, 0x04);
            assert_eq!((&raw mut (*data_ptr).max_udp_dg) as usize - base, 0x06);
            assert_eq!((&raw mut (*data_ptr).vendor_info) as usize - base, 0x08);
            assert_eq!((&raw mut (*data_ptr).description) as usize - base, 0x10);
            assert_eq!((&raw mut (*data_ptr).system_status) as usize - base, 0x111);

            assert_eq!(WSAStartup(0x0202, data_ptr), 0);
            assert_eq!(data.version, 0x0202);
            assert_eq!(WSACleanup(), 0);
        }
    }

    #[test]
    fn test_winsock2_extended_structures_and_events() {
        assert_eq!(
            std::mem::size_of::<WsaBuf>(),
            16,
            "Win64 WSABUF must be 16 bytes (4 bytes len + 4 bytes padding + 8 bytes ptr)"
        );
        assert_eq!(
            std::mem::size_of::<WinAddrInfoA>(),
            48,
            "Win64 ADDRINFOA must be 48 bytes"
        );
        assert_eq!(
            std::mem::size_of::<WinAddrInfoW>(),
            48,
            "Win64 ADDRINFOW must be 48 bytes"
        );

        unsafe {
            let ev = WSACreateEvent();
            assert_ne!(ev, 0, "WSACreateEvent must return non-null handle");
            assert_eq!(WSASetEvent(ev), 1);
            assert_eq!(WSAResetEvent(ev), 1);
            assert_eq!(WSACloseEvent(ev), 1);
        }
    }
}
