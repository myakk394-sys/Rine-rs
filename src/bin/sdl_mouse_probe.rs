use std::ffi::{c_char, c_int, c_void, CString};

type SdlInitFn = unsafe extern "C" fn(u32) -> c_int;
type SdlCreateWindowFn =
    unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, c_int, u32) -> *mut c_void;
type SdlDestroyWindowFn = unsafe extern "C" fn(*mut c_void);
type SdlPollEventFn = unsafe extern "C" fn(*mut u8) -> c_int;
type SdlEventStateFn = unsafe extern "C" fn(u32, c_int) -> u8;
type SdlGetMouseStateFn = unsafe extern "C" fn(*mut c_int, *mut c_int) -> u32;
type SdlGetGlobalMouseStateFn = unsafe extern "C" fn(*mut c_int, *mut c_int) -> u32;
type SdlGetMouseFocusFn = unsafe extern "C" fn() -> *mut c_void;
type SdlGetKeyboardFocusFn = unsafe extern "C" fn() -> *mut c_void;
type SdlGetWindowIdFn = unsafe extern "C" fn(*mut c_void) -> u32;
type SdlGetTicksFn = unsafe extern "C" fn() -> u32;

fn main() {
    println!("============================================================");
    println!("STANDALONE SDL PHYSICAL MOUSE INPUT PROBE");
    println!("============================================================");

    let sdl_lib = unsafe {
        libc::dlopen(
            b"libSDL2-2.0.so.0\0".as_ptr() as *const c_char,
            libc::RTLD_NOW,
        )
    };
    if sdl_lib.is_null() {
        eprintln!("[PROBE ERROR] Failed to load libSDL2-2.0.so.0");
        return;
    }

    unsafe {
        let fn_init: SdlInitFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_Init\0".as_ptr() as *const c_char,
        ));
        let fn_create_window: SdlCreateWindowFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_CreateWindow\0".as_ptr() as *const c_char,
        ));
        let fn_destroy_window: SdlDestroyWindowFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_DestroyWindow\0".as_ptr() as *const c_char,
        ));
        let fn_poll_event: SdlPollEventFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_PollEvent\0".as_ptr() as *const c_char,
        ));
        let fn_event_state: SdlEventStateFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_EventState\0".as_ptr() as *const c_char,
        ));
        let fn_get_mouse_state: SdlGetMouseStateFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_GetMouseState\0".as_ptr() as *const c_char,
        ));
        let sym_global_mouse = libc::dlsym(
            sdl_lib,
            b"SDL_GetGlobalMouseState\0".as_ptr() as *const c_char,
        );
        let fn_get_global_mouse_state: Option<SdlGetGlobalMouseStateFn> =
            if !sym_global_mouse.is_null() {
                Some(std::mem::transmute(sym_global_mouse))
            } else {
                None
            };
        let fn_get_mouse_focus: SdlGetMouseFocusFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_GetMouseFocus\0".as_ptr() as *const c_char,
        ));
        let fn_get_keyboard_focus: SdlGetKeyboardFocusFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_GetKeyboardFocus\0".as_ptr() as *const c_char,
        ));
        let fn_get_window_id: SdlGetWindowIdFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_GetWindowID\0".as_ptr() as *const c_char,
        ));
        let fn_get_ticks: SdlGetTicksFn = std::mem::transmute(libc::dlsym(
            sdl_lib,
            b"SDL_GetTicks\0".as_ptr() as *const c_char,
        ));

        // 1. Initialize SDL2
        let init_res = fn_init(0x00000020 /* SDL_INIT_VIDEO */);
        println!("[PROBE] SDL_Init(SDL_INIT_VIDEO) -> {}", init_res);

        // 2. Query initial SDL Event States (SDL_QUERY = -1)
        let state_motion = fn_event_state(0x400 /* SDL_MOUSEMOTION */, -1);
        let state_down = fn_event_state(0x401 /* SDL_MOUSEBUTTONDOWN */, -1);
        let state_up = fn_event_state(0x402 /* SDL_MOUSEBUTTONUP */, -1);
        let state_wheel = fn_event_state(0x403 /* SDL_MOUSEWHEEL */, -1);

        println!("------------------------------------------------------------");
        println!("SDL EVENT STATES (SDL_QUERY):");
        println!(
            "  SDL_MOUSEMOTION     (0x400): {}",
            if state_motion == 1 {
                "SDL_ENABLE (1)"
            } else {
                "SDL_DISABLE (0)"
            }
        );
        println!(
            "  SDL_MOUSEBUTTONDOWN (0x401): {}",
            if state_down == 1 {
                "SDL_ENABLE (1)"
            } else {
                "SDL_DISABLE (0)"
            }
        );
        println!(
            "  SDL_MOUSEBUTTONUP   (0x402): {}",
            if state_up == 1 {
                "SDL_ENABLE (1)"
            } else {
                "SDL_DISABLE (0)"
            }
        );
        println!(
            "  SDL_MOUSEWHEEL      (0x403): {}",
            if state_wheel == 1 {
                "SDL_ENABLE (1)"
            } else {
                "SDL_DISABLE (0)"
            }
        );
        println!("------------------------------------------------------------");

        // 3. Create window
        let title = CString::new("Rine Standalone SDL Mouse Probe").unwrap();
        let win = fn_create_window(
            title.as_ptr(),
            0x2FFF0000, // SDL_WINDOWPOS_CENTERED
            0x2FFF0000,
            800,
            600,
            0x00000004 | 0x00000020, // SDL_WINDOW_SHOWN | SDL_WINDOW_RESIZABLE
        );
        if win.is_null() {
            eprintln!("[PROBE ERROR] Failed to create SDL window");
            return;
        }
        let win_id = fn_get_window_id(win);
        println!("[PROBE] Created SDL Window: {:p}, ID={}", win, win_id);

        let start_ticks = fn_get_ticks();
        let mut sdl_event = [0u8; 56];
        let mut event_count = 0usize;
        let mut motion_count = 0usize;
        let mut down_count = 0usize;
        let mut up_count = 0usize;

        println!("[PROBE] Running event loop for 60 seconds or until window close / ESC / Q...");
        println!("[PROBE] PLEASE MOVE MOUSE AND CLICK LEFT BUTTON INSIDE THE PROBE WINDOW NOW!");

        while fn_get_ticks().wrapping_sub(start_ticks) < 60000 {
            while fn_poll_event(sdl_event.as_mut_ptr()) != 0 {
                event_count += 1;
                let event_type =
                    u32::from_le_bytes([sdl_event[0], sdl_event[1], sdl_event[2], sdl_event[3]]);
                let timestamp =
                    u32::from_le_bytes([sdl_event[4], sdl_event[5], sdl_event[6], sdl_event[7]]);
                let event_win_id =
                    u32::from_le_bytes([sdl_event[8], sdl_event[9], sdl_event[10], sdl_event[11]]);

                match event_type {
                    0x400 => {
                        // SDL_MOUSEMOTION
                        motion_count += 1;
                        let state = u32::from_le_bytes(sdl_event[16..20].try_into().unwrap());
                        let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
                        let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
                        if motion_count <= 5 || motion_count % 50 == 0 {
                            println!(
                                "[PROBE EVENT #{} | +{}ms] SDL_MOUSEMOTION: winID={}, x={}, y={}, btn_state=0x{:X}",
                                event_count, timestamp.wrapping_sub(start_ticks), event_win_id, x, y, state
                            );
                        }
                    }
                    0x401 => {
                        // SDL_MOUSEBUTTONDOWN
                        down_count += 1;
                        let button = sdl_event[16];
                        let state = sdl_event[17];
                        let clicks = sdl_event[18];
                        let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
                        let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
                        println!(
                            "[PROBE EVENT #{} | +{}ms] >>> SDL_MOUSEBUTTONDOWN <<<: winID={}, button={}, state={}, clicks={}, x={}, y={}",
                            event_count, timestamp.wrapping_sub(start_ticks), event_win_id, button, state, clicks, x, y
                        );
                    }
                    0x402 => {
                        // SDL_MOUSEBUTTONUP
                        up_count += 1;
                        let button = sdl_event[16];
                        let state = sdl_event[17];
                        let clicks = sdl_event[18];
                        let x = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
                        let y = i32::from_le_bytes(sdl_event[24..28].try_into().unwrap());
                        println!(
                            "[PROBE EVENT #{} | +{}ms] >>> SDL_MOUSEBUTTONUP <<<: winID={}, button={}, state={}, clicks={}, x={}, y={}",
                            event_count, timestamp.wrapping_sub(start_ticks), event_win_id, button, state, clicks, x, y
                        );
                    }
                    0x300 => {
                        // SDL_KEYDOWN
                        let sym = i32::from_le_bytes(sdl_event[20..24].try_into().unwrap());
                        println!(
                            "[PROBE EVENT #{} | +{}ms] SDL_KEYDOWN: sym=0x{:X}",
                            event_count,
                            timestamp.wrapping_sub(start_ticks),
                            sym
                        );
                        if sym == 27 || sym == b'q' as i32 || sym == b'Q' as i32 {
                            println!("[PROBE] Exit key pressed.");
                            break;
                        }
                    }
                    0x100 => {
                        // SDL_QUIT
                        println!("[PROBE] SDL_QUIT received.");
                        break;
                    }
                    _ => {
                        if event_count <= 10 {
                            println!(
                                "[PROBE EVENT #{} | +{}ms] Raw Event type=0x{:04X}, winID={}",
                                event_count,
                                timestamp.wrapping_sub(start_ticks),
                                event_type,
                                event_win_id
                            );
                        }
                    }
                }
            }

            // Sample mouse state and focus periodically
            let mut mx = 0i32;
            let mut my = 0i32;
            let mstate = fn_get_mouse_state(&mut mx, &mut my);
            let mfocus = fn_get_mouse_focus();
            let kfocus = fn_get_keyboard_focus();

            if (mstate & 1) != 0 {
                println!(
                    "[PROBE STATE] SDL_GetMouseState LEFT BUTTON PRESSED! x={}, y={}, mask=0x{:X}, mfocus={:p}, kfocus={:p}",
                    mx, my, mstate, mfocus, kfocus
                );
            }

            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        println!("============================================================");
        println!("PROBE SUMMARY:");
        println!("  Total events polled: {}", event_count);
        println!("  Motion events:       {}", motion_count);
        println!("  Button Down events:  {}", down_count);
        println!("  Button Up events:    {}", up_count);
        println!("============================================================");

        fn_destroy_window(win);
    }
}
