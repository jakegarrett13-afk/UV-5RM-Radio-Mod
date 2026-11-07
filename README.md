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
| `5RHNRFB-YP07` | This is the right hardware, so carry on |
| Anything else, such as `...NRF` or `...BK` | **Don't flash it.** It may boot but not transmit or receive |

## What you need

- A Windows PC. The tools are Python, so other systems may work, but they're untested.
- [Python](https://www.python.org/downloads/), plus the required libraries:
  `py -m pip install pyserial pillow`
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

**2. Enter flash mode.** Turn the radio off, hold **both side buttons** and turn it on. The screen
stays black. Check that it answers (this doesn't write anything):

```
py source-changes\bootloader_probe.py COM3
```

**3. Flash.** This is the point of no return. It takes under a minute, so don't touch the radio or
the cable. The long number is a checksum, and the tool refuses a file that doesn't match it.

```
py source-changes\flash_bf.py UV-5RM_Custom_Firmware.BF COM3 4340f205430f0741
```

**4. Install the logo and apps.** Wait for the radio to restart to its main screen. Choose one of
the included 160x128 RGB565 logos; the command reads everything back to confirm it.

| File | Design |
|---|---|
| `skull_logo.rgb565` | Original skull logo |
| `OOT.rgb565` | The Legend of Zelda: Ocarina of Time logo |
| `zelda.rgb565` | White Zelda crest on black |

```
py source-changes\setup_radio.py COM3 skull_logo.rgb565 64 apps
```

Replace `skull_logo.rgb565` in that command with `OOT.rgb565` or `zelda.rgb565` to install a
different bundled logo.

**5. Turn on transmit.** It starts off as a safety step. Go to `SETTINGS` and set `TXINH` to off.

**6. Load channels (optional).** Open `UV-5RM_Channel_Uploader.html` in Chrome or Edge. The channel
list in this repo is for Clark County, Nevada, so check that it suits your area. Your radio should
keep whatever channels it already had.

To choose which channels get scanned, open `UV-5RM_Scan_Builder.html`, tick the ones you want, and
load the CSV it saves into the uploader.

**7. Set a start-up tune (optional).** The unified tune tool supports named presets:

| Preset | Also accepted | Length | Notes |
|---|---|---:|---|
| `doom` | *(default when omitted)* | 2.43 s | Doom-style riff |
| `zelda` | | 6.12 s | Opening phrase from the Legend of Zelda NES title theme |
| `song_of_storms` | `storms` | 7.14 s | Song of Storms melody |
| `mario` | | 2.04 s | Sequence from the older radio-two Mario script |

Run the command from the `UV-5RM Custom Firmware` folder, replacing `COM3` and the preset as needed:

```
py source-changes\set_boot_tune.py COM3 my-backup\boot-tune-before.bin zelda
```

The second argument must be a **file name**, not a directory such as `.`. Its parent directory must
already exist. The script saves the radio's 168-byte settings record there before writing the tune,
then reads the settings back to verify that nothing else changed. If you omit the preset, it uses
`doom`. Enable the radio's boot sound in `SETTINGS` if it is currently off.

`set_radio2_mario_tune.py` is retained as a reference; use the unified command above with `mario`
for normal use.

**8. Install a custom boot logo (optional).** To switch to one of the bundled RGB565 logos later,
save the currently installed logo and write the selected file with:

```
py source-changes\set_boot_logo.py restore COM3 zelda.rgb565 --current-backup my-logo-backup.rgb565
```

Use `OOT.rgb565` instead to install the Ocarina of Time design. To create your own logo, images are
fitted to the 160x128 display, composited over black, converted to the radio's RGB565 format, and
previewed after color conversion. Preview an image locally without touching the radio:

```
py source-changes\set_boot_logo.py preview photo.png --preview logo-preview.png --fit contain
```

When the preview looks right, upload it while saving the currently installed logo first:

```
py source-changes\set_boot_logo.py upload COM3 photo.png logo-backup.rgb565 --preview logo-preview.png
```

The uploader skips an identical logo and reads the entire write back for verification. If
verification fails, it restores the original automatically. A backup can also be restored later:

```
py source-changes\set_boot_logo.py restore COM3 logo-backup.rgb565
```

Keep the `.rgb565` backup: it is the exact logo previously read from your radio. Backup files named
`*-backup.rgb565` are ignored by Git so they are not accidentally committed.

Set `SETTINGS` -> `BOOTSCR` to `LOGO`. The logo is also dimmed behind menus, so a dark background
and a centered subject usually keep menu text easiest to read. Use `--fit cover` to crop and fill the
screen or `--fit stretch` to ignore the source aspect ratio.

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
- Modem settings researched from the UV-K5 community's
  [messenger firmware](https://github.com/kamilsss655/uv-k5-firmware-custom)
- Licensed under GPL-3.0, the same as the firmware it's built on. See `LICENSE`.

Provided as is, with no warranty.
