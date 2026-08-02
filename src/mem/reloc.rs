use crate::sys::Result;

pub const IMAGE_REL_BASED_ABSOLUTE: u16 = 0;
pub const IMAGE_REL_BASED_DIR64: u16 = 10;

#[repr(C, packed)]
pub struct ImageBaseRelocation {
    pub virtual_address: u32,
    pub size_of_block: u32,
}

pub struct BaseRelocator;

impl BaseRelocator {
    /// Applies PE base relocations if allocated base address differs from preferred image base.
    pub unsafe fn apply_relocations(
        mapped_base: usize,
        preferred_base: usize,
        reloc_rva: usize,
        reloc_size: usize,
    ) -> Result<()> {
        if mapped_base == preferred_base || reloc_size == 0 {
            return Ok(());
        }

        let delta = mapped_base.wrapping_sub(preferred_base) as i64;
        println!(
            "[PE RELOCATOR] Applying base relocations. Allocated: 0x{:016X}, Preferred: 0x{:016X}, Delta: 0x{:016X}",
            mapped_base, preferred_base, delta
        );

        let mut offset = 0;
        while offset < reloc_size {
            let block_ptr = (mapped_base + reloc_rva + offset) as *const ImageBaseRelocation;
            let block_rva = (*block_ptr).virtual_address as usize;
            let block_size = (*block_ptr).size_of_block as usize;

            if block_size == 0 {
                break;
            }

            let entries_count = (block_size - std::mem::size_of::<ImageBaseRelocation>()) / 2;
            let entries_ptr =
                (block_ptr as usize + std::mem::size_of::<ImageBaseRelocation>()) as *const u16;

            for i in 0..entries_count {
                let entry = *entries_ptr.add(i);
                let reloc_type = entry >> 12;
                let reloc_offset = (entry & 0x0FFF) as usize;

                if reloc_type == IMAGE_REL_BASED_DIR64 {
                    let target_addr = (mapped_base + block_rva + reloc_offset) as *mut u64;
                    let old_val = *target_addr;
                    let new_val = (old_val as i64 + delta) as u64;
                    *target_addr = new_val;
                }
            }

            offset += block_size;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_relocation_no_delta() {
        let res = unsafe { BaseRelocator::apply_relocations(0x140000000, 0x140000000, 0, 0) };
        assert!(res.is_ok());
    }
}
