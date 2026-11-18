use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use anyhow::Result;
use sysinfo::System;

use crate::color::Color;
use crate::device::LenovoKeyboard;

#[derive(Debug, Clone, PartialEq)]
pub enum AnimationEffect {
    RainbowWave,
    Breathing(Color),
    ColorCycle(Vec<Color>),
    CpuMonitor,
    RamMonitor,
    Pulse(Color),
    Fire,
    Police,
    Matrix,
}

impl AnimationEffect {
    pub fn from_name(name: &str, color: Option<Color>) -> Option<Self> {
        let base_col = color.unwrap_or(Color::LEGION_CYAN);
        match name.to_lowercase().trim() {
            "rainbow-wave" | "rainbow" | "wave" => Some(Self::RainbowWave),
            "breathing" | "breath" => Some(Self::Breathing(base_col)),
            "color-cycle" | "cycle" => Some(Self::ColorCycle(vec![
                Color::RED,
                Color::ORANGE,
                Color::YELLOW,
                Color::GREEN,
                Color::LEGION_CYAN,
                Color::BLUE,
                Color::PURPLE,
                Color::MAGENTA,
            ])),
            "cpu" | "cpu-monitor" | "cpu-load" => Some(Self::CpuMonitor),
            "ram" | "ram-monitor" | "memory" => Some(Self::RamMonitor),
            "pulse" | "heartbeat" => Some(Self::Pulse(base_col)),
            "fire" | "flame" | "campfire" => Some(Self::Fire),
            "police" | "emergency" | "siren" => Some(Self::Police),
            "matrix" | "digital-rain" | "hacker" => Some(Self::Matrix),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::RainbowWave => "Rainbow Wave",
            Self::Breathing(_) => "Smooth Breathing",
            Self::ColorCycle(_) => "Color Cycle",
            Self::CpuMonitor => "CPU Load Monitor",
            Self::RamMonitor => "RAM Usage Monitor",
            Self::Pulse(_) => "Heartbeat Pulse",
            Self::Fire => "Campfire",
            Self::Police => "Police Siren",
            Self::Matrix => "Matrix Rain",
        }
    }

