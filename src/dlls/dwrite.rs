#![allow(non_snake_case)]

const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

const IID_IUNKNOWN: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];
const IID_IDWRITE_FACTORY: [u8; 16] = [
    0x5a, 0xee, 0x59, 0xb8, 0x38, 0xd8, 0x5b, 0x4b, 0xa2, 0xe8, 0x1a, 0xdc, 0x7d, 0x93, 0xdb, 0x48,
];
const IID_IDWRITE_FONT_COLLECTION: [u8; 16] = [
    0xa7, 0x4f, 0x4a, 0xa5, 0xd7, 0x46, 0xe6, 0x48, 0x92, 0xf8, 0x3a, 0x16, 0xf4, 0xf5, 0x47, 0x28,
];

unsafe extern "win64" fn dwrite_factory_query_interface(
    this: *mut std::ffi::c_void,
    riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = std::ptr::null_mut();
    if riid.is_null() || this.is_null() {
        return E_NOINTERFACE;
    }
    let iid = std::slice::from_raw_parts(riid, 16);
    if iid == IID_IUNKNOWN || iid == IID_IDWRITE_FACTORY {
        *ppv = this;
        crate::dlls::d3d11::com_object_add_ref(this);
        0 // S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "win64" fn dwrite_font_collection_query_interface(
    this: *mut std::ffi::c_void,
    riid: *const u8,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = std::ptr::null_mut();
    if riid.is_null() || this.is_null() {
        return E_NOINTERFACE;
    }
    let iid = std::slice::from_raw_parts(riid, 16);
    if iid == IID_IUNKNOWN || iid == IID_IDWRITE_FONT_COLLECTION {
        *ppv = this;
        crate::dlls::d3d11::com_object_add_ref(this);
        0 // S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "win64" fn dwrite_method_not_implemented(
    _this: *mut std::ffi::c_void,
    _p1: usize,
    _p2: usize,
    _p3: usize,
    _p4: usize,
) -> i32 {
    0 // Return S_OK to allow font subsystem progress
}

unsafe extern "win64" fn dwrite_font_collection_get_font_family_count(
    _this: *mut std::ffi::c_void,
) -> u32 {
    0 // Return 0 system font families so Godot falls back to embedded FreeType fonts
}

unsafe extern "win64" fn dwrite_font_collection_find_family_name(
    _this: *mut std::ffi::c_void,
    _name: *const u16,
    index_out: *mut u32,
    exists_out: *mut i32,
) -> i32 {
    if !index_out.is_null() {
        *index_out = 0xFFFF_FFFF;
    }
    if !exists_out.is_null() {
        *exists_out = 0; // FALSE
    }
    0 // S_OK
}

static mut DWRITE_FONT_COLLECTION_VTABLE: [*const (); 15] = [std::ptr::null(); 15];

fn get_dwrite_font_collection_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        DWRITE_FONT_COLLECTION_VTABLE[0] = dwrite_font_collection_query_interface as *const ();
        DWRITE_FONT_COLLECTION_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        DWRITE_FONT_COLLECTION_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        DWRITE_FONT_COLLECTION_VTABLE[3] =
            dwrite_font_collection_get_font_family_count as *const ();
        for i in 4..15 {
            DWRITE_FONT_COLLECTION_VTABLE[i] = dwrite_method_not_implemented as *const ();
        }
        DWRITE_FONT_COLLECTION_VTABLE[5] = dwrite_font_collection_find_family_name as *const ();
    });
    unsafe { DWRITE_FONT_COLLECTION_VTABLE.as_ptr() }
}

unsafe extern "win64" fn dwrite_factory_get_system_font_collection(
    _this: *mut std::ffi::c_void,
    font_collection_out: *mut *mut std::ffi::c_void,
    _check_for_updates: i32,
) -> i32 {
    if !font_collection_out.is_null() {
        let vtable_ptr = get_dwrite_font_collection_vtable() as usize;
        *font_collection_out = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            vtable_ptr,
            "dwrite!GetSystemFontCollection",
            "IDWriteFontCollection",
            "{A54A4FA7-46D7-48E6-92F8-3A16F4F54728}",
            "{A54A4FA7-46D7-48E6-92F8-3A16F4F54728}",
            15,
            vec![0, 1, 2, 3, 5],
        );
    }
    0 // S_OK
}

static mut DWRITE_FACTORY_VTABLE: [*const (); 25] = [std::ptr::null(); 25];

