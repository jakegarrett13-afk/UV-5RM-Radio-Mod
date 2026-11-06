# UV5-RM-MOD
This repo contains new firmware for Baofeng UV5-RM radios adding new apps including texting. CAUTION: CHIRP will no longer work so use the included apps to program channels and categories

Custom firmware and add-on apps for the Baofeng UV-5RM, built on the open-source
[bf-5rh-firmware-open](https://github.com/sophiel-meow/bf-5rh-firmware-open). The main addition
is `TEXT`: short text messages between two radios, with delivery confirmation and no phone,
internet or extra hardware.

> **Flashing can't be undone.** No copy of the factory firmware is known for this hardware, so
> there's no way back to stock, and CHIRP stops working with the radio. A bad flash could brick
> it. This has only been run on two radios, so use it at your own risk.

## What it adds

| Feature | What it does |
|---|---|
| **TEXT** | Sends messages up to 50 characters, retries until confirmed, saves them, and plays a tone when one arrives |
| **Scan modes** | Press Band to switch between Weather, Police/Fire/EMS, Air, Repeaters, FRS, GMRS, MURS and more, or type a custom range |
| **BAND SWEEP** | Sweeps a whole band for activity |
| **RPT FINDER** | Works out a repeater's input frequency and tone |
| **FOX HUNT** | Signal meter and rough distance estimate for finding a transmitter |
| **SEARCH/QT SCAN** | Finds a nearby signal's frequency, or the tone on your channel; press 1 to switch |
| **Look and feel** | Skull start-up logo, black and red menus, and a custom start-up tune |

Screenshots: see the `preview_*.png` files in `UV-5RM Custom Firmware`.

## Before you start: check your radio

This firmware only fits one hardware version. Radios that look identical can have a different chip.

1. Turn the radio off, then hold **8** while turning it on.
2. Read the text on the screen, then turn the radio off without pressing anything else.

| Screen says | What to do |
|---|---|
| `5RHNRFB-YP07` | This is the right hardware, so carry on with `UV-5RM_Custom_Firmware.BF` |
| `5RHBK` | Carry on, but flash `UV-5RM_Custom_Firmware_BK4819.BF` instead. See [BK4819 radios](#bk4819-radios-5rhbk) |
| Anything else, such as `...NRF` | **Don't flash it.** It may boot but not transmit or receive |

## What you need

- A Windows PC. The tools are Python, so other systems may work, but they're untested.
- [Python](https://www.python.org/downloads/), plus the serial library: `py -m pip install pyserial`
- A Baofeng programming cable. The commands below assume `COM3`, so change it to match your port.

Run these from the `UV-5RM Custom Firmware` folder.

## Install

**1. Back up the radio.** Flash mode can't read it, so do this first, with the radio on and on
its main screen. This only reads. It may not answer until the radio has been power-cycled, and
the folder has to exist.

```
mkdir my-backup
py source-changes\radio_read.py COM3 my-backup\factory_memory.img
```

**2. Enter flash mode.** Turn the radio off, hold **both side buttons** and turn it on. On radios
with an orange button above PTT and a black one below it, those are the two to hold. The screen
stays black, and the flashlight may come on. Check that it answers (this doesn't write anything):

```
py source-changes\bootloader_probe.py COM3
```

**3. Flash.** This is the point of no return. It takes under a minute, so don't touch the radio or
the cable. The long number is a checksum, and the tool refuses a file that doesn't match it.

```
py source-changes\flash_bf.py UV-5RM_Custom_Firmware.BF COM3 4340f205430f0741
```

On a `5RHBK` radio, flash the BK4819 build instead:

```
py source-changes\flash_bf.py UV-5RM_Custom_Firmware_BK4819.BF COM3 05efa908ffa23816
```

**4. Install the logo and apps.** Wait for the radio to restart to its main screen. This reads
everything back to confirm it.

```
py source-changes\setup_radio.py COM3 skull_logo.rgb565 64 apps
```

**5. Turn on transmit.** It starts off as a safety step. Go to `SETTINGS`, then `RADIO`, and select
`TXINH`. Press **Menu**, use **Up/Down** to choose off, then press **Menu** again to save. **Exit**
cancels the change, and the radio keeps showing `TX PROHIBITED`.

**6. Load channels (optional).** Open `UV-5RM_Channel_Uploader.html` in Chrome or Edge. The channel
list in this repo is for Clark County, Nevada, so check that it suits your area. Your radio should
keep whatever channels it already had.

To choose which channels get scanned, open `UV-5RM_Scan_Builder.html`, tick the ones you want, and
load the CSV it saves into the uploader.

Optional extras: `set_boot_tune.py` sets a Doom-style start-up riff, and `set_radio2_mario_tune.py`
sets a Super Mario one but only runs on one specific radio, so copy and edit it for yours.

## Using TEXT

Both radios need `TEXT` open, on the same simplex frequency. It uses the frequency the radio was on
when you opened it. Don't use a repeater.

| Key | Action |
|---|---|
| Number keys | Type, flip-phone style; the letters are shown at the top of the screen |
| **Menu** | Send |
| **Exit** | Delete a letter; stops retries while sending; closes the app when nothing is typed |
| Hold **Exit** | Close the app |
| **Up / Down** | Scroll saved messages |
| **\*** | Send a numbered test ping |
| **A/B** | Switch between low (default) and high power |
| Hold **#** twice | Erase all saved messages |

| Mark | Meaning |
|---|---|
| `>` | Sent, waiting for a reply |
| `+` | The other radio confirmed it |
| `!` | Never confirmed. It may still have arrived, since only the reply may have been lost |
| `<` | Received |

**Limits:**
- A radio only receives while `TEXT` is open.
- It's built for two radios; with more, the automatic replies would collide.
- Messages have **no encryption**, and they're stored unencrypted on the radio. Scrambling isn't
  security, so any radio with this app can read them.
- Range and long messages have barely been tested. Most testing was short pings a few feet apart.

## BK4819 radios (5RHBK)

Some UV-5RMs have a Beken BK4819 radio chip instead of the FD6818 this firmware was written for. Their
boot screen (hold **8** while turning on) shows `5RHBK`. The rest of the board is the same: the same
MCU, pins, flash chip, screen and bootloader. `UV-5RM_Custom_Firmware_BK4819.BF` is built for them.

**What's different in the BK4819 build** (behind the `bk4819` build option, see
[Building from source](#building-from-source)):

- It starts the radio chip with Baofeng's own values for this board, taken from the stock `5RH_BK_V0.14`
  firmware. The standard build uses tuning for the FD6818's sibling chip, which a BK4819 doesn't accept,
  so a BK radio would boot but not transmit or receive.
- It sets transmit power the way the stock BK firmware does, which is what the factory power calibration
  assumes.
- If the MCU's 16 MHz crystal doesn't start, it runs from the internal clock at the same speed instead
  of freezing on a black screen. Stock BK firmware never uses the crystal.

**Tested so far:** one `5RHBK` radio on stock `V0.14`. It boots, receives and transmits, and the logo and
all apps install and verify. Output power hasn't been measured, and `TEXT` hasn't been tried between two
BK radios. Start on low power, ideally into a dummy load or wattmeter.

**Going back to stock** should be possible, because flashing doesn't touch the bootloader. Baofeng's
stock BK firmware is in
[amoxu/Baofeng-UV-5RM-5RH-RE](https://github.com/amoxu/Baofeng-UV-5RM-5RH-RE/tree/main/factory_firmware/BK_BK4819).
Those files carry a second data region that `source-changes/flash_bf.py` refuses, so they need
`firmware-source/tools/flash.py`. This restore hasn't been tried.

## Legal

You're responsible for what you transmit.

- **`TEXT` transmits on its own.** With the app open, the radio sends a reply automatically to any
  message it receives, so stay on a frequency you're licensed for.
- **Ham bands:** you need a license and must identify with your callsign. `TEXT` doesn't send one,
  so you'd have to identify by voice. Encrypting messages isn't allowed.
- **FRS, GMRS and MURS:** this radio isn't type-accepted for them, so treat those channels as
  receive-only. Rules also vary by country.
- **Power:** output hasn't been measured, and the radio's actual power may differ from its label.

## Building from source

`firmware-source` is the complete source of the provided firmware and apps; the firmware and `TEXT`
were confirmed to rebuild byte for byte. It needs [Rust](https://rustup.rs) and the target in its
`rust-toolchain.toml`. Use a short `--target-dir`, because long paths break linking on Windows.

```
cargo build --release --manifest-path firmware-source/Cargo.toml --config firmware-source/.cargo/config.toml --target-dir C:/t/fw
py source-changes/elf2bin.py C:/t/fw/thumbv7em-none-eabi/release/bf5rh-fw fw.bin
```

For a `5RHBK` radio, build with `--features bk4819` and wrap the image as a `.BF`. Without the option, the
build stays byte for byte identical to `UV-5RM_Custom_Firmware.BF` (checked with Rust 1.99.1).
`wrap_bf.py` checks its encryption against at least five factory `.BF` files first. Download them from
the `factory_firmware` folder of [amoxu/Baofeng-UV-5RM-5RH-RE](https://github.com/amoxu/Baofeng-UV-5RM-5RH-RE),
keeping one subfolder per model:

```
cargo build --release --features bk4819 --manifest-path firmware-source/Cargo.toml --config firmware-source/.cargo/config.toml --target-dir C:/t/fwbk
py source-changes/elf2bin.py C:/t/fwbk/thumbv7em-none-eabi/release/bf5rh-fw bk.bin
py source-changes/wrap_bf.py bk.bin UV-5RM_Custom_Firmware_BK4819.BF factory_firmware
```

`source-changes` holds the patches against upstream and the PC tools. `source-changes/simulator` runs
the apps against a simulated radio, with no hardware needed:

```
cargo build --release --manifest-path source-changes/simulator/Cargo.toml --target x86_64-pc-windows-msvc --target-dir C:/t/sim
C:/t/sim/x86_64-pc-windows-msvc/release/sim.exe out-folder skull_logo.rgb565
```

The output folder has to exist. The simulator only checks logic, so it can't prove how a real radio
behaves on the air.

## Credits and license

- Firmware base: [bf-5rh-firmware-open](https://github.com/sophiel-meow/bf-5rh-firmware-open)
- BK4819 values and stock firmware from
  [amoxu/Baofeng-UV-5RM-5RH-RE](https://github.com/amoxu/Baofeng-UV-5RM-5RH-RE)
- Modem settings researched from the UV-K5 community's
  [messenger firmware](https://github.com/kamilsss655/uv-k5-firmware-custom)
- Licensed under GPL-3.0, the same as the firmware it's built on. See `LICENSE`.

Provided as is, with no warranty.
