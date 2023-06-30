// Copyright (C) 2023, Alex Badics
// This file is part of tiny-linux-usb
// Licensed under the MIT license. See LICENSE file in the project root for details.

mod descriptor;
mod ioctl;

use std::{
    cell::OnceCell,
    ffi::c_void,
    fs::{File, OpenOptions},
    io::{Read, Seek},
    os::fd::{FromRawFd, IntoRawFd},
    time::Duration,
};

use ioctl::{usbdevfs_control, ControlTransfer};

use crate::{
    descriptor::DeviceTree,
    ioctl::{
        usbdevfs_bulk, usbdevfs_claim_interface, usbdevfs_ioctl, BulkTransfer, SubIoctl,
        IOCTL_USBFS_DISCONNECT,
    },
};

#[derive(Debug, Clone)]
pub struct UsbDevice {
    fd: i32,
    descriptor_cache: OnceCell<DeviceTree>,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    DescriptorError(descriptor::Error),
    IoError(std::io::Error),
    IoctlError(nix::errno::Errno),
    InvalidEndpoint,
    DeviceDisconnected,
    NotFound,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Direction {
    Out = 0x0,
    In = 0x80,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RequestType {
    Standard = 0,
    Class = 1 << 5,
    Vendor = 2 << 5,
    Reserved = 3 << 5,
}

/// Recipients of control transfers.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Recipient {
    Device = 0,
    Interface = 1,
    Endpoint = 2,
    Other = 3,
}

impl UsbDevice {
    pub fn new(fd: impl IntoRawFd) -> Result<Self> {
        let fd = fd.into_raw_fd();
        Ok(Self {
            fd,
            descriptor_cache: OnceCell::new(),
        })
    }

    pub fn claim_interface(&self, interface: u8) -> Result<()> {
        let mut command = SubIoctl {
            ifno: interface as i32,
            ioctl_code: IOCTL_USBFS_DISCONNECT,
            data: std::ptr::null_mut(),
        };
        // No unwrap, because we don't really care if it fails
        // (e.g. no drver attached)
        let _ = unsafe { usbdevfs_ioctl(self.fd, &mut command as *mut _) };
        let mut interface = interface as u32;

        unsafe { usbdevfs_claim_interface(self.fd, &mut interface as *mut _)? };
        Ok(())
    }

    pub fn claim_endpoint(&self, endpoint_address: u8) -> Result<()> {
        let descriptors = self.descriptors()?;
        let mut interface_to_claim = None;
        'outer: for interface in &descriptors
            .configurations
            .get(0)
            .ok_or(Error::InvalidEndpoint)?
            .interfaces
        {
            for endpoint in &interface.endpoints {
                if endpoint.bEndpointAddress == endpoint_address {
                    interface_to_claim = Some(interface.desc.bInterfaceNumber);
                    break 'outer;
                }
            }
        }
        match interface_to_claim {
            Some(i) => self.claim_interface(i),
            None => Err(Error::InvalidEndpoint),
        }
    }

    pub fn descriptors(&self) -> Result<&DeviceTree> {
        if let Some(d) = self.descriptor_cache.get() {
            return Ok(d);
        }
        let mut fd_as_file = unsafe { File::from_raw_fd(self.fd) };
        let mut descriptor_data = Vec::new();
        fd_as_file.rewind()?;
        fd_as_file.read_to_end(&mut descriptor_data)?;
        // Don't close the fd
        std::mem::forget(fd_as_file);
        self.descriptor_cache
            .set(DeviceTree::from_byte_array(&descriptor_data)?)
            .unwrap();
        Ok(self.descriptor_cache.get().unwrap())
    }

    pub fn read_bulk(&self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        if endpoint & 0x80 == 0 {
            return Err(Error::InvalidEndpoint);
        }
        let mut bulk_desc = BulkTransfer {
            ep: endpoint as u32,
            len: buf.len() as u32,
            timeout: timeout.as_millis() as u32,
            data: buf.as_mut_ptr() as *mut c_void,
        };
        unsafe { usbdevfs_bulk(self.fd, &mut bulk_desc as *mut _)? };
        Ok(bulk_desc.len as usize)
    }

