use crate::sys::{NtStatus, STATUS_INVALID_PARAMETER, STATUS_SUCCESS};
use std::sync::{Arc, Condvar, Mutex as StdMutex};

#[derive(Debug, Clone)]
pub struct EventState {
    pub manual_reset: bool,
    pub signaled: bool,
}

#[derive(Debug, Clone)]
pub struct MutexState {
    pub owner_tid: Option<u64>,
    pub recursion: u32,
    pub signaled: bool,
}

#[derive(Debug, Clone)]
pub struct SemaphoreState {
    pub current_count: u32,
    pub maximum_count: u32,
}

#[derive(Debug, Clone)]
pub struct SectionState {
    pub size: u64,
    pub memory_address: usize,
}

#[derive(Debug, Clone)]
pub struct ProcessState {
    pub pid: u32,
    pub exit_code: u32,
    pub is_wow64: bool,
}

#[derive(Debug, Clone)]
pub struct ThreadState {
    pub tid: u64,
    pub exit_code: u32,
    pub teb_address: u64,
}

#[derive(Clone)]
pub enum KernelObject {
    Event {
        state: Arc<(StdMutex<EventState>, Condvar)>,
        name: String,
    },
    Mutant {
        state: Arc<(StdMutex<MutexState>, Condvar)>,
        name: String,
    },
    Semaphore {
        state: Arc<(StdMutex<SemaphoreState>, Condvar)>,
        name: String,
    },
    Section {
        state: Arc<SectionState>,
        name: String,
    },
    Process {
        state: Arc<StdMutex<ProcessState>>,
        name: String,
    },
    Thread {
        state: Arc<StdMutex<ThreadState>>,
        name: String,
    },
}

impl KernelObject {
    pub fn new_event(manual_reset: bool, initial_state: bool, name: &str) -> Self {
        Self::Event {
            state: Arc::new((
                StdMutex::new(EventState {
                    manual_reset,
                    signaled: initial_state,
                }),
                Condvar::new(),
            )),
            name: name.to_string(),
        }
    }

    pub fn new_mutex(initial_owner: bool, current_tid: u64, name: &str) -> Self {
        Self::Mutant {
            state: Arc::new((
                StdMutex::new(MutexState {
                    owner_tid: if initial_owner {
                        Some(current_tid)
                    } else {
                        None
                    },
                    recursion: if initial_owner { 1 } else { 0 },
                    signaled: !initial_owner,
                }),
                Condvar::new(),
            )),
            name: name.to_string(),
        }
    }

    pub fn new_semaphore(initial_count: u32, maximum_count: u32, name: &str) -> Self {
        Self::Semaphore {
            state: Arc::new((
                StdMutex::new(SemaphoreState {
                    current_count: initial_count,
                    maximum_count,
                }),
                Condvar::new(),
            )),
            name: name.to_string(),
        }
    }

    pub fn new_section(size: u64, memory_address: usize, name: &str) -> Self {
        Self::Section {
            state: Arc::new(SectionState {
                size,
                memory_address,
            }),
            name: name.to_string(),
        }
    }

    pub fn new_process(pid: u32, is_wow64: bool, name: &str) -> Self {
        Self::Process {
            state: Arc::new(StdMutex::new(ProcessState {
                pid,
                exit_code: 0x00000103, // STILL_ACTIVE (259)
                is_wow64,
            })),
            name: name.to_string(),
        }
    }

    pub fn new_thread(tid: u64, teb_address: u64, name: &str) -> Self {
        Self::Thread {
            state: Arc::new(StdMutex::new(ThreadState {
                tid,
                exit_code: 0x00000103, // STILL_ACTIVE (259)
                teb_address,
            })),
            name: name.to_string(),
        }
    }

    pub fn set_event(&self) -> NtStatus {
        if let Self::Event { state, .. } = self {
            let (lock, cvar) = &**state;
            let mut event = lock.lock().unwrap();
            event.signaled = true;
            if event.manual_reset {
                cvar.notify_all();
            } else {
                cvar.notify_one();
            }
            STATUS_SUCCESS
        } else {
            STATUS_INVALID_PARAMETER
        }
    }

    pub fn reset_event(&self) -> NtStatus {
        if let Self::Event { state, .. } = self {
            let (lock, _) = &**state;
            let mut event = lock.lock().unwrap();
            event.signaled = false;
            STATUS_SUCCESS
        } else {
            STATUS_INVALID_PARAMETER
        }
    }

    pub fn release_mutex(&self, current_tid: u64) -> NtStatus {
        if let Self::Mutant { state, .. } = self {
            let (lock, cvar) = &**state;
            let mut mtx = lock.lock().unwrap();
            if mtx.owner_tid != Some(current_tid) {
                return STATUS_INVALID_PARAMETER;
            }

            mtx.recursion -= 1;
            if mtx.recursion == 0 {
                mtx.owner_tid = None;
                mtx.signaled = true;
                cvar.notify_one();
            }
            STATUS_SUCCESS
        } else {
            STATUS_INVALID_PARAMETER
        }
    }

    pub fn release_semaphore(
        &self,
        release_count: u32,
        previous_count: Option<&mut u32>,
    ) -> NtStatus {
        if let Self::Semaphore { state, .. } = self {
            let (lock, cvar) = &**state;
            let mut sem = lock.lock().unwrap();

            if let Some(prev) = previous_count {
                *prev = sem.current_count;
            }

            if sem.current_count + release_count > sem.maximum_count {
                return STATUS_INVALID_PARAMETER;
            }

            sem.current_count += release_count;
            for _ in 0..release_count {
                cvar.notify_one();
            }
            STATUS_SUCCESS
        } else {
            STATUS_INVALID_PARAMETER
        }
    }
}