fn get_dwrite_factory_vtable() -> *const *const () {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| unsafe {
        DWRITE_FACTORY_VTABLE[0] = dwrite_factory_query_interface as *const ();
        DWRITE_FACTORY_VTABLE[1] = crate::dlls::d3d11::com_object_add_ref as *const ();
        DWRITE_FACTORY_VTABLE[2] = crate::dlls::d3d11::com_object_release as *const ();
        DWRITE_FACTORY_VTABLE[3] = dwrite_factory_get_system_font_collection as *const ();
        for i in 4..25 {
            DWRITE_FACTORY_VTABLE[i] = dwrite_method_not_implemented as *const ();
        }
    });
    unsafe { DWRITE_FACTORY_VTABLE.as_ptr() }
}

#[no_mangle]
pub unsafe extern "win64" fn DWriteCreateFactory(
    _factory_type: u32,
    _iid: *const u8,
    factory_out: *mut *mut std::ffi::c_void,
) -> i32 {
    if !factory_out.is_null() {
        let mem = crate::dlls::d3d11::allocate_mock_com_object_with_meta(
            get_dwrite_factory_vtable() as usize,
            "dwrite!DWriteCreateFactory",
            "IDWriteFactory",
            "{B859EE5A-D838-4B5B-A2E8-1ADC7D93DB48}",
            "{B859EE5A-D838-4B5B-A2E8-1ADC7D93DB48}",
            25,
            (0..25).collect(),
        );
        *factory_out = mem;
    }
    0 // S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dwrite_create_factory_contract_and_vtable() {
        let mut factory_ptr: *mut std::ffi::c_void = std::ptr::null_mut();

        // 1. Invoke via IatSymbolResolver address
        let resolver_addr = crate::pe::imports::IatSymbolResolver::resolve_symbol(
            "DWrite.dll",
            "DWriteCreateFactory",
        )
        .expect("DWriteCreateFactory must resolve");

        let dwrite_create_fn: unsafe extern "win64" fn(
            u32,
            *const u8,
            *mut *mut std::ffi::c_void,
        ) -> i32 = unsafe { std::mem::transmute(resolver_addr) };

        let hr = unsafe { dwrite_create_fn(0, IID_IDWRITE_FACTORY.as_ptr(), &mut factory_ptr) };

        assert_eq!(hr, 0);
        assert!(!factory_ptr.is_null());

        // 2. Verify VTable Slot 0 (QueryInterface) for IID_IUnknown
        let mut qi_out: *mut std::ffi::c_void = std::ptr::null_mut();
        let qi_fn: unsafe extern "win64" fn(
            *mut std::ffi::c_void,
            *const u8,
            *mut *mut std::ffi::c_void,
        ) -> i32 = unsafe {
            let vtable = *(factory_ptr as *const *const *const ());
            std::mem::transmute(*vtable.add(0))
        };

        let hr_qi_unk = unsafe { qi_fn(factory_ptr, IID_IUNKNOWN.as_ptr(), &mut qi_out) };
        assert_eq!(hr_qi_unk, 0);
        assert_eq!(qi_out, factory_ptr);

        // 3. Verify QueryInterface for unknown IID returns E_NOINTERFACE
        let unknown_iid = [0xFFu8; 16];
        let mut qi_unknown_out: *mut std::ffi::c_void = factory_ptr;
        let hr_qi_unk_err =
            unsafe { qi_fn(factory_ptr, unknown_iid.as_ptr(), &mut qi_unknown_out) };
        assert_eq!(hr_qi_unk_err, E_NOINTERFACE);
        assert!(qi_unknown_out.is_null());

        // 4. Test Slot 3: GetSystemFontCollection
        let mut font_collection: *mut std::ffi::c_void = std::ptr::null_mut();
        let get_sys_fonts: unsafe extern "win64" fn(
            *mut std::ffi::c_void,
            *mut *mut std::ffi::c_void,
            i32,
        ) -> i32 = unsafe {
            let vtable = *(factory_ptr as *const *const *const ());
            std::mem::transmute(*vtable.add(3))
        };

        let hr_fonts = unsafe { get_sys_fonts(factory_ptr, &mut font_collection, 0) };
        assert_eq!(hr_fonts, 0);
        assert!(!font_collection.is_null());

        unsafe {
            crate::dlls::d3d11::com_object_release(font_collection);
            crate::dlls::d3d11::com_object_release(factory_ptr);
        }
    }
}
