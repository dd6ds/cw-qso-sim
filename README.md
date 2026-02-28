# WIP - this Project has a lot bugs !

# cw-qso-sim

A terminal-based Morse code QSO simulator and trainer — cross-platform, multi-adapter, multi-language.

Practice realistic CW contacts against a simulated station at adjustable speed.  
Supports iambic paddles (VBand USB HID, ATtiny85/Digispark MIDI), straight keys, and a keyboard text-input fallback so you can run it anywhere without hardware.

---

## Features

- **QSO engine** — ragchew, contest, DX pile-up, and random styles
- **Iambic keyer** — mode A and B, straight key, or keyboard text-input fallback
- **Sidetone** — real-time audio feedback via CPAL
- **Farnsworth timing** — stretch inter-character gaps for beginners
- **Adaptive decoder** — separate WPM clocks for the simulator TX and your paddle
- **Multi-language TUI** — English, German, French, Italian (`--lang`)
- **Zero-install config** — sane defaults, write a starter config with one flag

---

## Supported adapters

| Adapter | Interface | VID / PID | Platform |
|---|---|---|---|
| VBand USB CW Adapter | USB HID | VID `413d` / PID `2107` | Linux, Windows |
| ATtiny85 / Digispark | USB MIDI | VID `16d0` / PID `0753` | Linux, Windows |
| Arduino Nano | Serial MIDI (31250 baud) | VID `1a86` / PID `7523` (CH340)<br>VID `1a86` / PID `55d4` (CH9102)<br>VID `0403` / PID `6001` (FT232RL)<br>VID `2341` / PID `0043` (ATmega16U2, new bootloader)<br>VID `2341` / PID `0001` (ATmega16U2, old bootloader) | Linux, Windows |
| Arduino Uno | Serial MIDI (31250 baud) | VID `2341` / PID `0043` (ATmega16U2, new bootloader)<br>VID `2341` / PID `0001` (ATmega16U2, old bootloader)<br>VID `2341` / PID `0049` (WiFi Rev2)<br>VID `2341` / PID `0069` (R4 Minima)<br>VID `2341` / PID `0070` (R4 WiFi)<br>VID `1a86` / PID `7523` (CH340 clone)<br>VID `0403` / PID `6001` (FT232RL clone) | Linux, Windows |
| ESP8266 | Serial MIDI (115200 baud) | VID `1a86` / PID `7523` | Linux, Windows |
| ESP32 DEV | Serial MIDI (115200 baud) | VID `10c4` / PID `ea60` | Linux, Windows |
| Keyboard / text-input | Built-in fallback | All (no hardware needed) |

---

## Quick start

```sh
# 1. Write the default config and set your callsign
cw-qso-sim --write-config
$EDITOR ~/.config/cw-qso-sim/config.toml   # set mycall

# 2. Run (auto-detects adapter, falls back to keyboard)
cw-qso-sim

# 3. Keyboard text-input mode (no paddle required)
#    Type normally — Space = commit word, Enter = end of over (like pressing K)
cw-qso-sim --adapter keyboard
```

---

## CLI reference

```
USAGE:
    cw-qso-sim [OPTIONS]

OPTIONS:
    --mycall <CALL>          Your callsign
    --sim-wpm <N>            Simulator TX speed in WPM (default: 25)
    --user-wpm <N>           Your keying / decoder speed in WPM (default: 18)
    --tone <HZ>              Sidetone frequency in Hz (default: 620)
    --adapter <TYPE>         auto | vband | attiny85 | arduino-nano | arduino-uno | keyboard
    --paddle-mode <MODE>     iambic_a | iambic_b | straight
    --switch-paddle          Swap DIT and DAH paddles
    --port <PORT>            Serial / MIDI port (ATtiny85 port name/substring)
    --midi-port <PORT>       MIDI port override for ATtiny85 (takes precedence over --port)
    --who-starts <WHO>       me | sim — who sends CQ first (default: sim)
    --style <STYLE>          ragchew | contest | dx_pileup | darc_cw_contest | mwc-contest | random
    --lang <LANG>            en | de | fr | it
    --config <PATH>          Custom config file path
    --write-config           Write the built-in default config.toml and exit
    --print-config           Print the built-in default config.toml to stdout
    --list-ports             List detected HID / MIDI keyer devices and exit
    --check-adapter          Interactive paddle test (press DIT then DAH when prompted)
    -h, --help               Print help
    -V, --version            Print version
```

---

## Configuration

Config is loaded from `~/.config/cw-qso-sim/config.toml` (Linux/macOS) or `%APPDATA%\cw-qso-sim\config.toml` (Windows).  
CLI flags always override the file.

