//! This module sets the necessary tables to pass a device tree
//! to the Linux kernel

use core::ptr::copy_nonoverlapping;

use alloc::vec::Vec;
use bitflags::bitflags;
use uefi::{
    prelude::BootServices,
    proto::unsafe_protocol,
    table::boot::{AllocateType, MemoryType},
    Handle, Result, Status, StatusExt,
};

use crate::uefi_helpers::{bytes_to_pages, UEFI_PAGE_BITS};

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
    /// Fixup flags as descrribed in https://github.com/U-Boot-EFI/EFI_DT_FIXUP_PROTOCOL
    pub struct DTFixupFlags: u32 {
        const APPLY_FIXUPS = 1 << 0;
        const RESERVE_MEMORY = 1 << 1;
    }
}

/// The device tree fixup protocol.
///
/// Device trees do not contain machine-specific information like
/// serial numbers or MAC addresses out of the box. The firmware,
/// usually U-Boot exposes this protocol to add such machine-specific
/// options.
///
/// For more information see the [u-boot
/// proposal](https://github.com/U-Boot-EFI/EFI_DT_FIXUP_PROTOCOL)
#[unsafe_protocol("e617d64c-fe08-46da-f4dc-bbd5870c7300")]
struct DTFixupProtocol {
    pub fixup: unsafe extern "efiapi" fn(
        this: *mut DTFixupProtocol,
        fdt: *mut u8,
        buffer_size: *mut usize,
        flags: DTFixupFlags,
    ) -> Status,
}

/// Fixup an fdt [`DTFixupProtocol`]
fn fixup_fdt(
    boot_services: &BootServices,
    fixup_handle: Handle,
    mut fdt_data: Vec<u8>,
) -> Result<()> {
    let mut fixup_protocol =
        boot_services.open_protocol_exclusive::<DTFixupProtocol>(fixup_handle)?;

    let mut fdt_size = fdt_data.len();

    unsafe {
        let status = (fixup_protocol.fixup)(
            &mut *fixup_protocol,
            fdt_data.as_mut_ptr(),
            &mut fdt_size as *mut usize,
            DTFixupFlags::APPLY_FIXUPS | DTFixupFlags::RESERVE_MEMORY,
        );

        if status.is_success() {
            return Ok(());
        }
        if status != Status::BUFFER_TOO_SMALL {
            return status.to_result();
        }

        // Everything is fine except our buffer is too small, make a new bigger one
        let num_pages = bytes_to_pages(fdt_size);
        fdt_size = num_pages << UEFI_PAGE_BITS;
        let base = boot_services.allocate_pages(
            AllocateType::AnyPages,
            MemoryType::ACPI_NON_VOLATILE,
            num_pages,
        )? as *mut u8;

        copy_nonoverlapping(fdt_data.as_ptr(), base, fdt_data.len());
        drop(fdt_data);

        (fixup_protocol.fixup)(
            &mut *fixup_protocol,
            base,
            &mut fdt_size as *mut usize,
            DTFixupFlags::APPLY_FIXUPS | DTFixupFlags::RESERVE_MEMORY,
        )
        .to_result()
    }
}

/// Install a device tree without fixup
fn install_fdt(_boot_services: &BootServices, _fdt_data: Vec<u8>) -> Result<()> {
    todo!()
}

/// A RAII wrapper to set and restore the device tree
///
/// **Note:** You need to call [`FdtLoader::uninstall`], before
/// this is dropped.
pub struct FdtLoader {
    handle: Handle,
    set: bool,
}

impl FdtLoader {
    /// Create a new [`FdtLoader`].
    ///
    /// `handle` is the handle where the protocols are registered
    /// on. `file` is the file that is served to Linux.
    pub fn new(boot_services: &BootServices, handle: Handle, fdt_data: Vec<u8>) -> Result<Self> {
        if let Ok(fixup_handle) = boot_services.get_handle_for_protocol::<DTFixupProtocol>() {
            fixup_fdt(boot_services, fixup_handle, fdt_data)?;
        } else {
            install_fdt(boot_services, fdt_data)?;
        };
        Ok(FdtLoader { handle, set: true })
    }

    pub fn uninstall(&mut self, _boot_services: &BootServices) -> Result<()> {
        // This should only be called once.
        assert!(self.set);
        self.set = false;
        Ok(())
    }
}

impl Drop for FdtLoader {
    fn drop(&mut self) {
        // Dropped without unregistering!
        assert!(!self.set);
    }
}
