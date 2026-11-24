#![allow(non_snake_case)]

pub const DXGI_ERROR_UNSUPPORTED: i32 = 0x887A0004_u32 as i32;
pub const E_NOTIMPL: i32 = 0x80004001_u32 as i32;

#[no_mangle]
pub unsafe extern "win64" fn D3D12CreateDevice(
    _pAdapter: *mut std::ffi::c_void,
    _MinimumFeatureLevel: u32,
    _riid: *const std::ffi::c_void,
    _ppDevice: *mut *mut std::ffi::c_void,
) -> i32 {
    println!("[D3D12] D3D12CreateDevice called -> returning DXGI_ERROR_UNSUPPORTED (0x887A0004)");
    DXGI_ERROR_UNSUPPORTED
}

#[no_mangle]
pub unsafe extern "win64" fn D3D12GetDebugInterface(
    _riid: *const std::ffi::c_void,
    _ppvDebug: *mut *mut std::ffi::c_void,
) -> i32 {
    println!("[D3D12] D3D12GetDebugInterface called -> returning E_NOTIMPL");
    E_NOTIMPL
}

#[no_mangle]
pub unsafe extern "win64" fn D3D12SerializeRootSignature(
    _pRootSignature: *const std::ffi::c_void,
    _Version: u32,
    _ppBlob: *mut *mut std::ffi::c_void,
    _ppErrorBlob: *mut *mut std::ffi::c_void,
) -> i32 {
    println!("[D3D12] D3D12SerializeRootSignature called -> returning E_NOTIMPL");
    E_NOTIMPL
}
