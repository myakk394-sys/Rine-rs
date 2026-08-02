use crate::rineserver::objects::KernelObject;
use crate::sys::{NtStatus, STATUS_INVALID_HANDLE, STATUS_SUCCESS};
use std::collections::HashMap;
use std::sync::Mutex;

pub type Handle = u32;

pub struct HandleTable {
    handles: Mutex<HashMap<Handle, KernelObject>>,
    next_handle: Mutex<u32>,
}

impl HandleTable {
    pub fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            next_handle: Mutex::new(0x4), // Windows handles start at 0x4
        }
    }

    /// Allocates a new handle for a KernelObject.
    pub fn allocate(&self, obj: KernelObject) -> Handle {
        let mut next = self.next_handle.lock().unwrap();
        let handle = *next;
        *next += 4; // Increments of 4 (0x4, 0x8, 0xC...)

        let mut map = self.handles.lock().unwrap();
        map.insert(handle, obj);
        handle
    }

    /// Retrieves a cloned KernelObject by handle.
    pub fn get(&self, handle: Handle) -> Option<KernelObject> {
        let map = self.handles.lock().unwrap();
        map.get(&handle).cloned()
    }

    /// Closes a handle.
    pub fn close(&self, handle: Handle) -> NtStatus {
        let mut map = self.handles.lock().unwrap();
        if map.remove(&handle).is_some() {
            STATUS_SUCCESS
        } else {
            STATUS_INVALID_HANDLE
        }
    }

    /// Duplicates a handle.
    pub fn duplicate(&self, handle: Handle) -> Result<Handle, NtStatus> {
        if let Some(obj) = self.get(handle) {
            Ok(self.allocate(obj))
        } else {
            Err(STATUS_INVALID_HANDLE)
        }
    }

    pub fn len(&self) -> usize {
        let map = self.handles.lock().unwrap();
        map.len()
    }
}
