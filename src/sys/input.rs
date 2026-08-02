pub const WM_NULL: u32 = 0x0000;
pub const WM_CREATE: u32 = 0x0001;
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_QUIT: u32 = 0x0012;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_RBUTTONUP: u32 = 0x0205;

// Virtual Key Codes (VK)
pub const VK_LBUTTON: u32 = 0x01;
pub const VK_RBUTTON: u32 = 0x02;
pub const VK_BACK: u32 = 0x08;
pub const VK_TAB: u32 = 0x09;
pub const VK_RETURN: u32 = 0x0D;
pub const VK_SHIFT: u32 = 0x10;
pub const VK_CONTROL: u32 = 0x11;
pub const VK_MENU: u32 = 0x12; // Alt key
pub const VK_ESCAPE: u32 = 0x1B;
pub const VK_SPACE: u32 = 0x20;
pub const VK_LEFT: u32 = 0x25;
pub const VK_UP: u32 = 0x26;
pub const VK_RIGHT: u32 = 0x27;
pub const VK_DOWN: u32 = 0x28;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WinMsg {
    pub hwnd: u64,
    pub message: u32,
    pub w_param: u64,
    pub l_param: u64,
    pub time: u32,
    pub pt_x: i32,
    pub pt_y: i32,
}

pub struct InputSubsystem {
    pub message_queue: std::collections::VecDeque<WinMsg>,
}

impl InputSubsystem {
    pub fn new() -> Self {
        Self {
            message_queue: std::collections::VecDeque::new(),
        }
    }

    /// Captures a mouse movement event and pushes to Win32 message queue.
    pub fn push_mouse_move(&mut self, hwnd: u64, x: i32, y: i32) {
        let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);
        self.message_queue.push_back(WinMsg {
            hwnd,
            message: WM_MOUSEMOVE,
            w_param: 0,
            l_param,
            time: 0,
            pt_x: x,
            pt_y: y,
        });
    }

    /// Captures a mouse click event (down/up).
    pub fn push_mouse_click(&mut self, hwnd: u64, is_left: bool, is_down: bool, x: i32, y: i32) {
        let message = match (is_left, is_down) {
            (true, true) => WM_LBUTTONDOWN,
            (true, false) => WM_LBUTTONUP,
            (false, true) => WM_RBUTTONDOWN,
            (false, false) => WM_RBUTTONUP,
        };
        let l_param = ((y as u64 & 0xFFFF) << 16) | (x as u64 & 0xFFFF);
        let w_param = if is_left {
            VK_LBUTTON as u64
        } else {
            VK_RBUTTON as u64
        };

        self.message_queue.push_back(WinMsg {
            hwnd,
            message,
            w_param,
            l_param,
            time: 0,
            pt_x: x,
            pt_y: y,
        });
    }

    /// Captures a keyboard key event.
    pub fn push_key(&mut self, hwnd: u64, vk_code: u32, is_down: bool) {
        let message = if is_down { WM_KEYDOWN } else { WM_KEYUP };
        self.message_queue.push_back(WinMsg {
            hwnd,
            message,
            w_param: vk_code as u64,
            l_param: 1, // Repeat count 1
            time: 0,
            pt_x: 0,
            pt_y: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_subsystem_translation() {
        let mut input = InputSubsystem::new();
        input.push_mouse_move(0x100, 450, 300);
        input.push_key(0x100, VK_RETURN, true);

        assert_eq!(input.message_queue.len(), 2);
        let msg1 = input.message_queue.pop_front().unwrap();
        assert_eq!(msg1.message, WM_MOUSEMOVE);
        assert_eq!(msg1.pt_x, 450);
        assert_eq!(msg1.pt_y, 300);

        let msg2 = input.message_queue.pop_front().unwrap();
        assert_eq!(msg2.message, WM_KEYDOWN);
        assert_eq!(msg2.w_param, VK_RETURN as u64);
    }
}