```toml
[general]
mycall     = "DD6DS"         # your callsign
who_starts = "sim"           # "sim" = simulator sends CQ | "me" = you send CQ
language   = "en"            # en | de | fr | it

[morse]
sim_wpm        = 25          # simulator TX speed
user_wpm       = 18          # your keying / decoder speed
farnsworth_wpm = 0           # 0 = disabled; stretches inter-char gaps for beginners
tone_hz        = 620         # sidetone frequency in Hz
volume         = 0.7         # 0.0 – 1.0
sidetone       = true        # play sidetone while you key

[keyer]
adapter    = "auto"          # auto | vband | attiny85 | arduino-nano | arduino-uno | keyboard
mode       = "iambic_a"      # iambic_a | iambic_b | straight
# port     = "/dev/ttyUSB0" # serial port for arduino-nano / arduino-uno (auto-detected if omitted)
# switch_paddle = false      # true = swap DIT and DAH paddles

[qso]
style        = "ragchew"     # ragchew | contest | dx_pileup | darc_cw_contest | random
min_delay_ms = 800           # simulated operator reaction time (ms)
max_delay_ms = 2500
typo_rate    = 0.05          # probability of a simulated typo (0.0 – 1.0)
```

---

## Adapter setup

### VBand USB HID (Linux)

The device appears as `/dev/hidraw*` which is root-only by default.  
Create a udev rule once and you never need `sudo` again:

```sh
sudo tee /etc/udev/rules.d/99-vband-cw.rules <<'EOF'
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="413d", ATTRS{idProduct}=="2107", GROUP="plugdev", MODE="0660"
EOF
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG plugdev $USER   # re-login required
```

Quick test without rebooting:
```sh
sudo chmod a+rw /dev/hidraw*
```

### VBand USB HID (Windows 11)

The VBand adapter works out-of-the-box with the built-in **HidUsb** driver.  
If you accidentally installed a WinUSB / libwdi driver via Zadig, the device disappears from the HID stack. Two options:

**Option A — restore the HID driver (recommended):**  
Device Manager → right-click the VBand device → *Update driver → Browse → Let me pick → HID-compliant device*

**Option B — build with WinUSB fallback support:**  
```sh
cargo build --release --features keyer-vband,keyer-vband-winusb
```
This adds a `rusb`/libusb backend that reaches the device through the WinUSB driver automatically when HID fails.

---

### ATtiny85 / Digispark (all platforms)

#### Wiring

```
ATtiny85 Pin 2 (P2)  →  LEFT  paddle (DIT)
ATtiny85 Pin 0 (P0)  →  RIGHT paddle (DAH)
ATtiny85 GND         →  Paddle common ground
```


### ESP8266 (all platforms)

https://arduino.esp8266.com/stable/package_esp8266com_index.json

#### Wiring

```
ESP32 GPIO14 (D5)   →  LEFT  paddle (DIT)
ESP32 GPIO12 (D6)   →  RIGHT paddle (DAH)
ESP32 GND           →  Paddle common ground
```


### ESP32 (all platforms)

Tools -> Board -> Boardmanager -> Search esp32 Espressif -> Install

Select: ESP32 DEV
Upload Speed 115200

#### Wiring

```
ESP32 GPIO22   →  LEFT  paddle (DIT)
ESP32 GPIO23   →  RIGHT paddle (DAH)
ESP32 GND      →  Paddle common ground
```



### Arduino Nano (all platforms)

The Nano runs `paddle_debug_Arduino_Nano.ino` and sends MIDI bytes over its hardware UART at 31250 baud. No USB-MIDI bridge is needed — cw-qso-sim opens the serial port directly.

The adapter is selected with `--adapter arduino-nano`. The port is auto-detected by USB VID/PID; use `--port /dev/ttyUSB0` (or `--port COM3` on Windows) if auto-detection fails.

#### Wiring

```
Arduino Nano D2  →  LEFT  paddle (DIT)
Arduino Nano D3  →  RIGHT paddle (DAH)
Arduino Nano GND →  Paddle common ground
```


Flashing:


File -> Preferences

add https://github.com/FortySevenEffects/arduino_midi_library

Tools -> Manage Libraries

search for FortySevenEffects
select USB-MIDI -> Install ALL

Processor: ATMega328P (Old Bootloader)
Port: /dev/ttyUS0


#### udev rules (Linux)

Most cheap Nano clones use a CH340 USB–serial chip and appear as `/dev/ttyUSB*`. Genuine Nanos with an ATmega16U2 appear as `/dev/ttyACM*`. Add yourself to the `dialout` group once and you never need `sudo`:

```sh
sudo usermod -aG dialout $USER   # re-login required
```

Or create a fine-grained udev rule. Save as `/etc/udev/rules.d/49-arduino-nano.rules`:

