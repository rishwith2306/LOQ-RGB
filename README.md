# 🌈 Lenovo RGB (`lenovo-rgb`)

A blazing-fast, native **Rust** application and interactive Terminal User Interface (TUI) to control the **4-Zone RGB keyboard backlight** on **Lenovo Legion**, **LOQ**, and **IdeaPad Gaming** laptops on Linux.

It replicates and extends the keyboard lighting capabilities previously exclusive to **Lenovo Vantage** on Windows, communicating directly with the hardware without bloatware or background battery drain.

---

## ✨ Features

- **No Windows / Lenovo Vantage Required**: Communicates directly with the ITE keyboard controller via Linux `hidraw` ioctls.
- **Full 4-Zone Control**: Set custom colors per zone (Zone 1: Esc/Left, Zone 2: Mid-Left, Zone 3: Mid-Right, Zone 4: Numpad/Nav) or all zones at once.
- **Flexible Color Formats**: Hex (`#00D2FF`, `00d2ff`, `#f0f`), RGB (`0,210,255`), or friendly names (`cyan`, `ice-blue`, `red`, `green`, `purple`, `gold`, `white`, etc.).
- **Hardware Effects (EC Firmware)**:
  - **Static**: Custom 4-zone solid colors.
  - **Wave**: Moving RGB rainbow wave across the 4 zones (speed 1–4).
  - **Breath**: Smooth breathing pulsation in chosen color/speed.
  - **Hue Cycle**: Continuous spectrum rainbow color wheel.
- **Dynamic Software Animations**:
  - 🖥️ **CPU Load Monitor**: Turns the keyboard into a live hardware CPU gauge (Cool Cyan -> Green -> Warning Yellow -> Red based on CPU utilization).
  - 🧠 **RAM Monitor**: Real-time memory pressure indicator.
  - 🌊 **Rainbow Wave**: Smooth sinusoidal traveling color wave.
  - 🔥 **Campfire**: Organic warm flickering embers.
  - 🚨 **Police Siren**: Alternating emergency strobe.
  - 🟩 **Matrix Rain**: Cascading hacker green digital rain.
- **Interactive TUI Dashboard (`ratatui`)**:
  - Live 24-bit TrueColor 4-zone visual keyboard preview.
  - RGB sliders with immediate real-time hardware response.
  - Built-in color preset palette (Legion Cyan, Cyberpunk, Amber, Deep Space, etc.).
  - Profile switcher and one-key profile saving (`S`).
- **Profile System**: Save, load, and manage custom color setups in `~/.config/lenovo-rgb/profiles.json`.
- **Systemd Boot Integration**: Restore your favorite profile automatically on login/reboot via a systemd user service.
- **Shell Completions**: Pre-generated autocomplete scripts for Bash, Zsh, and Fish.

---

## 🛠️ Supported Hardware

Designed for Lenovo laptops featuring **ITE Tech** embedded keyboard lighting controllers (vendor ID `048d`):

| Controller | Typical Product IDs | Laptops |
|---|---|---|
| **ITE 8295 / IT8295** | `048d:c993`, `048d:c994`, `048d:c995` | Lenovo Legion Pro 5 (2023/2024), LOQ 15/16 (2024), Legion Slim |
| **ITE 829x Variants** | `048d:c985`, `048d:c984`, `048d:c983` | Lenovo Legion 5 / 5 Pro (2022/2023) |
| **ITE 8176 / Legacy** | `048d:c975`, `048d:c965`, `048d:c955` | Legion 5 (2020/2021), IdeaPad Gaming 3 |

Run `lenovo-rgb status` to verify detection on your machine.

---

## 🚀 Quick Start & CLI Usage

### Launch Interactive TUI
Simply run `lenovo-rgb` in any terminal:
```bash
lenovo-rgb
# or
lenovo-rgb tui
```

