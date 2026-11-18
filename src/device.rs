use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Context, Result};
use crate::color::Color;

pub const VENDOR_ID: u16 = 0x048D;
pub const KNOWN_PIDS: &[u16] = &[
    0xC995, 0xC994, 0xC993,
    0xC985, 0xC984, 0xC983,
    0xC975, 0xC973,
    0xC965, 0xC963,
    0xC955,
];

pub const REPORT_ID: u8 = 0xCC;
pub const CMD_LIGHTING: u8 = 0x16;
pub const PAYLOAD_LEN: usize = 33;

/// Linux HIDIOCSFEATURE(33) ioctl number on x86_64:
/// _IOC(_IOC_READ|_IOC_WRITE, 'H', 0x06, 33) = 0xc0214806
const IOC_HIDIOCSFEATURE_33: libc::c_ulong = 0xc0214806;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareEffect {
    Static = 0x01,
    Breath = 0x03,
    Wave = 0x04,
    Hue = 0x06,
}

impl HardwareEffect {
    pub fn code(&self) -> u8 {
        *self as u8
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().trim() {
            "static" | "solid" | "fixed" => Some(Self::Static),
            "breath" | "breathing" | "pulse" => Some(Self::Breath),
            "wave" | "rainbow-wave" => Some(Self::Wave),
            "hue" | "cycle" | "spectrum" | "rainbow" => Some(Self::Hue),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Static => "Static",
            Self::Breath => "Breath",
            Self::Wave => "Wave",
            Self::Hue => "Hue",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub path: PathBuf,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
    pub is_lighting_interface: bool,
}

pub struct LenovoKeyboard {
    file: File,
    path: PathBuf,
    info: DeviceInfo,
    current_brightness: u8,
}

impl LenovoKeyboard {
    /// Automatically find and open the Lenovo RGB keyboard lighting interface
    pub fn open_auto() -> Result<Self> {
        let candidates = find_lenovo_devices()?;
        if candidates.is_empty() {
            return Err(anyhow!(
                "No Lenovo RGB keyboard found in /dev/hidraw*.\n\
                 Ensure your laptop has a supported 4-zone RGB keyboard (ITE 8295 controller),\n\
                 and that the udev rule is installed (run: lenovo-rgb setup-udev)."
            ));
        }

        // Prefer lighting interface match first
        let best = candidates
            .iter()
            .find(|d| d.is_lighting_interface)
            .or_else(|| candidates.first())
            .unwrap();

        Self::open_path(&best.path)
    }

    /// Open a specific device path (e.g. /dev/hidraw2)
    pub fn open_path(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .with_context(|| {
                format!(
                    "Failed to open '{path:?}'. Make sure you have read/write permissions.\n\
                     You can install the udev rule with: lenovo-rgb setup-udev"
                )
            })?;

        let info = inspect_hidraw_device(path).unwrap_or_else(|_| DeviceInfo {
            path: path.to_path_buf(),
            vendor_id: VENDOR_ID,
            product_id: 0xC993,
            name: "Lenovo ITE Device".to_string(),
            is_lighting_interface: true,
        });

        Ok(Self {
            file,
            path: path.to_path_buf(),
            info,
            current_brightness: 2,
        })
    }

    pub fn device_info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Send raw 33-byte packet to the keyboard
    pub fn send_raw_packet(&mut self, packet: &[u8; PAYLOAD_LEN]) -> Result<()> {
        let fd = self.file.as_raw_fd();
        let mut buf = *packet;

        // Perform the HIDIOCSFEATURE ioctl
        let res = unsafe { libc::ioctl(fd, IOC_HIDIOCSFEATURE_33, buf.as_mut_ptr()) };
        if res < 0 {
            let err = std::io::Error::last_os_error();
            return Err(anyhow!("HIDIOCSFEATURE ioctl failed on {:?}: {}", self.path, err));
        }
        Ok(())
    }

    /// Build and send a 33-byte lighting feature report
    pub fn send_lighting_command(
        &mut self,
        effect: u8,
        speed: u8,
        brightness: u8,
        zones: &[Color; 4],
    ) -> Result<()> {
        let mut packet = [0u8; PAYLOAD_LEN];
        packet[0] = REPORT_ID;
        packet[1] = CMD_LIGHTING;
        packet[2] = effect;
        packet[3] = speed.clamp(1, 4);
        packet[4] = brightness.clamp(0, 2);

        for (i, col) in zones.iter().enumerate().take(4) {
            let base = 5 + i * 3;
            packet[base] = col.r;
            packet[base + 1] = col.g;
            packet[base + 2] = col.b;
        }

        self.current_brightness = brightness;
        self.send_raw_packet(&packet)
    }

    /// Set static colors across the 4 zones
    pub fn set_static(&mut self, zones: &[Color; 4], brightness: u8) -> Result<()> {
        self.send_lighting_command(HardwareEffect::Static.code(), 1, brightness, zones)
    }

    /// Set a single color on all 4 zones
    pub fn set_all(&mut self, color: Color, brightness: u8) -> Result<()> {
        let zones = [color; 4];
        self.set_static(&zones, brightness)
    }

    /// Set hardware animation effect (Static, Breath, Wave, Hue)
    pub fn set_hardware_effect(
        &mut self,
        effect: HardwareEffect,
        speed: u8,
        brightness: u8,
        zones: &[Color; 4],
    ) -> Result<()> {
        self.send_lighting_command(effect.code(), speed, brightness, zones)
    }

    /// Set hardware brightness (0 = off, 1 = low, 2 = high)
    pub fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        let zones = [Color::LEGION_CYAN; 4];
        self.send_lighting_command(HardwareEffect::Static.code(), 1, brightness, &zones)
    }

    /// Turn off keyboard backlight
    pub fn turn_off(&mut self) -> Result<()> {
        let black = [Color::BLACK; 4];
        self.send_lighting_command(HardwareEffect::Static.code(), 1, 0, &black)
    }
}

/// Inspect a specific hidraw device node (/dev/hidrawX)
pub fn inspect_hidraw_device(dev_path: &Path) -> Result<DeviceInfo> {
    let file_name = dev_path
        .file_name()
        .ok_or_else(|| anyhow!("Invalid path"))?
        .to_string_lossy();

    let sys_path = PathBuf::from(format!("/sys/class/hidraw/{}/device", file_name));
    if !sys_path.exists() {
        return Err(anyhow!("Sysfs path {:?} does not exist", sys_path));
    }

    let uevent_path = sys_path.join("uevent");
    let uevent_content = fs::read_to_string(&uevent_path).unwrap_or_default();

    let mut vendor_id = 0u16;
    let mut product_id = 0u16;
    let mut name = String::new();

    for line in uevent_content.lines() {
        if let Some(val) = line.strip_prefix("HID_ID=") {
            // Format: 0003:0000048D:0000C993
            let parts: Vec<&str> = val.split(':').collect();
            if parts.len() >= 3 {
                vendor_id = u16::from_str_radix(parts[1].trim_start_matches('0'), 16).unwrap_or(0);
                product_id = u16::from_str_radix(parts[2].trim_start_matches('0'), 16).unwrap_or(0);
            }
        } else if let Some(val) = line.strip_prefix("HID_NAME=") {
            name = val.to_string();
        }
    }

    let mut is_lighting_interface = false;
    let descriptor_path = sys_path.join("report_descriptor");
    if let Ok(desc) = fs::read(&descriptor_path) {
        // Look for Vendor Usage Page 0xFF89 (0x06, 0x89, 0xFF) and Usage 0x00CC (0x09, 0xCC)
        // or Report ID 0xCC (0x85, 0xCC)
        if desc.windows(3).any(|w| w == [0x06, 0x89, 0xFF])
            && desc.windows(2).any(|w| w == [0x85, 0xCC] || w == [0x09, 0xCC])
        {
            is_lighting_interface = true;
        }
    }

    Ok(DeviceInfo {
        path: dev_path.to_path_buf(),
        vendor_id,
        product_id,
        name,
        is_lighting_interface,
    })
}

/// Enumerate and find all Lenovo candidate devices in `/sys/class/hidraw`
pub fn find_lenovo_devices() -> Result<Vec<DeviceInfo>> {
    let mut devices = Vec::new();
    let hidraw_dir = Path::new("/sys/class/hidraw");
    if !hidraw_dir.exists() {
        return Ok(devices);
    }

    for entry in fs::read_dir(hidraw_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let dev_path = PathBuf::from(format!("/dev/{}", name.to_string_lossy()));

        if let Ok(info) = inspect_hidraw_device(&dev_path) {
            if info.vendor_id == VENDOR_ID || KNOWN_PIDS.contains(&info.product_id) {
                devices.push(info);
            }
        }
    }

    // Sort so lighting interfaces come first
    devices.sort_by_key(|d| !d.is_lighting_interface);
    Ok(devices)
}
