# LOQ-RGB

A fast, lightweight Rust application and Terminal User Interface (TUI) to control 4-zone RGB keyboard backlighting on Lenovo LOQ, Legion, and IdeaPad Gaming laptops on Linux.

It replaces the keyboard lighting controls of Lenovo Vantage by communicating directly with the embedded controller via Linux `hidraw` ioctls.

---

## Features

- **Direct Hardware Control**: Communicates with ITE keyboard controllers via Linux `hidraw` without proprietary background services.
- **4-Zone Customization**: Set colors per zone (Zone 1: Esc/Left, Zone 2: Mid-Left, Zone 3: Mid-Right, Zone 4: Numpad/Nav) or across all zones simultaneously.
- **Hardware Effects (EC Firmware)**: Static, Wave, Breath, and Hue spectrum cycle.
- **Dynamic Software Animations**: Real-time CPU load monitor, RAM monitor, Rainbow Wave, Campfire, and Matrix rain.
- **Interactive TUI**: Terminal dashboard with live 4-zone preview, RGB sliders, and quick presets.
- **Profiles & Boot Restore**: Save custom profiles and automatically apply your preferred profile on login via systemd.
- **No Sudo Needed**: Includes a udev rule for unprivileged user access.

---

## Supported Hardware

Compatible with Lenovo laptops using ITE Tech keyboard lighting controllers (`vendor ID 048d`):

- **ITE 8295 / IT8295** (`048d:c993`, `048d:c994`, `048d:c995`): Lenovo Legion Pro 5, LOQ 15/16, Legion Slim
- **ITE 829x Variants** (`048d:c985`, `048d:c984`, `048d:c983`): Lenovo Legion 5 / 5 Pro
- **ITE 8176 / Legacy** (`048d:c975`, `048d:c965`, `048d:c955`): Legion 5, IdeaPad Gaming 3

Run `lenovo-rgb status` to verify detection on your machine.

---

## Installation

### 1. Build and Install

```bash
git clone https://github.com/rishwith2306/LOQ-RGB.git
cd LOQ-RGB
cargo install --path .
```

### 2. Configure Udev Permissions (One-Time)

Grant your user access to the keyboard interface without root:

```bash
lenovo-rgb setup-udev
```

Or manually:

```bash
sudo cp 99-lenovo-rgb.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=hidraw --action=change
```

---

## Usage

### Interactive TUI

Launch the terminal user interface:

```bash
lenovo-rgb
# or
lenovo-rgb tui
```

### Command Line Interface

```bash
# Set all zones to a single color (hex, rgb, or name)
lenovo-rgb set-all "#00d2ff"
lenovo-rgb set-all red
lenovo-rgb set-all "0,255,128"

# Set individual zones
lenovo-rgb set --z1 "#ff0055" --z2 "#00d2ff" --z3 "#ffff00" --z4 "#9300d3"

# Hardware brightness (0 = off, 1 = low, 2 = high)
lenovo-rgb brightness 2
lenovo-rgb off

# Hardware effects
lenovo-rgb effect wave --speed 3
lenovo-rgb effect breath --speed 2 --color "#00d2ff"
lenovo-rgb effect hue --speed 2

# Software animations
lenovo-rgb animate cpu-monitor
lenovo-rgb animate rainbow-wave
lenovo-rgb animate fire

# Profiles
lenovo-rgb profile list
lenovo-rgb profile apply cyberpunk
lenovo-rgb profile save my-profile --z1 cyan --z2 magenta --z3 yellow --z4 blue

# Auto-apply profile on login
lenovo-rgb systemd install cyberpunk
```

---

## Shell Completions

```bash
# Bash
lenovo-rgb completions bash > ~/.local/share/bash-completion/completions/lenovo-rgb

# Zsh
lenovo-rgb completions zsh > ~/.zsh/completion/_lenovo-rgb

# Fish
lenovo-rgb completions fish > ~/.config/fish/completions/lenovo-rgb.fish
```

---

## Technical Details

The ITE 8295 controller exposes a vendor-specific HID interface (`Usage Page 0xFF89`, `Usage 0x00CC`). Lighting commands are sent as 33-byte feature reports using the `HIDIOCSFEATURE(33)` ioctl (`0xc0214806`) on `/dev/hidraw*`:

```text
Byte  0:     0xCC (Report ID)
Byte  1:     0x16 (Lighting Command Header)
Byte  2:     Effect Mode (0x01: Static, 0x03: Breath, 0x04: Wave, 0x06: Hue)
Byte  3:     Speed (1..4)
Byte  4:     Brightness (0: Off, 1: Low, 2: High)
Bytes 5-7:   Zone 1 (Esc/Left)   - R, G, B
Bytes 8-10:  Zone 2 (Mid-Left)   - R, G, B
Bytes 11-13: Zone 3 (Mid-Right) - R, G, B
Bytes 14-16: Zone 4 (Numpad)    - R, G, B
Bytes 17-32: Padding (16 zeros)
```

---

## License

MIT