### Static Colors
```bash
# Set all zones to signature Legion Vantage Cyan
lenovo-rgb set-all "#00d2ff"

# Set all zones by name or RGB
lenovo-rgb set-all red
lenovo-rgb set-all "0,255,128"

# Set individual zones independently
lenovo-rgb set --z1 "#ff0055" --z2 "#00d2ff" --z3 "#ffff00" --z4 "#9300d3"

# Turn off keyboard backlight
lenovo-rgb off

# Adjust hardware brightness: 0 (off), 1 (low), 2 (high)
lenovo-rgb brightness 2
```

### Hardware Effects (EC Firmware)
```bash
# Wave effect at speed 3
lenovo-rgb effect wave --speed 3

# Breathing effect with custom color
lenovo-rgb effect breath --speed 2 --color "#00d2ff"

# Hue / Spectrum color cycle
lenovo-rgb effect hue --speed 2
```

### Dynamic Software Animations
```bash
# Live CPU load monitor across the 4 zones
lenovo-rgb animate cpu-monitor

# Smooth software rainbow wave
lenovo-rgb animate rainbow-wave --speed 1.5

# Campfire embers
lenovo-rgb animate fire

# Matrix rain
lenovo-rgb animate matrix
```

### Profiles
```bash
# List all profiles
lenovo-rgb profile list

# Apply a preset profile (e.g. cyberpunk, vantage-cyan, gaming-fire, stealth, deep-space)
lenovo-rgb profile apply cyberpunk

# Save current colors as a new profile
lenovo-rgb profile save sunset --z1 "#ff5e36" --z2 "#ff007f" --z3 "#7928ca" --z4 "#4338ca"
```

### Auto-restore on Boot / Login
To automatically apply your favorite lighting profile every time you log in:
```bash
lenovo-rgb systemd install cyberpunk
```

---

## 📦 Installation & Setup

### 1. Build and Install Binary
```bash
cargo build --release
cargo install --path .
```
This places the binary at `~/.cargo/bin/lenovo-rgb`. A copy is also placed in `/usr/local/bin/lenovo-rgb`.

### 2. Configure Udev Permissions (One-Time)
To allow your user account to control the keyboard without `sudo`:
```bash
lenovo-rgb setup-udev
```
Or manually install the included rule:
```bash
sudo cp 99-lenovo-rgb.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=hidraw --action=change
```

### 3. Shell Autocompletions
```bash
# Bash:
lenovo-rgb completions bash > ~/.local/share/bash-completion/completions/lenovo-rgb

# Zsh:
lenovo-rgb completions zsh > ~/.zsh/completion/_lenovo-rgb

# Fish:
lenovo-rgb completions fish > ~/.config/fish/completions/lenovo-rgb.fish
```

---

## 🔬 Protocol & Hardware Details

The ITE 8295 controller exposes a vendor-specific HID interface (`Usage Page 0xFF89`, `Usage 0x00CC`) over USB. Lighting frames are transmitted using a fixed 33-byte feature report with Report ID `0xCC`:

```text
Byte  0:  0xCC  (Report ID)
Byte  1:  0x16  (Lighting Command Header)
Byte  2:  Effect Mode (0x01: Static, 0x03: Breath, 0x04: Wave, 0x06: Hue)
Byte  3:  Speed (1..4)
Byte  4:  Brightness (0: Off, 1: Low, 2: High)
Byte 5-7:  Zone 1 (Esc/Left)   - R, G, B (0..255)
Byte 8-10: Zone 2 (Mid-Left)   - R, G, B (0..255)
Byte 11-13: Zone 3 (Mid-Right) - R, G, B (0..255)
Byte 14-16: Zone 4 (Numpad)    - R, G, B (0..255)
Byte 17-32: Padding (16 zeros)
```

The app issues a Linux `HIDIOCSFEATURE(33)` ioctl call (`0xc0214806`) directly to the matching `/dev/hidraw*` character device node.

---

## 📜 License
MIT License.
