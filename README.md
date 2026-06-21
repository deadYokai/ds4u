[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/banner-direct-single.svg)](https://stand-with-ukraine.pp.ua)

[![Stand For Ukraine](https://img.shields.io/badge/Stand_For_Ukraine-find_a_charity-blue?style=for-the-badge)](https://standforukraine.com/)

---

[![Gitea](https://img.shields.io/badge/Gitea-Repo-black)](https://git.yokai.digital/deadYokai/ds4u)
[![GitLab](https://img.shields.io/badge/GitLab-Mirror-orange)](https://gitlab.com/deadYokai/ds4u)
[![GitHub](https://img.shields.io/badge/GitHub-Mirror-black)](https://github.com/deadYokai/ds4u)

[![AUR](https://img.shields.io/aur/version/ds4u?style=for-the-badge)](https://aur.archlinux.org/packages/ds4u)
[![AURGIT](https://img.shields.io/aur/version/ds4u-git?style=for-the-badge)](https://aur.archlinux.org/packages/ds4u-git)

# DS4U - DualSense for You

Native Linux Gui tool for configuring DualSense controllers.

> [!note]
> **DualSense Edge** support is wired in (device ID and firmware are handled) but  
> is **not tested and not officially supported** yet. If you have problems with  
> this device, please open an *Issue*.  

![app preview](preview/app.png)

## Features

### Controller & inputs
- Live input visualization (buttons, D-pad, sticks, triggers, touchpad)
- Battery level and charging status
- USB and Bluetooth connection, with automatic hotplug detection

### Analog sticks
- Per-stick sensitivity curves
- Inner and outer deadzones per stick
- Invert X / Y per stick
- Swap left/right sticks

### Adaptive triggers
- Independent left/right configuration
- Effect modes: `Off`, `Feedback`, `Weapon`, `Bow`, `Galloping`, `Vibration`, `Machine`
- Adjustable start/end position, strength and frequency
- Per-trigger deadband (release point and full-stroke point)

### Haptics & vibration
- Haptic patterns: `Constant`, `Pulse`, `Ramp`, `Wave` (with strength and speed)
- Raw PCM haptic streaming to the voice-coil actuators (USB, and Bluetooth via the daemon)

### Lightbar & LEDs
- RGB lightbar color, brightness and on/off
- Animated lightbar effects (via the daemon):
    - **Breath**
    - **Rainbow**
    - **Strobe**
- Player-number indicator LEDs
- Microphone mute LED: `Off`, `On`, `Pulse`

### Audio
- Output routing: internal speaker, headphone, or both
- Microphone on/off toggle

### Profiles
- Save, load, switch and delete profiles (see [Profiles](#profiles-1))
- A profile captures everything: lightbar/LEDs, sticks, triggers, haptics, gyro,
  touchpad, mic, plus button remaps and disabled buttons
- Switch/list/reload profiles from the [command line](#command-line-usage)

### Firmware
- Read the controller's current firmware version and build info
- Check the latest firmware version
- Download and flash official firmware for DualSense / DualSense Edge

### Themes
- Built-in themes:
    - Default
    - Author theme (my custom theme)
    - Deep Dark
    - Tokyo Night
- Custom theme support (see [Custom Themes](#custom-themes))

### Daemon
- Background service that keeps the controller connection alive and runs live effects and input processing (see [Daemon](#daemon))
- Start/stop directly from the GUI, run via the CLI, or install as a systemd user service

## Installation

### Arch Linux (AUR)

Released build:

```sh
yay -S ds4u
```

Latest git build:

```sh
yay -S ds4u-git
```

## Building

### Requirements
- Rust
- libraries: `libudev`, `alsa-lib`, `openssl`, `libxkbcommon`

### Build
```sh
git clone https://gitlab.com/deadYokai/ds4u.git
cd ds4u
cargo build --release
```

### Run
```sh
./target/release/ds4u
```

### Device permissions

To talk to the controller without running as root, install a `udev` rule that grants your user access to the DualSense `hidraw` device.  
The AUR package does this for you.

`/etc/udev/rules.d/70-ds4u.rules`:

```
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0ce6", MODE="0664", GROUP="input", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0df2", MODE="0664", GROUP="input", TAG+="uaccess"
```

Then reload the rules and reconnect the controller:

```sh
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## Command-line usage

Running `ds4u` with no arguments launches the GUI.  
The following flags talk to a running daemon over its Unix socket:


| Command | Description |
|---------|-------------|
| `ds4u` | Launch the GUI |
| `ds4u --daemon` | Run as the background daemon |
| `ds4u --status` | Show the connected controller (serial, product id, USB/BT) and battery |
| `ds4u --list-profiles` | List saved profile names |
| `ds4u --switch-profile <name>` | Switch the active profile |
| `ds4u --reload-profile` | Reload the current profile from disk |


The profile commands require the daemon to be running.

## Daemon

The daemon is a background process that owns the controller connection and runs
the *continuous* work the GUI cannot do on its own.  
While it runs it:
- maintains the connection to the controller and watches for hotplug events,
- applies the active profile, and
- runs the live loops for **animated lightbar effects**, **haptic-pattern playback**, **gyro processing**, and **input transforms** (stick curves, deadzones, invert/swap, button remap, touchpad behavior).

One-shot hardware settings (lightbar color, brightness, player LEDs, mic, mic LED) work whether or not the daemon is running;  
the live/animated features above need it.

### Starting daemon

From the GUI (Settings), via the CLI, or as a service.

```sh
ds4u --daemon
```

Or run as systemd service:

```ini
[Unit]
Description=DS4U DualSense Daemon
After=graphical-session.target

[Service]
ExecStart=/usr/local/bin/ds4u --daemon
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
```

## Profiles

Profiles are stored as JSON in `~/.config/ds4u/profiles/<name>.json`, and the filename is derived from the profile name.  
The active profile name is recorded in `settings.json`. A `Default` profile is created automatically if none exists.

Manage profiles from the GUI (the **Profiles** section) or from the [command line](#command-line-usage).

## Custom Themes

Filename must match `id` field inside JSON.

### Theme file format
```json
{
  "id": "tokyo_night",
  "name": "Tokyo Night",
  "dark_mode": true,
  "colors": {
    "window_bg":       [26,  27,  38],
    "panel_bg":        [22,  22,  30],
    "extreme_bg":      [16,  16,  24],
    "accent":          [122, 162, 247],
    "widget_hovered":  [41,  46,  66],
    "widget_inactive": [32,  36,  54],
    "text":            [192, 202, 245],
    "text_dim":        [86,  95,  137],
    "success":         [158, 206, 106],
    "error":           [247, 118, 142],
    "warning":         [224, 175, 104]
  }
}
```

All color values are `[R, G, B]` in the 0–255 range.

### Color roles

| Key              | Description                                      |
|------------------|--------------------------------------------------|
| `window_bg`      | Main application background                      |
| `panel_bg`       | Sidebar and panel backgrounds                    |
| `extreme_bg`     | Deepest background (headers, separators)         |
| `accent`         | Highlights, active indicators, selected items    |
| `widget_hovered` | Widget background on hover                       |
| `widget_inactive`| Widget background at rest                        |
| `text`           | Primary text                                     |
| `text_dim`       | Secondary / hint text                            |
| `success`        | Success indicators (e.g. connected, applied)     |
| `error`          | Error indicators                                 |
| `warning`        | Warning indicators                               |

## Config & Data Paths

| Path | Contents |
|------|----------|
| `~/.config/ds4u/settings.json` | Active theme ID and active profile name |
| `~/.config/ds4u/profiles/` | Saved profiles (`<name>.json`) |
| `~/.config/ds4u/themes/` | Custom theme files (`<id>.json`) |
| `$XDG_RUNTIME_DIR/ds4u.socket` | Daemon Unix socket |


## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## References:
- https://github.com/nowrep/dualsensectl
- https://github.com/hurryman2212/vds

## Todo
- [ ] other possible issues
- [ ] DualSense Egde support

## Support:
[![MySite](https://img.shields.io/badge/yokai.digital-black?style=for-the-badge)](https://yokai.digital)
[![Paypal](https://img.shields.io/badge/Paypal-black?style=for-the-badge&logo=paypal)](https://www.paypal.com/donate/?hosted_button_id=RLGYGXH4LZ8PC)
[![Monobank](https://img.shields.io/badge/Monobank-black?style=for-the-badge)](https://send.monobank.ua/jar/9oVcUiHxPd)
[![Liberapay](https://img.shields.io/liberapay/receives/deadYokai.svg?logo=liberapay&style=for-the-badge&logoColor=white&labelColor=black&color=orange&label=Liberapay)](https://liberapay.com/deadYokai/donate)

