use crate::pe::headers::*;
use crate::pe::imports::ImageImportDescriptor;
use crate::pe::section::ImageSectionHeader;
use crate::sys::{Result, RineError};

pub struct PeImage<'a> {
    pub raw_data: &'a [u8],
    pub dos_header: &'a ImageDosHeader,
    pub file_header: &'a ImageFileHeader,
    pub optional_header: &'a ImageOptionalHeader64,
    pub sections: &'a [ImageSectionHeader],
}

impl<'a> PeImage<'a> {
    /// Parses a PE32+ (Win64) executable binary from raw bytes.
    pub fn parse(raw_data: &'a [u8]) -> Result<Self> {
        if raw_data.len() < std::mem::size_of::<ImageDosHeader>() {
            return Err(RineError::TruncatedData);
        }

        // 1. Parse DOS Header
        let dos_header = unsafe { &*(raw_data.as_ptr() as *const ImageDosHeader) };
        if dos_header.e_magic() != IMAGE_DOS_SIGNATURE {
            return Err(RineError::InvalidDosHeader(dos_header.e_magic()));
        }

        let pe_offset = dos_header.e_lfanew() as usize;
        if pe_offset + 4 + std::mem::size_of::<ImageFileHeader>() > raw_data.len() {
            return Err(RineError::TruncatedData);
        }

        // 2. Parse PE Signature
        let pe_sig = unsafe { *(raw_data.as_ptr().add(pe_offset) as *const u32) };
        if pe_sig != IMAGE_NT_SIGNATURE {
            return Err(RineError::InvalidPeSignature(pe_sig));
        }

        // 3. Parse File Header
        let file_header_ptr =
            unsafe { raw_data.as_ptr().add(pe_offset + 4) as *const ImageFileHeader };
        let file_header = unsafe { &*file_header_ptr };

        if file_header.machine() != IMAGE_FILE_MACHINE_AMD64 {
            return Err(RineError::UnsupportedArchitecture(file_header.machine()));
        }

        // 4. Parse Optional Header (64-bit)
        let opt_header_offset = pe_offset + 4 + std::mem::size_of::<ImageFileHeader>();
        if opt_header_offset + std::mem::size_of::<ImageOptionalHeader64>() > raw_data.len() {
            return Err(RineError::TruncatedData);
        }

        let optional_header_ptr =
            unsafe { raw_data.as_ptr().add(opt_header_offset) as *const ImageOptionalHeader64 };
        let optional_header = unsafe { &*optional_header_ptr };

        if optional_header.magic() != IMAGE_NT_OPTIONAL_HDR64_MAGIC {
            return Err(RineError::UnsupportedPeFormat(optional_header.magic()));
        }

        // 5. Parse Section Headers
        let sections_offset = opt_header_offset + file_header.size_of_optional_header() as usize;
        let num_sections = file_header.number_of_sections() as usize;
        let total_sections_size = num_sections * std::mem::size_of::<ImageSectionHeader>();

        if sections_offset + total_sections_size > raw_data.len() {
            return Err(RineError::TruncatedData);
        }

        let sections = unsafe {
            std::slice::from_raw_parts(
                raw_data.as_ptr().add(sections_offset) as *const ImageSectionHeader,
                num_sections,
            )
        };

        Ok(Self {
            raw_data,
            dos_header,
            file_header,
            optional_header,
            sections,
        })
    }

    /// Converts a Relative Virtual Address (RVA) to raw file offset.
    pub fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        for section in self.sections {
            let sec_vaddr = section.virtual_address();
            let sec_vsize = if section.virtual_size() == 0 {
                section.size_of_raw_data()
            } else {
                section.virtual_size()
            };

            if rva >= sec_vaddr && rva < sec_vaddr + sec_vsize {
                let offset_in_sec = rva - sec_vaddr;
                return Some((section.pointer_to_raw_data() + offset_in_sec) as usize);
            }
        }
        None
    }

    /// Returns list of imported DLL descriptors with (dll_name, list of (func_name, iat_thunk_rva)).
    pub fn imports(&self) -> Vec<(&str, Vec<(&str, u32)>)> {
        let mut import_list = Vec::new();
        let import_dir = &self.optional_header.data_directory[IMAGE_DIRECTORY_ENTRY_IMPORT];

        if import_dir.virtual_address() == 0 || import_dir.size() == 0 {
            return import_list;
        }

        if let Some(mut offset) = self.rva_to_offset(import_dir.virtual_address()) {
            loop {
                if offset + std::mem::size_of::<ImageImportDescriptor>() > self.raw_data.len() {
                    break;
                }

                let descriptor = unsafe {
                    &*(self.raw_data.as_ptr().add(offset) as *const ImageImportDescriptor)
                };
                if descriptor.is_null() {
                    break;
                }

                let name_rva = descriptor.name();
                let dll_name = if let Some(name_offset) = self.rva_to_offset(name_rva) {
                    let mut end = name_offset;
                    while end < self.raw_data.len() && self.raw_data[end] != 0 {
                        end += 1;
                    }
                    std::str::from_utf8(&self.raw_data[name_offset..end]).unwrap_or(".unknown.dll")
                } else {
                    ".unknown.dll"
                };

                let mut funcs = Vec::new();
                let lookup_rva = if descriptor.original_first_thunk() != 0 {
                    descriptor.original_first_thunk()
                } else {
                    descriptor.first_thunk()
                };

                let iat_first_rva = descriptor.first_thunk();

                if let Some(mut thunk_offset) = self.rva_to_offset(lookup_rva) {
                    let mut iat_rva_idx = iat_first_rva;
                    loop {
                        if thunk_offset + 8 > self.raw_data.len() {
                            break;
                        }

                        let thunk_val =
                            unsafe { *(self.raw_data.as_ptr().add(thunk_offset) as *const u64) };
                        if thunk_val == 0 {
                            break;
                        }

                        // Top bit 63 set = import by ordinal
                        if (thunk_val & 0x8000_0000_0000_0000) != 0 {
                            funcs.push(("ordinal_stub", iat_rva_idx));
                        } else {
                            let name_entry_rva = (thunk_val & 0x7FFF_FFFF) as u32;
                            if let Some(fn_offset) = self.rva_to_offset(name_entry_rva) {
                                // Skip 2 bytes hint
                                let fn_name_offset = fn_offset + 2;
                                let mut fn_end = fn_name_offset;
                                while fn_end < self.raw_data.len() && self.raw_data[fn_end] != 0 {
                                    fn_end += 1;
                                }
                                if let Ok(fn_name) =
                                    std::str::from_utf8(&self.raw_data[fn_name_offset..fn_end])
                                {
                                    funcs.push((fn_name, iat_rva_idx));
                                } else {
                                    funcs.push(("unknown_stub", iat_rva_idx));
                                }
                            } else {
                                funcs.push(("unknown_stub", iat_rva_idx));
                            }
                        }

                        thunk_offset += 8;
                        iat_rva_idx += 8;
                    }
                }

                import_list.push((dll_name, funcs));
                offset += std::mem::size_of::<ImageImportDescriptor>();
            }
        }

        import_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_header_rejection() {
        let dummy_data = [0u8; 128];
        assert!(PeImage::parse(&dummy_data).is_err());
    }
}
