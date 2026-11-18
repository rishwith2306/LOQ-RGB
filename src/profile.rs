use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::device::{HardwareEffect, LenovoKeyboard};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProfileMode {
    Static,
    Breath,
    Wave,
    Hue,
    RainbowWave,
    CpuMonitor,
    RamMonitor,
    Fire,
    Police,
    Matrix,
}

impl ProfileMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Static => "Static",
            Self::Breath => "Breath (Hardware)",
            Self::Wave => "Wave (Hardware)",
            Self::Hue => "Hue Cycle (Hardware)",
            Self::RainbowWave => "Rainbow Wave (Software)",
            Self::CpuMonitor => "CPU Monitor (Software)",
            Self::RamMonitor => "RAM Monitor (Software)",
            Self::Fire => "Campfire (Software)",
            Self::Police => "Police Siren (Software)",
            Self::Matrix => "Matrix Rain (Software)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingProfile {
    pub name: String,
    pub description: String,
    pub mode: ProfileMode,
    pub brightness: u8, // 0 = off, 1 = low, 2 = high
    pub speed: u8,      // 1..4
    pub zones: [Color; 4],
}

impl LightingProfile {
    pub fn new_static(name: impl Into<String>, desc: impl Into<String>, zones: [Color; 4], brightness: u8) -> Self {
        Self {
            name: name.into(),
            description: desc.into(),
            mode: ProfileMode::Static,
            brightness,
            speed: 1,
            zones,
        }
    }

    pub fn new_hardware_effect(
        name: impl Into<String>,
        desc: impl Into<String>,
        mode: ProfileMode,
        speed: u8,
        brightness: u8,
        zones: [Color; 4],
    ) -> Self {
        Self {
            name: name.into(),
            description: desc.into(),
            mode,
            brightness,
            speed: speed.clamp(1, 4),
            zones,
        }
    }

    pub fn apply(&self, keyboard: &mut LenovoKeyboard) -> Result<()> {
        match self.mode {
            ProfileMode::Static => keyboard.set_static(&self.zones, self.brightness),
            ProfileMode::Breath => {
                keyboard.set_hardware_effect(HardwareEffect::Breath, self.speed, self.brightness, &self.zones)
            }
            ProfileMode::Wave => {
                keyboard.set_hardware_effect(HardwareEffect::Wave, self.speed, self.brightness, &self.zones)
            }
            ProfileMode::Hue => {
                keyboard.set_hardware_effect(HardwareEffect::Hue, self.speed, self.brightness, &self.zones)
            }
            // For software effects, apply initial static frame
            _ => keyboard.set_static(&self.zones, self.brightness),
        }
    }
}

pub fn get_config_dir() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("com", "lenovo-rgb", "lenovo-rgb") {
        proj.config_dir().to_path_buf()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("lenovo-rgb")
    }
}

pub fn get_profiles_file() -> PathBuf {
    get_config_dir().join("profiles.json")
}

pub fn default_profiles() -> Vec<LightingProfile> {
    vec![
        LightingProfile::new_static(
            "vantage-cyan",
            "Signature Lenovo Legion Vantage Ice Cyan",
            [Color::LEGION_CYAN; 4],
            2,
        ),
        LightingProfile::new_hardware_effect(
            "hardware-wave",
            "Hardware RGB Wave across all zones",
            ProfileMode::Wave,
            3,
            2,
            [Color::LEGION_CYAN; 4],
        ),
        LightingProfile::new_hardware_effect(
            "hardware-breath",
            "Hardware Breathing effect in Legion Cyan",
            ProfileMode::Breath,
            2,
            2,
            [Color::LEGION_CYAN; 4],
        ),
        LightingProfile::new_hardware_effect(
            "hardware-hue",
            "Hardware Color Spectrum Cycle",
            ProfileMode::Hue,
            2,
            2,
            [Color::LEGION_CYAN; 4],
        ),
        LightingProfile::new_static(
            "cyberpunk",
            "Cyberpunk Night City Neon Colors",
            [Color::YELLOW, Color::LEGION_CYAN, Color::MAGENTA, Color::PURPLE],
            2,
        ),
        LightingProfile::new_static(
            "gaming-fire",
            "Warm Aggressive Red & Amber Fire",
            [Color::RED, Color::ORANGE, Color::GOLD, Color::CRIMSON],
            2,
        ),
        LightingProfile::new_static(
            "stealth",
            "Soft Warm White Backlight (Low brightness)",
            [Color::new(255, 230, 180); 4],
            1,
        ),
        LightingProfile::new_static(
            "deep-space",
            "Cool Cosmic Blues & Violets",
            [Color::BLUE, Color::ICE_BLUE, Color::PURPLE, Color::MAGENTA],
            2,
        ),
        LightingProfile::new_static(
            "matrix",
            "Terminal Hacker Acid & Mint Greens",
            [Color::ACID_GREEN, Color::GREEN, Color::MINT, Color::GREEN],
            2,
        ),
        LightingProfile {
            name: "cpu-monitor".to_string(),
            description: "Live CPU Load Indicator (Software)".to_string(),
            mode: ProfileMode::CpuMonitor,
            brightness: 2,
            speed: 1,
            zones: [Color::GREEN; 4],
        },
    ]
}

pub fn load_profiles() -> Result<Vec<LightingProfile>> {
    let path = get_profiles_file();
    if !path.exists() {
        return Ok(default_profiles());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read profiles file at {:?}", path))?;
    let mut custom: Vec<LightingProfile> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse profiles file at {:?}", path))?;

    // Merge default profiles that aren't overwritten by custom ones
    let defaults = default_profiles();
    for def in defaults {
        if !custom.iter().any(|c| c.name.eq_ignore_ascii_case(&def.name)) {
            custom.push(def);
        }
    }

    Ok(custom)
}

pub fn save_profile(profile: LightingProfile) -> Result<()> {
    let mut profiles = load_profiles().unwrap_or_else(|_| default_profiles());

    // Replace if exists, else append
    if let Some(pos) = profiles.iter().position(|p| p.name.eq_ignore_ascii_case(&profile.name)) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }

    let config_dir = get_config_dir();
    fs::create_dir_all(&config_dir)?;

    let path = get_profiles_file();
    let json = serde_json::to_string_pretty(&profiles)?;
    fs::write(&path, json)?;
    Ok(())
}

pub fn delete_profile(name: &str) -> Result<bool> {
    let mut profiles = load_profiles()?;
    let original_len = profiles.len();
    profiles.retain(|p| !p.name.eq_ignore_ascii_case(name));

    if profiles.len() == original_len {
        return Ok(false);
    }

    let config_dir = get_config_dir();
    fs::create_dir_all(&config_dir)?;

    let path = get_profiles_file();
    let json = serde_json::to_string_pretty(&profiles)?;
    fs::write(&path, json)?;
    Ok(true)
}

pub fn find_profile(name: &str) -> Result<Option<LightingProfile>> {
    let profiles = load_profiles()?;
    Ok(profiles.into_iter().find(|p| p.name.eq_ignore_ascii_case(name)))
}
