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
    pub description: [u8; 257],
    pub system_status: [u8; 129],
    pub max_sockets: u16,
    pub max_udp_dg: u16,
    pub vendor_info: u64,
}

pub fn WSAStartup(requested_version: u16, data: &mut WsaData) -> i32 {
    data.version = requested_version;
    data.high_version = 0x0202; // Winsock 2.2
    println!(
        "[WS2_32] WSAStartup: Requested Version=0x{:04X} -> initialized",
        requested_version
    );
    0 // Success
}

pub fn WSACleanup() -> i32 {
    println!("[WS2_32] WSACleanup called");
    0
}

pub fn socket(domain: i32, socket_type: i32, protocol: i32) -> usize {
    let fd = unsafe { libc::socket(domain, socket_type, protocol) };
    if fd < 0 {
        println!("[WS2_32] socket failed with errno {}", unsafe {
            *libc::__errno_location()
        });
        INVALID_SOCKET
    } else {
        println!("[WS2_32] Created socket FD={}", fd);
        fd as usize
    }
}

pub fn closesocket(sock: usize) -> i32 {
    if sock != INVALID_SOCKET {
        unsafe { libc::close(sock as i32) };
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_winsock2_initialization() {
        let mut data = WsaData {
            version: 0,
            high_version: 0,
            description: [0; 257],
            system_status: [0; 129],
            max_sockets: 0,
            max_udp_dg: 0,
            vendor_info: 0,
        };

        assert_eq!(WSAStartup(0x0202, &mut data), 0);
        assert_eq!(data.version, 0x0202);
        assert_eq!(WSACleanup(), 0);
    }
}
