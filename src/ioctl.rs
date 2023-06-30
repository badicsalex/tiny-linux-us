// Copyright (C) 2023, Alex Badics
// This file is part of tiny-linux-usb
// Licensed under the MIT license. See LICENSE file in the project root for details.

use std::ffi::{c_int, c_uint, c_void};

use nix::{ioctl_read, ioctl_readwrite, ioctl_write_ptr, request_code_none};

#[repr(C)]
#[derive(Debug, Clone)]
pub struct ControlTransfer {
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    pub length: u16,
    pub timeout: u32,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct BulkTransfer {
    pub ep: c_uint,
    pub len: c_uint,
    pub timeout: c_uint,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct GetDriver {
    pub interface: c_uint,
    pub driver: [i8; 256],
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct SubIoctl {
    pub ifno: c_int,
    pub ioctl_code: c_int,
    pub data: *mut c_void,
}

pub const IOCTL_USBFS_DISCONNECT: c_int = request_code_none!('U', 22) as i32;
ioctl_readwrite!(usbdevfs_control, 'U', 0, ControlTransfer);
// This can do interrupts. See the kernel docs for usb_bulk_msg
ioctl_readwrite!(usbdevfs_bulk, 'U', 2, BulkTransfer);
ioctl_write_ptr!(usbdevfs_get_driver, 'U', 8, GetDriver);
ioctl_read!(usbdevfs_claim_interface, 'U', 15, c_uint);
ioctl_readwrite!(usbdevfs_ioctl, 'U', 18, SubIoctl);