```
# Arduino Nano — CH340 / CH341 (most clone boards)
SUBSYSTEMS=="usb", ATTRS{idVendor}=="1a86", ATTRS{idProduct}=="7523", MODE:="0666"
KERNEL=="ttyUSB*", ATTRS{idVendor}=="1a86", ATTRS{idProduct}=="7523", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"

# Arduino Nano — CH9102 (newer CH340-family variant)
SUBSYSTEMS=="usb", ATTRS{idVendor}=="1a86", ATTRS{idProduct}=="55d4", MODE:="0666"
KERNEL=="ttyUSB*", ATTRS{idVendor}=="1a86", ATTRS{idProduct}=="55d4", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"

# Arduino Nano — FT232RL
SUBSYSTEMS=="usb", ATTRS{idVendor}=="0403", ATTRS{idProduct}=="6001", MODE:="0666"
KERNEL=="ttyUSB*", ATTRS{idVendor}=="0403", ATTRS{idProduct}=="6001", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"

# Arduino Nano — genuine ATmega16U2 (new bootloader)
SUBSYSTEMS=="usb", ATTRS{idVendor}=="2341", ATTRS{idProduct}=="0043", MODE:="0666"
KERNEL=="ttyACM*", ATTRS{idVendor}=="2341", ATTRS{idProduct}=="0043", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"

# Arduino Nano — genuine ATmega16U2 (old bootloader)
SUBSYSTEMS=="usb", ATTRS{idVendor}=="2341", ATTRS{idProduct}=="0001", MODE:="0666"
KERNEL=="ttyACM*", ATTRS{idVendor}=="2341", ATTRS{idProduct}=="0001", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"
```

Then reload: `sudo udevadm control --reload-rules && sudo udevadm trigger`

Quick test without re-login:
```sh
sudo chmod a+rw /dev/ttyUSB0   # or /dev/ttyACM0 for genuine Nanos
```

#### Programming the Arduino Nano

