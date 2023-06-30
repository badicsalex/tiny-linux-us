// Copyright (C) 2023, Alex Badics
// This file is part of tiny-linux-usb
// Licensed under the MIT license. See LICENSE file in the project root for details.

macro_rules! check_size {
    ($t:ty, $s: literal) => {
        const _: () = assert!(std::mem::size_of::<$t>() == $s);
    };
}

#[allow(non_snake_case)]
#[repr(C)]
#[repr(packed)]
#[derive(Debug, Clone)]
pub struct DeviceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bcdUSB: u16,
    pub bDeviceClass: u8,
    pub bDeviceSubClass: u8,
    pub bDeviceProtocol: u8,
    pub bMaxPacketSize0: u8,
    pub idVendor: u16,
    pub idProduct: u16,
    pub bcdDevice: u16,
    pub iManufacturer: u8,
    pub iProduct: u8,
    pub iSerialNumber: u8,
    pub bNumConfigurations: u8,
}

check_size!(DeviceDescriptor, 18);

#[allow(non_snake_case)]
#[repr(C)]
#[repr(packed)]
#[derive(Debug, Clone)]
pub struct ConfigurationDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub wTotalLength: u16,
    pub bNumInterfaces: u8,
    pub bConfigurationValue: u8,
    pub iConfiguration: u8,
    pub bmAttributes: u8,
    pub MaxPower: u8,
}

check_size!(ConfigurationDescriptor, 9);

#[allow(non_snake_case)]
#[repr(C)]
#[repr(packed)]
#[derive(Debug, Clone)]
pub struct InterfaceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bInterfaceNumber: u8,
    pub bAlternateSetting: u8,
    pub bNumEndpoints: u8,
    pub bInterfaceClass: u8,
    pub bInterfaceSubClass: u8,
    pub bInterfaceProtocol: u8,
    pub iInterface: u8,
}
check_size!(InterfaceDescriptor, 9);

#[allow(non_snake_case)]
#[repr(C)]
#[repr(packed)]
#[derive(Debug, Clone)]
pub struct EndpointDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bEndpointAddress: u8,
    pub bmAttributes: u8,
    pub wMaxPacketSize: u16,
    pub bInterval: u8,
    // Only used for audio endpoints:
    // pub bRefresh: u8,
    // pub bSynchAddress: u8,
}
check_size!(EndpointDescriptor, 7);

#[derive(Debug, Clone)]
pub struct DeviceTree {
    pub desc: DeviceDescriptor,
    pub configurations: Vec<ConfigurationTree>,
}

#[derive(Debug, Clone)]
pub struct ConfigurationTree {
    pub desc: ConfigurationDescriptor,
    pub interfaces: Vec<InterfaceTree>,
}

#[derive(Debug, Clone)]
pub struct InterfaceTree {
    pub desc: InterfaceDescriptor,
    pub endpoints: Vec<EndpointDescriptor>,
}

impl DeviceTree {
    pub fn from_byte_array(data: &[u8]) -> Result<Self> {
        let descriptors = byte_array_to_descriptors(data)?;
        if descriptors.is_empty() {
            return Err(Error::InvalidSize);
        }
        if descriptors
            .iter()
            .filter(|d| matches!(d, AnyDescriptor::DeviceDescriptor(..)))
            .count()
            > 1
        {
            return Err(Error::TooManyDevices);
        }
        let desc = if let AnyDescriptor::DeviceDescriptor(d) = &descriptors[0] {
            d.clone()
        } else {
            return Err(Error::DeviceWasNotFirst);
        };

        Ok(Self {
            desc,
            configurations: split_by_parent_desc::<ConfigurationDescriptor>(&descriptors)
                .iter()
                .map(|(d, ds)| ConfigurationTree::from_descriptors((*d).clone(), ds))
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

impl ConfigurationTree {
    fn from_descriptors(
        desc: ConfigurationDescriptor,
        descriptors: &[AnyDescriptor],
    ) -> Result<Self> {
        Ok(Self {
            desc,
            interfaces: split_by_parent_desc::<InterfaceDescriptor>(descriptors)
                .iter()
                .map(|(d, ds)| InterfaceTree::from_descriptors((*d).clone(), ds))
                .collect::<Result<Vec<_>>>()?,
        })
    }
}
impl InterfaceTree {
    fn from_descriptors(desc: InterfaceDescriptor, descriptors: &[AnyDescriptor]) -> Result<Self> {
        Ok(Self {
            desc,
            endpoints: descriptors
                .iter()
                .filter_map(|d| {
                    let d: Option<&EndpointDescriptor> = d.try_into().ok();
                    Some(d?.clone())
                })
                .collect(),
        })
    }
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Error {
    InvalidSize,
    InvalidType,
    DeviceWasNotFirst,
    TooManyDevices,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::InvalidSize => "Invalid size field",
            Error::InvalidType => "Invalid type",
            Error::DeviceWasNotFirst => "The first descriptor was not of type Device",
            Error::TooManyDevices => "Too many device descritpors found",
        })
    }
}

macro_rules! any_descriptor {
    ($($name: tt),*) => {
        enum AnyDescriptor {
            $(
                $name($name),
            )*
            Other(u8),
        }
        $(
            impl<'a> TryFrom<&'a AnyDescriptor> for &'a $name {
                type Error = Error;

                fn try_from(value: &'a AnyDescriptor) -> std::result::Result<Self, Self::Error> {
                    if let AnyDescriptor::$name(v) = value {
                        Ok(v)
                    } else {
                        Err(Error::InvalidType)
                    }
                }
            }
        )*
    };
}

any_descriptor!(
    DeviceDescriptor,
    ConfigurationDescriptor,
    InterfaceDescriptor,
    EndpointDescriptor
);

fn split_by_parent_desc<'a, T>(descriptors: &'a [AnyDescriptor]) -> Vec<(&T, &'a [AnyDescriptor])>
where
    &'a T: std::convert::TryFrom<&'a AnyDescriptor>,
{
    let mut result = Vec::new();
    let split_points: Vec<(usize, &T)> = descriptors
        .iter()
        .enumerate()
        .filter_map(|(i, d)| Some((i, d.try_into().ok()?)))
        .collect();
    for (spi, (sp, d)) in split_points.iter().enumerate() {
        result.push((
            *d,
            if spi < split_points.len() - 1 {
                &descriptors[*sp..split_points[spi + 1].0]
            } else {
                &descriptors[*sp..]
            },
        ));
    }
    result
}

fn parse_descriptor<T>(data: &[u8]) -> Result<T> {
    if data.len() < std::mem::size_of::<T>() {
        return Err(Error::InvalidSize);
    }
    Ok(unsafe { std::ptr::read(data.as_ptr() as *const T) })
}

fn byte_array_to_descriptors(mut data: &[u8]) -> Result<Vec<AnyDescriptor>> {
    let mut result = Vec::new();
    while data.len() >= 2 {
        let l = data[0] as usize;
        if l < 2 || l > data.len() {
            return Err(Error::InvalidSize);
        }
        let descriptor_data = &data[..l];
        result.push(match descriptor_data[1] {
            1 => AnyDescriptor::DeviceDescriptor(parse_descriptor(descriptor_data)?),
            2 => AnyDescriptor::ConfigurationDescriptor(parse_descriptor(descriptor_data)?),
            4 => AnyDescriptor::InterfaceDescriptor(parse_descriptor(descriptor_data)?),
            5 => AnyDescriptor::EndpointDescriptor(parse_descriptor(descriptor_data)?),
            o => AnyDescriptor::Other(o),
        });
        data = &data[l..]
    }
    Ok(result)
}
