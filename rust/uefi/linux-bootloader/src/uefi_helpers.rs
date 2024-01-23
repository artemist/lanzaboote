use core::ffi::c_void;

use uefi::{prelude::BootServices, proto::loaded_image::LoadedImage, Result};

#[derive(Debug, Clone, Copy)]
pub struct PeInMemory {
    image_base: *const c_void,
    image_size: usize,
}

impl PeInMemory {
    /// Return a reference to the currently running image.
    ///
    /// # Safety
    ///
    /// The returned slice covers the whole loaded image in which we
    /// currently execute. This means the safety guarantees of
    /// [`core::slice::from_raw_parts`] that we use in this function
    /// are only guaranteed, if we we don't mutate anything in this
    /// range. This means no modification of global variables or
    /// anything.
    pub unsafe fn as_slice(&self) -> &'static [u8] {
        unsafe { core::slice::from_raw_parts(self.image_base as *const u8, self.image_size) }
    }
}

/// Open the currently executing image as a file.
pub fn booted_image_file(boot_services: &BootServices) -> Result<PeInMemory> {
    let loaded_image =
        boot_services.open_protocol_exclusive::<LoadedImage>(boot_services.image_handle())?;
    let (image_base, image_size) = loaded_image.info();

    Ok(PeInMemory {
        image_base,
        image_size: usize::try_from(image_size).map_err(|_| uefi::Status::INVALID_PARAMETER)?,
    })
}

/// UEFI mandates 4 KiB pages.                                                                                          
pub const UEFI_PAGE_BITS: usize = 12;
pub const UEFI_PAGE_MASK: usize = (1 << UEFI_PAGE_BITS) - 1;

/// Converts a length in bytes to the number of required pages.                                                         
pub fn bytes_to_pages(bytes: usize) -> usize {
    bytes
        .checked_add(UEFI_PAGE_MASK)
        .map(|rounded_up| rounded_up >> UEFI_PAGE_BITS)
        .unwrap_or(1 << (usize::try_from(usize::BITS).unwrap() - UEFI_PAGE_BITS))
}