1. Install [Arduino IDE](https://www.arduino.cc/en/software)
2. Install the **MIDI Library** via *Sketch → Include Library → Manage Libraries* → search **MIDI Library** (by Francois Best / lathoub)
3. Open `paddle_debug_Arduino_Nano.ino` from this repository
4. *Tools → Board → Arduino AVR Boards → Arduino Nano*
5. *Tools → Processor → ATmega328P* (or **ATmega328P (Old Bootloader)** for most cheap clones)
6. *Tools → Port* → select the Nano's serial port
7. *Sketch → Upload*

The onboard LED (D13) lights up while any paddle is pressed, confirming the firmware is running.

---

### Arduino Uno (all platforms)

#### Programming the Arduino UNO

1. Install [Arduino IDE](https://www.arduino.cc/en/software)
2. Install the **MIDI Library** via *Sketch → Include Library → Manage Libraries* → search **MIDI Library** (by Francois Best / lathoub)
3. Open `paddle_debug_Arduino_Nano.ino` from this repository
4. *Tools → Board → Arduino AVR Boards → Arduino UNO*
5. *Tools → Port* → select the Nano's serial port
6. *Sketch → Upload*



#### Wiring

```
Arduino Uno D2  →  LEFT  paddle (DIT)
Arduino Uno D3  →  RIGHT paddle (DAH)
Arduino Uno GND →  Paddle common ground
```


Flashing:


File -> Preferences

add https://github.com/FortySevenEffects/arduino_midi_library

Tools -> Manage Libraries

search for FortySevenEffects
select USB-MIDI -> Install ALL

Port: /dev/ttyUS0

 



#### udev rule (Linux / Debian / Mint)

Create `/etc/udev/rules.d/49-digispark.rules`:

```
# Digispark ATtiny85
SUBSYSTEMS=="usb", ATTRS{idVendor}=="16d0", ATTRS{idProduct}=="0753", MODE:="0666"
KERNEL=="ttyACM*",  ATTRS{idVendor}=="16d0", ATTRS{idProduct}=="0753", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"

# Digispark ATtiny167
SUBSYSTEMS=="usb", ATTRS{idVendor}=="16d0", ATTRS{idProduct}=="0753", MODE:="0666"
```

Create `/etc/udev/rules.d/49-arduino-uno.rules`:    (WIP)


```
# Arduino Uno (Original, with ATmega16U2 USB interface)
SUBSYSTEMS=="usb", ATTRS{idVendor}=="2341", ATTRS{idProduct}=="0043", MODE:="0666"
KERNEL=="ttyACM*", ATTRS{idVendor}=="2341", ATTRS{idProduct}=="0043", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"

# Arduino Uno (Clone boards using CH340 USB–serial chip)
SUBSYSTEMS=="usb", ATTRS{idVendor}=="1a86", ATTRS{idProduct}=="7523", MODE:="0666"
KERNEL=="ttyUSB*", ATTRS{idVendor}=="1a86", ATTRS{idProduct}=="7523", MODE:="0666", ENV{ID_MM_DEVICE_IGNORE}="1"
```




Then reload: `sudo udevadm control --reload-rules && sudo udevadm trigger`

#### Programming the ATtiny85

1. Install [Arduino IDE](https://www.arduino.cc/en/software)
2. Add Digispark board support:  
   *File → Preferences → Additional Board Manager URLs*  
   `https://raw.githubusercontent.com/digistump/arduino-boards-index/master/package_digistump_index.json`
3. *Tools → Board → Boards Manager* → install **Digistump AVR Boards**
4. Install [DigisparkMIDI](https://github.com/heartscrytech/DigisparkMIDI) library:  
   *Sketch → Include Library → Manage Libraries* → search for **DigisparkMIDI**
5. Open `paddle_decoder.ino` from this repository
6. *Tools → Board → Digispark (Default - 16.5 MHz)*
7. *Sketch → Upload* — plug in the Digispark when Arduino IDE prompts you to

---

## Building from source

```sh
# Debug build (host platform, all features)
cargo build

# Release build
cargo build --release

# Windows (cross-compile from Linux)
./build-cross.sh

# All targets
./cross/build-all.sh
```

### Cargo features

| Feature | Default | Description |
|---|---|---|
| `audio-cpal` | ✓ | CPAL audio backend (sidetone + CW playback) |
| `keyer-vband` | ✓ | VBand USB HID paddle support |
| `keyer-vband-winusb` | — | WinUSB fallback for VBand on Windows (libwdi/Zadig driver) |
| `keyer-attiny85` | ✓ | ATtiny85/Digispark MIDI paddle support |
| `tui` | ✓ | Ratatui terminal UI |



## Winkeyer

'''
# Linux
./cw-qso-sim-aarch64-unknown-linux-gnu --adapter win-keyer --port /dev/ttyUSB0 --style darc_cw_contest

# Windows:
cw-qso-sim-x86_64-pc-windows-gnu.exe --adapter win-keyer --port COM3 --style darc_cw_contest
'''


---

## Troubleshooting

**No adapter found / falls back to keyboard**  
Run `cw-qso-sim --list-ports` to see what's detected, then `cw-qso-sim --check-adapter` to run an interactive paddle test.

**Paddles are swapped**  
Add `--switch-paddle` on the CLI or set `switch_paddle = true` in `[keyer]`.

**MIDI port not found (ATtiny85)**  
Run `--list-ports` to see the exact port name, then set it with `--midi-port "Digispark"` or in config.

**Arduino Nano / Uno not found automatically**
Run `cw-qso-sim --list-ports` to see all detected serial ports, then supply the port explicitly: `--port /dev/ttyUSB0` (Linux) or `--port COM3` (Windows). On Linux, make sure you are in the `dialout` group (`sudo usermod -aG dialout $USER`, then re-login) or that the udev rule above is in place.

**Arduino Nano on Linux shows `/dev/ttyUSB0` but permission denied**
Either add the udev rule (see above) or run `sudo chmod a+rw /dev/ttyUSB0` as a quick workaround.

**WinUSB / libwdi device not detected (Windows)**
See [VBand USB HID (Windows 11)](#vband-usb-hid-windows-11) above.

---

## License

GPL v3

---


73 de DD6DS


! VIBE CODED !

---

##Check Adapter##

ATTiny85

./cw-qso-sim-x86_64-unknown-linux-gnu --adapter attiny85 --check-adapter

cw-qso-sim-x86_64-pc-windows-gnu.exe --adapter attiny85 --check-adapter

VBAND USB 

./cw-qso-sim-x86_64-unknown-linux-gnu --adapter vband --check-adapter

cw-qso-sim-x86_64-pc-windows-gnu.exe --adapter vband --check-adapter

Arduino UNO

./cw-qso-sim-x86_64-unknown-linux-gnu --adapter arduino-uno --port /dev/ttyUSB0 --check-adapter

cw-qso-sim-x86_64-pc-windows-gnu.exe --adapter arduino-uno --port com3 --check-adapter

Arduino NANO

./cw-qso-sim-x86_64-unknown-linux-gnu --adapter arduino-nano --port /dev/ttyUSB0 --check-adapter

cw-qso-sim-x86_64-pc-windows-gnu.exe --adapter arduino-nano --port com3 --check-adapter

ESP8266

./cw-qso-sim-x86_64-unknown-linux-gnu --adapter esp8266 --port /dev/ttyUSB0 --check-adapter

cw-qso-sim-x86_64-pc-windows-gnu.exe --adapter esp8266 --port com4 --check-adapter


ESP32 DEV

./cw-qso-sim-x86_64-unknown-linux-gnu --adapter esp32 --port /dev/ttyUSB0 --check-adapter

cw-qso-sim-x86_64-pc-windows-gnu.exe --adapter esp32 --port com4 --check-adapter




