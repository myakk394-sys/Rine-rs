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
}
