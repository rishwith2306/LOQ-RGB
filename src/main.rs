use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use colored::Colorize;

use lenovo_rgb::color::Color;
use lenovo_rgb::device::{find_lenovo_devices, HardwareEffect, LenovoKeyboard};
use lenovo_rgb::effects::{run_animation_loop, AnimationEffect};
use lenovo_rgb::profile::{
    delete_profile, find_profile, load_profiles, save_profile, LightingProfile,
};
use lenovo_rgb::service::{install_systemd_service, setup_udev_rules, uninstall_systemd_service};
use lenovo_rgb::tui::run_tui;

#[derive(Parser, Debug)]
#[command(
    name = "lenovo-rgb",
    author = "Ceaser",
    version,
    about = "Control Lenovo Legion / LOQ 4-Zone RGB Keyboard Backlight",
    long_about = "A native Rust replacement for Lenovo Vantage keyboard lighting control on Linux.\n\
                  Supports 4-zone static coloring, hardware effects (wave, breath, hue), dynamic software animations\n\
                  (CPU monitor, campfire, matrix, rainbow wave), profiles, and an interactive TUI."
)]
struct Cli {
    /// Explicit device path (e.g. /dev/hidraw2). Autodetected if omitted.
    #[arg(short, long, global = true)]
    device: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch the interactive terminal UI (default when run without arguments in a terminal)
    Tui,

    /// Set all 4 zones to a single color
    SetAll {
        /// Color (#RRGGBB, R,G,B, or name like 'cyan', 'red', 'white')
        color: Color,

        /// Hardware brightness level (0 = off, 1 = low, 2 = high)
        #[arg(short, long, default_value_t = 2)]
        brightness: u8,
    },

    /// Set individual colors for each zone
    Set {
        /// Color for Zone 1 (Left / Esc column)
        #[arg(long, short = '1')]
        z1: Option<Color>,

        /// Color for Zone 2 (Middle-Left)
        #[arg(long, short = '2')]
        z2: Option<Color>,

        /// Color for Zone 3 (Middle-Right)
        #[arg(long, short = '3')]
        z3: Option<Color>,

        /// Color for Zone 4 (Right / Numpad / Nav)
        #[arg(long, short = '4')]
        z4: Option<Color>,

        /// Color for all zones (overridden by specific zone flags)
        #[arg(long, short = 'a')]
        all: Option<Color>,

        /// Hardware brightness level (0 = off, 1 = low, 2 = high)
        #[arg(short, long, default_value_t = 2)]
        brightness: u8,
    },

    /// Trigger a built-in hardware lighting effect (executed on the laptop EC)
    Effect {
        /// Effect name: static, breath, wave, hue
        #[arg(value_enum)]
        effect: CliHardwareEffect,

        /// Effect animation speed (1 = slow, 4 = fast)
        #[arg(short, long, default_value_t = 2)]
        speed: u8,

        /// Hardware brightness level (0 = off, 1 = low, 2 = high)
        #[arg(short, long, default_value_t = 2)]
        brightness: u8,

        /// Optional base color for breath or static mode
        #[arg(short, long)]
        color: Option<Color>,
    },

    /// Run a dynamic software animation engine
    Animate {
        /// Animation type
        #[arg(value_enum)]
        animation: CliAnimation,

        /// Speed multiplier (default: 1.0)
        #[arg(short, long, default_value_t = 1.0)]
        speed: f32,

        /// Hardware brightness level (0 = off, 1 = low, 2 = high)
        #[arg(short, long, default_value_t = 2)]
        brightness: u8,

        /// Target frames per second (default: 30)
        #[arg(long, default_value_t = 30)]
        fps: u64,

        /// Primary color (for breathing, pulse, etc.)
        #[arg(short, long)]
        color: Option<Color>,
    },

    /// Set hardware brightness level
    Brightness {
        /// Level: 0 (off), 1 (low), 2 (high)
        level: u8,
    },

    /// Turn off keyboard backlight completely
    Off,

    /// Manage lighting profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Check hardware device connection, permissions, and diagnostic status
    Status,

    /// Install udev rules (/etc/udev/rules.d/99-lenovo-rgb.rules) for unprivileged access
    SetupUdev,

    /// Manage systemd user service to restore lighting profile on login
    Systemd {
        #[command(subcommand)]
        action: SystemdAction,
    },

