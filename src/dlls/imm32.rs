#![allow(non_snake_case)]

pub unsafe extern "win64" fn ImmGetContext(_hwnd: u64) -> u64 {
    0x00080001
}

pub unsafe extern "win64" fn ImmReleaseContext(_hwnd: u64, _himc: u64) -> i32 {
    1
}

pub unsafe extern "win64" fn ImmAssociateContext(_hwnd: u64, himc: u64) -> u64 {
    himc
}

pub unsafe extern "win64" fn ImmGetCompositionStringW(
    _himc: u64,
    _index: u32,
    _buf: u64,
    _buf_len: u32,
) -> i32 {
    0
}

pub unsafe extern "win64" fn ImmSetCandidateWindow(_himc: u64, _cand_form: u64) -> i32 {
    1
}

pub unsafe extern "win64" fn ImmSetCompositionWindow(_himc: u64, _comp_form: u64) -> i32 {
    1
}