    pub fn write_bulk(&self, endpoint: u8, buf: &[u8], timeout: Duration) -> Result<usize> {
        if endpoint & 0x80 != 0 {
            return Err(Error::InvalidEndpoint);
        }
        let mut bulk_desc = BulkTransfer {
            ep: endpoint as u32,
            len: buf.len() as u32,
            timeout: timeout.as_millis() as u32,
            data: buf.as_ptr() as *mut c_void,
        };
        unsafe { usbdevfs_bulk(self.fd, &mut bulk_desc as *mut _)? };
        Ok(bulk_desc.len as usize)
    }
    pub fn read_interrupt(&self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        self.read_bulk(endpoint, buf, timeout)
    }

    pub fn write_interrupt(&self, endpoint: u8, buf: &[u8], timeout: Duration) -> Result<usize> {
        self.write_bulk(endpoint, buf, timeout)
    }

    pub fn read_control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize> {
        if request_type & 0x80 == 0 {
            return Err(Error::InvalidEndpoint);
        }
        let mut desc = ControlTransfer {
            request_type,
            request,
            value,
            index,
            length: buf.len() as u16,
            timeout: timeout.as_millis() as u32,
            data: buf.as_ptr() as *mut c_void,
        };
        unsafe { usbdevfs_control(self.fd, &mut desc as *mut _)? };
        Ok(desc.length as usize)
    }

    pub fn write_control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &[u8],
        timeout: Duration,
    ) -> Result<usize> {
        if request_type & 0x80 != 0 {
            return Err(Error::InvalidEndpoint);
        }
        let mut desc = ControlTransfer {
            request_type,
            request,
            value,
            index,
            length: buf.len() as u16,
            timeout: timeout.as_millis() as u32,
            data: buf.as_ptr() as *mut c_void,
        };
        unsafe { usbdevfs_control(self.fd, &mut desc as *mut _)? };
        Ok(desc.length as usize)
    }
}

pub fn request_type(direction: Direction, request_type: RequestType, recipient: Recipient) -> u8 {
    direction as u8 | request_type as u8 | recipient as u8
}

impl From<descriptor::Error> for Error {
    fn from(value: descriptor::Error) -> Self {
        Self::DescriptorError(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<nix::errno::Errno> for Error {
    fn from(value: nix::errno::Errno) -> Self {
        match value {
            nix::errno::Errno::ENODEV => Self::DeviceDisconnected,
            nix::errno::Errno::ENOENT => Self::InvalidEndpoint,
            v => Self::IoctlError(v),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::DescriptorError(e) => Some(e),
            Error::IoError(e) => Some(e),
            Error::IoctlError(e) => Some(e),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DescriptorError(e) => std::fmt::Display::fmt(&e, f),
            Error::IoError(e) => std::fmt::Display::fmt(&e, f),
            Error::IoctlError(e) => std::fmt::Display::fmt(&e, f),
            Error::DeviceDisconnected => f.write_str("Device disconnected"),
            Error::InvalidEndpoint => f.write_str("Invalid endpoint"),
            Error::NotFound => f.write_str("Not found"),
        }
    }
}

pub fn open_device_vid_pid_endpoint(vid: u16, pid: u16, endpoint_address: u8) -> Result<UsbDevice> {
    let vid_str = format!("{vid:04x}");
    let pid_str = format!("{pid:04x}");
    for device_path in std::fs::read_dir("/sys/bus/usb/devices/")? {
        let device_path = device_path?.path();
        if let (Ok(dev_vid), Ok(dev_pid), Ok(devnum), Ok(busnum)) = (
            std::fs::read(device_path.join("idVendor")),
            std::fs::read(device_path.join("idProduct")),
            std::fs::read(device_path.join("devnum")),
            std::fs::read(device_path.join("busnum")),
        ) {
            if &dev_vid[..4] == vid_str.as_bytes() && &dev_pid[..4] == pid_str.as_bytes() {
                let devnum: usize = String::from_utf8(devnum).unwrap().trim().parse().unwrap();
                let busnum: usize = String::from_utf8(busnum).unwrap().trim().parse().unwrap();
                let usb_file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(format!("/dev/bus/usb/{busnum:03}/{devnum:03}"))?;
                let usb_device = UsbDevice::new(usb_file)?;
                usb_device.claim_endpoint(endpoint_address)?;
                return Ok(usb_device);
            }
        }
    }
    Err(Error::NotFound)
}