    /// Generate shell auto-completion script
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum CliHardwareEffect {
    Static,
    Breath,
    Wave,
    Hue,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum CliAnimation {
    RainbowWave,
    Breathing,
    ColorCycle,
    CpuMonitor,
    RamMonitor,
    Pulse,
    Fire,
    Police,
    Matrix,
}

#[derive(Subcommand, Debug)]
enum ProfileAction {
    /// List all available profiles
    List,

    /// Apply a saved lighting profile
    Apply {
        /// Name of profile to apply
        name: String,
    },

    /// Save current or specified lighting configuration as a named profile
    Save {
        /// Profile name
        name: String,

        /// Optional description
        #[arg(short, long)]
        description: Option<String>,

        /// Color for Zone 1
        #[arg(long)]
        z1: Option<Color>,

        /// Color for Zone 2
        #[arg(long)]
        z2: Option<Color>,

        /// Color for Zone 3
        #[arg(long)]
        z3: Option<Color>,

        /// Color for Zone 4
        #[arg(long)]
        z4: Option<Color>,

        /// Color for all zones
        #[arg(long, short = 'a')]
        all: Option<Color>,

        /// Brightness level (0..2)
        #[arg(short, long, default_value_t = 2)]
        brightness: u8,
    },

    /// Delete a saved profile
    Delete {
        /// Profile name
        name: String,
    },

    /// Show details of a profile
    Show {
        /// Profile name
        name: String,
    },
}

#[derive(Subcommand, Debug)]
enum SystemdAction {
    /// Install and enable systemd user service for a profile
    Install {
        /// Name of profile to apply at login
        #[arg(default_value = "vantage-cyan")]
        profile: String,
    },

    /// Disable and uninstall systemd user service
    Uninstall,
}

fn open_keyboard(custom_path: Option<&PathBuf>) -> Result<LenovoKeyboard> {
    if let Some(path) = custom_path {
        LenovoKeyboard::open_path(path)
    } else {
        LenovoKeyboard::open_auto()
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Default to TUI if executed in a terminal without subcommands
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            if std::io::stdout().is_terminal() {
                Commands::Tui
            } else {
                Cli::command().print_help()?;
                println!();
                return Ok(());
            }
        }
    };

    match command {
        Commands::Tui => {
            let kb = open_keyboard(cli.device.as_ref())?;
            run_tui(kb)?;
        }

        Commands::SetAll { color, brightness } => {
            let mut kb = open_keyboard(cli.device.as_ref())?;
            kb.set_all(color, brightness)?;
            println!(
                "{} Applied color {} to all zones at brightness {}",
                "✓".green().bold(),
                color.to_hex().cyan().bold(),
                brightness
            );
        }

        Commands::Set {
            z1,
            z2,
            z3,
            z4,
            all,
            brightness,
        } => {
            let mut kb = open_keyboard(cli.device.as_ref())?;
            let fallback = all.unwrap_or(Color::LEGION_CYAN);
            let zones = [
                z1.or(all).unwrap_or(fallback),
                z2.or(all).unwrap_or(fallback),
                z3.or(all).unwrap_or(fallback),
                z4.or(all).unwrap_or(fallback),
            ];
            kb.set_static(&zones, brightness)?;
            println!(
                "{} Applied zone colors: [Z1: {}, Z2: {}, Z3: {}, Z4: {}] (brightness {})",
                "✓".green().bold(),
                zones[0].to_hex().cyan(),
                zones[1].to_hex().cyan(),
                zones[2].to_hex().cyan(),
                zones[3].to_hex().cyan(),
                brightness
            );
        }

        Commands::Effect {
            effect,
            speed,
            brightness,
            color,
        } => {
            let mut kb = open_keyboard(cli.device.as_ref())?;
            let hw_eff = match effect {
                CliHardwareEffect::Static => HardwareEffect::Static,
                CliHardwareEffect::Breath => HardwareEffect::Breath,
                CliHardwareEffect::Wave => HardwareEffect::Wave,
                CliHardwareEffect::Hue => HardwareEffect::Hue,
            };
            let col = color.unwrap_or(Color::LEGION_CYAN);
            let zones = [col; 4];
            kb.set_hardware_effect(hw_eff, speed, brightness, &zones)?;
            println!(
                "{} Triggered hardware effect {} (speed {}, brightness {})",
                "✓".green().bold(),
                hw_eff.name().yellow().bold(),
                speed,
                brightness
            );
        }

        Commands::Animate {
            animation,
            speed,
            brightness,
            fps,
            color,
        } => {
            let mut kb = open_keyboard(cli.device.as_ref())?;
            let col = color.unwrap_or(Color::LEGION_CYAN);
            let anim = match animation {
                CliAnimation::RainbowWave => AnimationEffect::RainbowWave,
                CliAnimation::Breathing => AnimationEffect::Breathing(col),
                CliAnimation::ColorCycle => AnimationEffect::ColorCycle(vec![
                    Color::RED,
                    Color::ORANGE,
                    Color::YELLOW,
                    Color::GREEN,
                    Color::LEGION_CYAN,
                    Color::BLUE,
                    Color::PURPLE,
                ]),
                CliAnimation::CpuMonitor => AnimationEffect::CpuMonitor,
                CliAnimation::RamMonitor => AnimationEffect::RamMonitor,
                CliAnimation::Pulse => AnimationEffect::Pulse(col),
                CliAnimation::Fire => AnimationEffect::Fire,
                CliAnimation::Police => AnimationEffect::Police,
                CliAnimation::Matrix => AnimationEffect::Matrix,
            };

            let stop = Arc::new(AtomicBool::new(false));
            let stop_clone = stop.clone();

            ctrlc::set_handler(move || {
                stop_clone.store(true, Ordering::SeqCst);
            })?;

            println!(
                "{} Running animation {} (speed {:.1}x, {} FPS)... Press Ctrl+C to stop.",
                "▶".cyan().bold(),
                anim.name().yellow().bold(),
                speed,
                fps
            );

            run_animation_loop(&mut kb, &anim, speed, brightness, fps, stop)?;
            println!("\n{} Animation stopped.", "✓".green());
        }

        Commands::Brightness { level } => {
            let mut kb = open_keyboard(cli.device.as_ref())?;
            kb.set_brightness(level)?;
            println!("{} Set brightness to {}", "✓".green().bold(), level);
        }

        Commands::Off => {
            let mut kb = open_keyboard(cli.device.as_ref())?;
            kb.turn_off()?;
            println!("{} Turned off keyboard backlight", "✓".green().bold());
        }

        Commands::Profile { action } => match action {
            ProfileAction::List => {
                let profiles = load_profiles()?;
                println!("{} Available Lighting Profiles:\n", "★".yellow().bold());
                for p in &profiles {
                    let mode_str = p.mode.name();
                    println!(
                        "  {} {} ({})",
                        "•".cyan(),
                        p.name.bold(),
                        mode_str.dimmed()
                    );
                    println!("    Description: {}", p.description);
                    println!(
                        "    Zones: [Z1: {}, Z2: {}, Z3: {}, Z4: {}]  Brightness: {}",
                        p.zones[0].to_hex().cyan(),
                        p.zones[1].to_hex().cyan(),
                        p.zones[2].to_hex().cyan(),
                        p.zones[3].to_hex().cyan(),
                        p.brightness
                    );
                    println!();
                }
            }

            ProfileAction::Apply { name } => {
                let mut kb = open_keyboard(cli.device.as_ref())?;
                if let Some(prof) = find_profile(&name)? {
                    prof.apply(&mut kb)?;
                    println!("{} Applied profile '{}' successfully!", "✓".green().bold(), prof.name.cyan());
                } else {
                    return Err(anyhow!("Profile '{}' not found. Run 'lenovo-rgb profile list' to view available profiles.", name));
                }
            }

            ProfileAction::Save {
                name,
                description,
                z1,
                z2,
                z3,
                z4,
                all,
                brightness,
            } => {
                let fallback = all.unwrap_or(Color::LEGION_CYAN);
                let zones = [
                    z1.or(all).unwrap_or(fallback),
                    z2.or(all).unwrap_or(fallback),
                    z3.or(all).unwrap_or(fallback),
                    z4.or(all).unwrap_or(fallback),
                ];
                let desc = description.unwrap_or_else(|| "User saved custom profile".to_string());
                let prof = LightingProfile::new_static(name.clone(), desc, zones, brightness);
                save_profile(prof)?;
                println!("{} Saved profile '{}'!", "✓".green().bold(), name.cyan());
            }

            ProfileAction::Delete { name } => {
                if delete_profile(&name)? {
                    println!("{} Deleted profile '{}'.", "✓".green().bold(), name);
                } else {
                    println!("{} Profile '{}' not found.", "✗".red(), name);
                }
            }

            ProfileAction::Show { name } => {
                if let Some(p) = find_profile(&name)? {
                    println!("{}:", p.name.bold().cyan());
                    println!("  Description: {}", p.description);
                    println!("  Mode: {:?}", p.mode);
                    println!("  Brightness: {}", p.brightness);
                    println!("  Speed: {}", p.speed);
                    println!("  Zones: {:?}", p.zones.map(|c| c.to_hex()));
                } else {
                    return Err(anyhow!("Profile '{}' not found.", name));
                }
            }
        },

        Commands::Status => {
            println!("{}", "=== Lenovo RGB Keyboard Diagnostic Status ===".cyan().bold());
            let devices = find_lenovo_devices()?;
            if devices.is_empty() {
                println!("{}", "No candidate Lenovo RGB devices found in /sys/class/hidraw*.".red());
                println!("Run 'lenovo-rgb setup-udev' to configure permissions.");
            } else {
                println!("Found {} candidate HID device(s):", devices.len());
                for (i, dev) in devices.iter().enumerate() {
                    let indicator = if dev.is_lighting_interface {
                        "[RGB Lighting Interface]".green().bold()
                    } else {
                        "[Other HID collection]".dimmed()
                    };
                    println!(
                        "  {}. {} (VID: {:04x}, PID: {:04x}) on {:?}",
                        i + 1,
                        dev.name.bold(),
                        dev.vendor_id,
                        dev.product_id,
                        dev.path
                    );
                    println!("     Status: {}", indicator);

                    // Test open
                    match std::fs::OpenOptions::new().read(true).write(true).open(&dev.path) {
                        Ok(_) => println!("     Permissions: {} (Read/Write OK)", "✓".green()),
                        Err(e) => println!("     Permissions: {} ({})", "✗".red(), e),
                    }
                }
            }
        }

        Commands::SetupUdev => {
            setup_udev_rules()?;
        }

        Commands::Systemd { action } => match action {
            SystemdAction::Install { profile } => {
                install_systemd_service(&profile)?;
            }
            SystemdAction::Uninstall => {
                uninstall_systemd_service()?;
            }
        },

        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        }
    }

    Ok(())
}
