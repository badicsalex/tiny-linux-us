// Copyright (C) 2023, Alex Badics
// This file is part of tiny-linux-usb
// Licensed under the MIT license. See LICENSE file in the project root for details.

use std::{fs::OpenOptions, time::Duration};

use usbfs_test::{open_device_vid_pid_endpoint, UsbDevice};

fn main() {
    let device = open_device_vid_pid_endpoint(0x0486, 0x573c, 0x1).unwrap();
    let mut result = [0u8; 64];
    device
        .write_bulk(
            0x01,
            b"\x02:3:5:3:88:92cd0cb2:\x03".as_slice(),
            Duration::from_millis(100),
        )
        .unwrap();
    eprintln!(
        "{:?}",
        device.read_bulk(0x81, &mut result, Duration::from_millis(100))
    );
    eprintln!("{}", String::from_utf8_lossy(&result));
}
