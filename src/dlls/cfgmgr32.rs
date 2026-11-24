#![allow(non_snake_case)]

pub const CR_SUCCESS: u32 = 0;
pub const CR_DEFAULT: u32 = 1;
pub const CR_OUT_OF_MEMORY: u32 = 2;
pub const CR_INVALID_POINTER: u32 = 3;
pub const CR_INVALID_FLAG: u32 = 4;
pub const CR_INVALID_DEVNODE: u32 = 5;
pub const CR_NO_SUCH_DEVNODE: u32 = 13;
pub const CR_NO_SUCH_VALUE: u32 = 37;

pub const DN_ROOT_ENUMERATED: u32 = 0x00000001;
pub const DN_DRIVER_LOADED: u32 = 0x00000002;
pub const DN_ENUM_LOADED: u32 = 0x00000004;
pub const DN_STARTED: u32 = 0x00000008;

#[no_mangle]
pub unsafe extern "win64" fn CM_Locate_DevNodeA(
    p_dn_dev_node: *mut u32,
    _p_device_id: *const u8,
    _flags: u32,
) -> u32 {
    if !p_dn_dev_node.is_null() {
        *p_dn_dev_node = 1;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Locate_DevNodeW(
    p_dn_dev_node: *mut u32,
    _p_device_id: *const u16,
    _flags: u32,
) -> u32 {
    if !p_dn_dev_node.is_null() {
        *p_dn_dev_node = 1;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Locate_DevNode_ExA(
    p_dn_dev_node: *mut u32,
    p_device_id: *const u8,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Locate_DevNodeA(p_dn_dev_node, p_device_id, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Locate_DevNode_ExW(
    p_dn_dev_node: *mut u32,
    p_device_id: *const u16,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Locate_DevNodeW(p_dn_dev_node, p_device_id, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Child(
    p_dn_dev_node: *mut u32,
    _dn_dev_node: u32,
    _flags: u32,
) -> u32 {
    if !p_dn_dev_node.is_null() {
        *p_dn_dev_node = 0;
    }
    CR_NO_SUCH_DEVNODE
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_ChildA(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Child(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_ChildW(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Child(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Child_Ex(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Get_Child(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Child_ExA(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    h_machine: usize,
) -> u32 {
    CM_Get_Child_Ex(p_dn_dev_node, dn_dev_node, flags, h_machine)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Child_ExW(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    h_machine: usize,
) -> u32 {
    CM_Get_Child_Ex(p_dn_dev_node, dn_dev_node, flags, h_machine)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Sibling(
    p_dn_dev_node: *mut u32,
    _dn_dev_node: u32,
    _flags: u32,
) -> u32 {
    if !p_dn_dev_node.is_null() {
        *p_dn_dev_node = 0;
    }
    CR_NO_SUCH_DEVNODE
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_SiblingA(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Sibling(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_SiblingW(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Sibling(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Sibling_Ex(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Get_Sibling(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Sibling_ExA(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    h_machine: usize,
) -> u32 {
    CM_Get_Sibling_Ex(p_dn_dev_node, dn_dev_node, flags, h_machine)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Sibling_ExW(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    h_machine: usize,
) -> u32 {
    CM_Get_Sibling_Ex(p_dn_dev_node, dn_dev_node, flags, h_machine)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Parent(
    p_dn_dev_node: *mut u32,
    _dn_dev_node: u32,
    _flags: u32,
) -> u32 {
    if p_dn_dev_node.is_null() {
        return CR_INVALID_POINTER;
    }
    *p_dn_dev_node = 0;
    // On Windows, if dn_dev_node specifies the root or a top-level device node
    // with no parent, CM_Get_Parent returns CR_NO_SUCH_DEVNODE.
    // Returning CR_SUCCESS when dn_dev_node == 0 causes infinite loops in games
    // (such as Unity) that traverse parent nodes via while (CM_Get_Parent(...) == CR_SUCCESS).
    CR_NO_SUCH_DEVNODE
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_ParentA(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Parent(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_ParentW(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Parent(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Parent_Ex(
    p_dn_dev_node: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Get_Parent(p_dn_dev_node, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_IDA(
    _dn_dev_node: u32,
    buffer: *mut u8,
    buffer_len: u32,
    _flags: u32,
) -> u32 {
    let id = b"HID\\VID_046D&PID_C077\\0000\0";
    if !buffer.is_null() && buffer_len > 0 {
        let copy_len = (buffer_len as usize).min(id.len());
        std::ptr::copy_nonoverlapping(id.as_ptr(), buffer, copy_len);
        *buffer.add(copy_len - 1) = 0;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_IDW(
    _dn_dev_node: u32,
    buffer: *mut u16,
    buffer_len: u32,
    _flags: u32,
) -> u32 {
    let id: Vec<u16> = "HID\\VID_046D&PID_C077\\0000\0".encode_utf16().collect();
    if !buffer.is_null() && buffer_len > 0 {
        let copy_len = (buffer_len as usize).min(id.len());
        std::ptr::copy_nonoverlapping(id.as_ptr(), buffer, copy_len);
        *buffer.add(copy_len - 1) = 0;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_ExA(
    dn_dev_node: u32,
    buffer: *mut u8,
    buffer_len: u32,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Get_Device_IDA(dn_dev_node, buffer, buffer_len, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_ExW(
    dn_dev_node: u32,
    buffer: *mut u16,
    buffer_len: u32,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Get_Device_IDW(dn_dev_node, buffer, buffer_len, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_Size(
    pulLen: *mut u32,
    _dn_dev_node: u32,
    _flags: u32,
) -> u32 {
    if !pulLen.is_null() {
        *pulLen = 27; // Length of "HID\\VID_046D&PID_C077\\0000"
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_SizeA(
    pulLen: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Device_ID_Size(pulLen, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_SizeW(
    pulLen: *mut u32,
    dn_dev_node: u32,
    flags: u32,
) -> u32 {
    CM_Get_Device_ID_Size(pulLen, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_Size_Ex(
    pulLen: *mut u32,
    dn_dev_node: u32,
    flags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Get_Device_ID_Size(pulLen, dn_dev_node, flags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_ListA(
    _pszFilter: *const u8,
    Buffer: *mut u8,
    BufferLen: u32,
    _ulFlags: u32,
) -> u32 {
    if !Buffer.is_null() && BufferLen >= 2 {
        *Buffer = 0;
        *Buffer.add(1) = 0;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_ListW(
    _pszFilter: *const u16,
    Buffer: *mut u16,
    BufferLen: u32,
    _ulFlags: u32,
) -> u32 {
    if !Buffer.is_null() && BufferLen >= 2 {
        *Buffer = 0;
        *Buffer.add(1) = 0;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_List_SizeA(
    pulLen: *mut u32,
    _pszFilter: *const u8,
    _ulFlags: u32,
) -> u32 {
    if !pulLen.is_null() {
        *pulLen = 2;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_Device_ID_List_SizeW(
    pulLen: *mut u32,
    _pszFilter: *const u16,
    _ulFlags: u32,
) -> u32 {
    if !pulLen.is_null() {
        *pulLen = 2;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_DevNode_Status(
    pulStatus: *mut u32,
    pulProblemNumber: *mut u32,
    _dnDevInst: u32,
    _ulFlags: u32,
) -> u32 {
    if !pulStatus.is_null() {
        *pulStatus = DN_DRIVER_LOADED | DN_STARTED | DN_ROOT_ENUMERATED;
    }
    if !pulProblemNumber.is_null() {
        *pulProblemNumber = 0;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_DevNode_Status_Ex(
    pulStatus: *mut u32,
    pulProblemNumber: *mut u32,
    dnDevInst: u32,
    ulFlags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Get_DevNode_Status(pulStatus, pulProblemNumber, dnDevInst, ulFlags)
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_DevNode_Registry_PropertyA(
    _dnDevInst: u32,
    _ulProperty: u32,
    pulRegDataType: *mut u32,
    Buffer: *mut u8,
    pulLength: *mut u32,
    _ulFlags: u32,
) -> u32 {
    if !pulRegDataType.is_null() {
        *pulRegDataType = 1; // REG_SZ
    }
    if !pulLength.is_null() {
        if !Buffer.is_null() && *pulLength > 0 {
            *Buffer = 0;
        }
        *pulLength = 1;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Get_DevNode_Registry_PropertyW(
    _dnDevInst: u32,
    _ulProperty: u32,
    pulRegDataType: *mut u32,
    Buffer: *mut u16,
    pulLength: *mut u32,
    _ulFlags: u32,
) -> u32 {
    if !pulRegDataType.is_null() {
        *pulRegDataType = 1; // REG_SZ
    }
    if !pulLength.is_null() {
        if !Buffer.is_null() && *pulLength >= 2 {
            *Buffer = 0;
        }
        *pulLength = 2;
    }
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Enumerate_EnumeratorsA(
    _ulEnumIndex: u32,
    Buffer: *mut u8,
    pulLength: *mut u32,
    _ulFlags: u32,
) -> u32 {
    if !pulLength.is_null() {
        if !Buffer.is_null() && *pulLength > 0 {
            *Buffer = 0;
        }
        *pulLength = 1;
    }
    CR_NO_SUCH_VALUE
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Enumerate_EnumeratorsW(
    _ulEnumIndex: u32,
    Buffer: *mut u16,
    pulLength: *mut u32,
    _ulFlags: u32,
) -> u32 {
    if !pulLength.is_null() {
        if !Buffer.is_null() && *pulLength >= 2 {
            *Buffer = 0;
        }
        *pulLength = 2;
    }
    CR_NO_SUCH_VALUE
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Reenumerate_DevNode(_dnDevInst: u32, _ulFlags: u32) -> u32 {
    CR_SUCCESS
}

#[no_mangle]
pub unsafe extern "win64" fn CM_Reenumerate_DevNode_Ex(
    dnDevInst: u32,
    ulFlags: u32,
    _h_machine: usize,
) -> u32 {
    CM_Reenumerate_DevNode(dnDevInst, ulFlags)
}

pub fn resolve_cfgmgr32_symbol(func_name: &str) -> Option<usize> {
    let clean = if func_name.starts_with('_') || func_name.contains('@') {
        let mut s = func_name;
        if let Some(stripped) = s.strip_prefix('_') {
            s = stripped;
        }
        if let Some(idx) = s.find('@') {
            s = &s[..idx];
        }
        s
    } else {
        func_name
    };

    match clean {
        "CM_Locate_DevNode" | "CM_Locate_DevNodeA" => Some(CM_Locate_DevNodeA as *const () as usize),
        "CM_Locate_DevNodeW" => Some(CM_Locate_DevNodeW as *const () as usize),
        "CM_Locate_DevNode_Ex" | "CM_Locate_DevNode_ExA" => Some(CM_Locate_DevNode_ExA as *const () as usize),
        "CM_Locate_DevNode_ExW" => Some(CM_Locate_DevNode_ExW as *const () as usize),
        "CM_Get_Child" => Some(CM_Get_Child as *const () as usize),
        "CM_Get_ChildA" => Some(CM_Get_ChildA as *const () as usize),
        "CM_Get_ChildW" => Some(CM_Get_ChildW as *const () as usize),
        "CM_Get_Child_Ex" => Some(CM_Get_Child_Ex as *const () as usize),
        "CM_Get_Child_ExA" => Some(CM_Get_Child_ExA as *const () as usize),
        "CM_Get_Child_ExW" => Some(CM_Get_Child_ExW as *const () as usize),
        "CM_Get_Sibling" => Some(CM_Get_Sibling as *const () as usize),
        "CM_Get_SiblingA" => Some(CM_Get_SiblingA as *const () as usize),
        "CM_Get_SiblingW" => Some(CM_Get_SiblingW as *const () as usize),
        "CM_Get_Sibling_Ex" => Some(CM_Get_Sibling_Ex as *const () as usize),
        "CM_Get_Sibling_ExA" => Some(CM_Get_Sibling_ExA as *const () as usize),
        "CM_Get_Sibling_ExW" => Some(CM_Get_Sibling_ExW as *const () as usize),
        "CM_Get_Parent" => Some(CM_Get_Parent as *const () as usize),
        "CM_Get_ParentA" => Some(CM_Get_ParentA as *const () as usize),
        "CM_Get_ParentW" => Some(CM_Get_ParentW as *const () as usize),
        "CM_Get_Parent_Ex" => Some(CM_Get_Parent_Ex as *const () as usize),
        "CM_Get_Device_ID" | "CM_Get_Device_IDA" => Some(CM_Get_Device_IDA as *const () as usize),
        "CM_Get_Device_IDW" => Some(CM_Get_Device_IDW as *const () as usize),
        "CM_Get_Device_ID_Ex" | "CM_Get_Device_ID_ExA" => Some(CM_Get_Device_ID_ExA as *const () as usize),
        "CM_Get_Device_ID_ExW" => Some(CM_Get_Device_ID_ExW as *const () as usize),
        "CM_Get_Device_ID_Size" | "CM_Get_Device_ID_SizeA" => Some(CM_Get_Device_ID_Size as *const () as usize),
        "CM_Get_Device_ID_SizeW" => Some(CM_Get_Device_ID_SizeW as *const () as usize),
        "CM_Get_Device_ID_Size_Ex" => Some(CM_Get_Device_ID_Size_Ex as *const () as usize),
        "CM_Get_Device_ID_List" | "CM_Get_Device_ID_ListA" => Some(CM_Get_Device_ID_ListA as *const () as usize),
        "CM_Get_Device_ID_ListW" => Some(CM_Get_Device_ID_ListW as *const () as usize),
        "CM_Get_Device_ID_List_Size" | "CM_Get_Device_ID_List_SizeA" => Some(CM_Get_Device_ID_List_SizeA as *const () as usize),
        "CM_Get_Device_ID_List_SizeW" => Some(CM_Get_Device_ID_List_SizeW as *const () as usize),
        "CM_Get_DevNode_Status" => Some(CM_Get_DevNode_Status as *const () as usize),
        "CM_Get_DevNode_Status_Ex" => Some(CM_Get_DevNode_Status_Ex as *const () as usize),
        "CM_Get_DevNode_Registry_Property" | "CM_Get_DevNode_Registry_PropertyA" => Some(CM_Get_DevNode_Registry_PropertyA as *const () as usize),
        "CM_Get_DevNode_Registry_PropertyW" => Some(CM_Get_DevNode_Registry_PropertyW as *const () as usize),
        "CM_Enumerate_EnumeratorsA" => Some(CM_Enumerate_EnumeratorsA as *const () as usize),
        "CM_Enumerate_EnumeratorsW" => Some(CM_Enumerate_EnumeratorsW as *const () as usize),
        "CM_Reenumerate_DevNode" => Some(CM_Reenumerate_DevNode as *const () as usize),
        "CM_Reenumerate_DevNode_Ex" => Some(CM_Reenumerate_DevNode_Ex as *const () as usize),
        _ => None,
    }
}
