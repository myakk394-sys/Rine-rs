#![allow(non_snake_case)]
use crate::sys::{NtStatus, STATUS_SUCCESS};

pub fn BCryptOpenAlgorithmProvider(
    ph_algorithm: &mut u64,
    psz_alg_id: &str,
    _psz_implementation: Option<&str>,
    _dw_flags: u32,
) -> NtStatus {
    *ph_algorithm = 0x00050001;
    println!(
        "[BCRYPT] BCryptOpenAlgorithmProvider: AlgID='{}' -> Handle=0x{:X}",
        psz_alg_id, *ph_algorithm
    );
    STATUS_SUCCESS
}

pub fn BCryptCloseAlgorithmProvider(h_algorithm: u64, _dw_flags: u32) -> NtStatus {
    println!(
        "[BCRYPT] BCryptCloseAlgorithmProvider: Handle=0x{:X}",
        h_algorithm
    );
    STATUS_SUCCESS
}
