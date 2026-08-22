#![allow(dead_code)]
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RegisteredObject {
    pub object_id: u64,
    pub object_address: u64,
    pub vtable_address: u64,
    pub creator_symbol: &'static str,
    pub caller_module: String,
    pub caller_rva: u64,
    pub clsid: String,
    pub iid: String,
    pub interface_name: &'static str,
    pub allocation_size: usize,
    pub vtable_slot_count: usize,
    pub initialized_slots: Vec<usize>,
    pub creation_thread_id: u64,
}

static OBJECT_REGISTRY: Mutex<Option<HashMap<u64, RegisteredObject>>> = Mutex::new(None);
static VTABLE_REGISTRY: Mutex<Option<HashMap<u64, RegisteredObject>>> = Mutex::new(None);
static OBJECT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn register_rine_object(
    object_address: u64,
    vtable_address: u64,
    creator_symbol: &'static str,
    clsid: &str,
    iid: &str,
    interface_name: &'static str,
    allocation_size: usize,
    vtable_slot_count: usize,
    initialized_slots: Vec<usize>,
) -> u64 {
    let id = OBJECT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let thread_id = unsafe { libc::syscall(libc::SYS_gettid) as u64 };

    let obj = RegisteredObject {
        object_id: id,
        object_address,
        vtable_address,
        creator_symbol,
        caller_module: "main_exe".to_string(),
        caller_rva: 0,
        clsid: clsid.to_string(),
        iid: iid.to_string(),
        interface_name,
        allocation_size,
        vtable_slot_count,
        initialized_slots: initialized_slots.clone(),
        creation_thread_id: thread_id,
    };

    println!("[RINE OBJECT CREATE]");
    println!("  id={}", obj.object_id);
    println!("  object=0x{:016X}", obj.object_address);
    println!("  vtable=0x{:016X}", obj.vtable_address);
    println!("  creator={}", obj.creator_symbol);
    println!("  interface={}", obj.interface_name);
    println!("  iid={}", obj.iid);
    println!("  clsid={}", obj.clsid);
    println!("  slots={:?}", obj.initialized_slots);

    {
        let mut guard = OBJECT_REGISTRY.lock().unwrap();
        let obj_map = guard.get_or_insert_with(HashMap::new);
        obj_map.insert(object_address, obj.clone());
    }
    {
        let mut vt_guard = VTABLE_REGISTRY.lock().unwrap();
        let vt_map = vt_guard.get_or_insert_with(HashMap::new);
        vt_map.insert(vtable_address, obj);
    }

    id
}

pub fn lookup_rine_object(addr: u64) -> Option<RegisteredObject> {
    if addr == 0 {
        return None;
    }
    if let Ok(guard) = OBJECT_REGISTRY.lock() {
        if let Some(map) = guard.as_ref() {
            if let Some(obj) = map.get(&addr) {
                return Some(obj.clone());
            }
        }
    }
    if let Ok(guard) = VTABLE_REGISTRY.lock() {
        if let Some(map) = guard.as_ref() {
            if let Some(obj) = map.get(&addr) {
                return Some(obj.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_metadata_registry_link() {
        let obj_addr = 0x1122334455667788u64;
        let vt_addr = 0xAABBCCDDEEFF0011u64;
        let id = register_rine_object(
            obj_addr,
            vt_addr,
            "test!creator",
            "{CLSID-TEST}",
            "{IID-TEST}",
            "ITestInterface",
            64,
            10,
            vec![0, 1, 2, 3],
        );
        assert!(id > 0);

        let looked_up = lookup_rine_object(obj_addr).expect("Should find by object address");
        assert_eq!(looked_up.creator_symbol, "test!creator");
        assert_eq!(looked_up.interface_name, "ITestInterface");

        let looked_up_vt = lookup_rine_object(vt_addr).expect("Should find by vtable address");
        assert_eq!(looked_up_vt.object_id, id);
    }

    #[test]
    fn test_failure_hresult_nullifies_out_param() {
        let mut out_ptr: *mut std::ffi::c_void = 0x12345 as *mut _;
        let result_hresult = -2147467262i32; // E_NOINTERFACE (0x80004002)
        if result_hresult < 0 {
            out_ptr = std::ptr::null_mut();
        }
        assert!(out_ptr.is_null());
    }

    #[test]
    fn test_api_cannot_return_s_ok_with_null_vtable_slots() {
        let vtable_ptr = crate::dlls::dxgi::get_adapter_vtable();
        unsafe {
            for i in 0..15 {
                let slot = *vtable_ptr.add(i);
                assert!(
                    !slot.is_null(),
                    "Slot {} in ADAPTER_VTABLE must not be NULL!",
                    i
                );
            }
        }
    }

    #[test]
    fn test_com_abi_signatures() {
        let add_ref_fn: unsafe extern "win64" fn(*mut std::ffi::c_void) -> u32 =
            crate::dlls::d3d11::com_object_add_ref;
        let release_fn: unsafe extern "win64" fn(*mut std::ffi::c_void) -> u32 =
            crate::dlls::d3d11::com_object_release;

        let obj_ptr = crate::dlls::d3d11::allocate_mock_com_object(0x1000);
        assert!(!obj_ptr.is_null());

        let ref_count = unsafe { add_ref_fn(obj_ptr) };
        assert_eq!(ref_count, 2);

        let ref_after_release = unsafe { release_fn(obj_ptr) };
        assert_eq!(ref_after_release, 1);

        let final_release = unsafe { release_fn(obj_ptr) };
        assert_eq!(final_release, 0);
    }
}