    /// Calculate the 4-zone colors for a given timestamp and animation state
    pub fn get_frame(&self, elapsed_secs: f32, speed_factor: f32, sys: &mut Option<System>) -> [Color; 4] {
        let t = elapsed_secs * speed_factor;

        match self {
            Self::RainbowWave => {
                let mut zones = [Color::BLACK; 4];
                for i in 0..4 {
                    let hue = (t * 120.0 + (i as f32) * 50.0) % 360.0;
                    zones[i] = Color::from_hsv(hue, 1.0, 1.0);
                }
                zones
            }

            Self::Breathing(col) => {
                // Smooth sinusoidal breathing with gamma adjustment
                let sin_val = (t * std::f32::consts::PI).sin();
                let intensity = (sin_val.abs()).powf(1.8).clamp(0.05, 1.0);
                [col.scale(intensity); 4]
            }

            Self::ColorCycle(colors) => {
                if colors.is_empty() {
                    return [Color::LEGION_CYAN; 4];
                }
                let len = colors.len();
                let pos = (t * 0.8) % (len as f32);
                let idx1 = pos.floor() as usize % len;
                let idx2 = (idx1 + 1) % len;
                let frac = pos - pos.floor();

                let col = colors[idx1].lerp(colors[idx2], frac);
                [col; 4]
            }

            Self::CpuMonitor => {
                if let Some(system) = sys {
                    system.refresh_cpu_all();
                    let cpu_usage = system.global_cpu_usage(); // 0.0 to 100.0

                    // Color zones based on overall CPU load:
                    // 0..30% : Cyan to Green (Cool)
                    // 30..65%: Green to Yellow (Moderate)
                    // 65..85%: Yellow to Orange (High)
                    // 85..100%: Orange to Red (Max)
                    let col = if cpu_usage < 30.0 {
                        let frac = cpu_usage / 30.0;
                        Color::LEGION_CYAN.lerp(Color::GREEN, frac)
                    } else if cpu_usage < 65.0 {
                        let frac = (cpu_usage - 30.0) / 35.0;
                        Color::GREEN.lerp(Color::YELLOW, frac)
                    } else if cpu_usage < 85.0 {
                        let frac = (cpu_usage - 65.0) / 20.0;
                        Color::YELLOW.lerp(Color::ORANGE, frac)
                    } else {
                        let frac = ((cpu_usage - 85.0) / 15.0).clamp(0.0, 1.0);
                        Color::ORANGE.lerp(Color::RED, frac)
                    };

                    // Fill zones bar-graph style:
                    // Zone 0: active if > 5%
                    // Zone 1: active if > 25%
                    // Zone 2: active if > 50%
                    // Zone 3: active if > 75%
                    let mut zones = [Color::BLACK; 4];
                    let thresholds = [5.0, 25.0, 50.0, 75.0];
                    for i in 0..4 {
                        if cpu_usage >= thresholds[i] {
                            let ratio = ((cpu_usage - thresholds[i]) / 25.0).clamp(0.3, 1.0);
                            zones[i] = col.scale(ratio);
                        } else {
                            zones[i] = col.scale(0.08); // faint dim indicator
                        }
                    }
                    zones
                } else {
                    [Color::GREEN; 4]
                }
            }

            Self::RamMonitor => {
                if let Some(system) = sys {
                    system.refresh_memory();
                    let total = system.total_memory() as f32;
                    let used = system.used_memory() as f32;
                    let ram_usage = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };

                    let col = if ram_usage < 50.0 {
                        Color::ICE_BLUE.lerp(Color::MINT, ram_usage / 50.0)
                    } else if ram_usage < 80.0 {
                        Color::MINT.lerp(Color::GOLD, (ram_usage - 50.0) / 30.0)
                    } else {
                        Color::GOLD.lerp(Color::CRIMSON, ((ram_usage - 80.0) / 20.0).clamp(0.0, 1.0))
                    };

                    let mut zones = [Color::BLACK; 4];
                    let thresholds = [15.0, 40.0, 65.0, 85.0];
                    for i in 0..4 {
                        if ram_usage >= thresholds[i] {
                            zones[i] = col;
                        } else {
                            zones[i] = col.scale(0.06);
                        }
                    }
                    zones
                } else {
                    [Color::ICE_BLUE; 4]
                }
            }

            Self::Pulse(col) => {
                let p = (t * 2.5) % 2.0;
                let intensity = if p < 0.25 {
                    (p / 0.25).powf(0.5)
                } else if p < 0.5 {
                    1.0 - ((p - 0.25) / 0.25)
                } else if p < 0.75 {
                    ((p - 0.5) / 0.25).powf(0.5) * 0.7
                } else if p < 1.0 {
                    0.7 - ((p - 0.75) / 0.25) * 0.7
                } else {
                    0.0
                };
                let scaled = col.scale(intensity.clamp(0.04, 1.0));
                [scaled; 4]
            }

            Self::Fire => {
                // Procedural campfire flicker
                let mut zones = [Color::BLACK; 4];
                let offsets = [0.0, 1.3, 2.7, 4.1];
                for i in 0..4 {
                    let phase = t * 6.0 + offsets[i];
                    let noise = (phase.sin() * 0.5 + 0.5) * 0.7 + ((phase * 2.3).sin() * 0.5 + 0.5) * 0.3;
                    let r = 255;
                    let g = (noise * 140.0) as u8;
                    let b = (noise * 25.0) as u8;
                    zones[i] = Color::new(r, g, b);
                }
                zones
            }

            Self::Police => {
                let step = (t * 5.0) as u32 % 4;
                match step {
                    0 => [Color::RED, Color::RED, Color::BLACK, Color::BLACK],
                    1 => [Color::BLACK, Color::BLACK, Color::BLACK, Color::BLACK],
                    2 => [Color::BLACK, Color::BLACK, Color::BLUE, Color::BLUE],
                    _ => [Color::BLACK, Color::BLACK, Color::BLACK, Color::BLACK],
                }
            }

            Self::Matrix => {
                let phase = (t * 4.0) as usize % 4;
                let mut zones = [Color::BLACK; 4];
                for i in 0..4 {
                    let dist = (i + 4 - phase) % 4;
                    let intensity = match dist {
                        0 => 1.0,
                        1 => 0.5,
                        2 => 0.2,
                        _ => 0.03,
                    };
                    zones[i] = Color::new(0, (255.0 * intensity) as u8, (70.0 * intensity) as u8);
                }
                zones
            }
        }
    }
}

/// Run an animation loop until stop_flag is true
pub fn run_animation_loop(
    keyboard: &mut LenovoKeyboard,
    effect: &AnimationEffect,
    speed_factor: f32,
    brightness: u8,
    fps: u64,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let frame_duration = Duration::from_millis(1000 / fps.max(1));
    let start_time = Instant::now();
    let mut sys = if matches!(effect, AnimationEffect::CpuMonitor | AnimationEffect::RamMonitor) {
        Some(System::new_all())
    } else {
        None
    };

    while !stop_flag.load(Ordering::Relaxed) {
        let frame_start = Instant::now();
        let elapsed = start_time.elapsed().as_secs_f32();

        let zones = effect.get_frame(elapsed, speed_factor, &mut sys);
        let _ = keyboard.set_static(&zones, brightness);

        let work_time = frame_start.elapsed();
        if work_time < frame_duration {
            thread::sleep(frame_duration - work_time);
        }
    }

    Ok(())
}
