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
| **TEXT** | Sends messages up to 50 characters, retries until confirmed, saves them, and plays a tone when one arrives. Ten quick messages send without typing, and you can change them from the PC |
| **Bands** | Press Band to pick a set of channels to use and scan, like Weather, Air or your own. Make up to 16 on the PC, or type a custom range |
| **BAND SWEEP** | Sweeps a whole band for activity |
| **RPT FINDER** | Works out a repeater's input frequency and tone |
| **FOX HUNT** | Signal meter and rough distance estimate for finding a transmitter |
| **SEARCH/QT SCAN** | Finds a nearby signal's frequency, or the tone on your channel; press 1 to switch |
| **CALL button** | Opens `TEXT`; press it again to go back to the radio |
| **Settings** | Split into `CURRENT CHANNEL` (saved to that channel) and `ALL CHANNELS`, with adjustable mic gain |
| **Roger tones** | `ROGER`, `MDC1200`, plus `COIN`, `MORSE K` (Morse for "over") and `CHIRP` |
| **Look and feel** | Skull start-up logo with a CRT static effect, black and red menus, a custom start-up tune and a lower key beep |

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

**2. Enter flash mode.** Turn the radio off, hold **both side buttons** and turn it on. The screen
stays black. Check that it answers (this doesn't write anything):

```
py source-changes\bootloader_probe.py COM3
```

**3. Flash.** This is the point of no return. It takes under a minute, so don't touch the radio or
the cable. The long number is a checksum, and the tool refuses a file that doesn't match it.

```
py source-changes\flash_bf.py UV-5RM_Custom_Firmware.BF COM3 fb160fe76849a819
```

**4. Install the logo and apps.** Wait for the radio to restart to its main screen. This reads
everything back to confirm it.

```
py source-changes\setup_radio.py COM3 skull_logo.rgb565 64 apps
```

**5. Turn on transmit.** It starts off as a safety step. Go to `SETTINGS` and set `TXINH` to off.

**6. Set up bands.** Your radio keeps the channels it already had, but it has no bands until you
write them from the PC. Until then, `Band` only offers `ALL` and `CUSTOM RANGE`. See
[Programming channels](#programming-channels).

Optional extras: `set_boot_tune.py` sets a Doom-style start-up riff, and `set_radio2_mario_tune.py`
sets a Super Mario one but only runs on one specific radio, so copy and edit it for yours.

## Programming channels

Open `UV-5RM_Channels.html` in Chrome or Edge. Other browsers can't reach the cable. With the radio on
its main screen, pick where to start:

| Start from | Use it to |
|---|---|
| **Read my radio** | Change the channels and bands already on it. A radio with no bands yet starts with the Clark County ones |
| **Clark County list** | Load the list in this repo, for Clark County, Nevada, with its 11 bands. Check it suits your area |
| **Open a file** | Reopen a list you saved, or a backup made while writing |

The page lists your channels under each band. A **band** is a set of channels, like Weather or Air:
on the radio, press **Band** to pick one, then hold **#** to scan it. `ALL` is always there.

- **Make a band:** click **New band**, and give it a name for the Band menu and a short name for the
  main screen. The radio holds up to 16.
- **Add a channel:** click **Add channel** on a band's bar to add it to that band, or at the top.
- **Change a band:** **Edit** renames it. **Delete** removes it, and you choose whether its channels
  stay on the radio.
- **Edit a channel:** click it. Its band tickboxes put it in any number of bands. `TX tone` and
  `RX tone` work like the radio's own menus. **✕** deletes it, and **Undo** brings it back.
- **Choose what scans:** a band scans its channels that have **Scan** ticked. Tick a band's bar to tick
  all of them.
- **Write to radio:** shows what will change before anything is written. A backup of the radio's
  channels downloads first, and the radio restarts when it's done.
- **Save to file:** keeps your channels and bands to reopen later.

The radio then ends up with exactly the channels and bands in the list. Channels that are on the radio
but not in the list get removed, and the check before writing shows them.

**Quick messages:** click **Quick messages**, on the start screen or at the top of the page. **Read from
radio** shows the ones on your radio, and **Write to radio** saves your changes and checks them. Each
is up to 50 characters, and letters become capitals. **Use built-in messages** puts back the ones the
radio came with. Writing them doesn't touch your channels.

## Settings

`SETTINGS` splits the old `RADIO` menu in two, so it's clear what a change affects:

| Menu | What's in it | Saved where |
|---|---|---|
| `CURRENT CHANNEL` | `W/N`, `PWR`, `RX TONE`, `TX TONE`, `SHIFT`, `OFFSET`, `RX SCRAMBLE`, `BUSY LOCK`, `SCAN` | On the channel you're on, when you press Menu to confirm |
| `ALL CHANNELS` | `SQL`, `MIC GAIN`, `STEP`, `TOT`, `TDR`, `SAVE`, `TXINH`, `SCANMD`, `RIT`, `CH DISP` | One value for the whole radio |

- **Confirm to save:** scrolling through values only previews them. Menu saves, and Exit puts the old
  value back.
- **Receive-only channels:** these keep their blank transmit frequency, so `SHIFT` and `OFFSET` don't
  change them.
- **`SCAN`:** shows `N/A` in frequency mode.
- **`MIC GAIN`:** `DEFAULT` is the louder level we settled on by ear. The numbers 0 to 127 don't work
  like a volume knob, so if you change it, check how you sound on another radio. Pressing 0 while
  editing returns to `DEFAULT`. The deviation at `DEFAULT` hasn't been measured, so ask someone how
  you sound before relying on it.
- **`RX SCRAMBLE`:** this only unscrambles received audio, from radios using standard voice-inversion
  scramble groups 1 to 8. This chip doesn't scramble what it transmits, so it gives you no privacy.
- **CALL button:** set to `TEXT` under `KEYS` → `S1-SH`. If yours doesn't open `TEXT` after flashing,
  set it there.

## Using TEXT

Both radios need `TEXT` open, on the same simplex frequency. It uses the frequency the radio was on
when you opened it. Don't use a repeater. Press **CALL** to open it from the main screen, and press
**CALL** again to go straight back to the radio.

An incoming message plays a low then a high note (`boo BEE`). When the other radio confirms one of
yours, you hear the reverse (`BEE boo`). Messages never get a roger tone.

| Key | Action |
|---|---|
| Number keys | Type, flip-phone style; the letters are shown at the top of the screen |
| **Menu** | Send |
| **Exit** | Delete a letter; stops retries while sending; closes the app when nothing is typed |
| Hold **Exit** | Close the app |
| **Up / Down** | Scroll saved messages |
| **VFO/MR** | Open the quick messages (see below) |
| **Band** | Send a numbered test ping |
| Hold **\*** | Lock or unlock the keys, as on the main screen |
| **A/B** | Switch between low (default) and high power |
| Hold **#** twice | Erase all saved messages |

**Quick messages:** press **VFO/MR** for a list of ten ready-made messages. Pick one with **Up/Down**
or its number key (**0** is the tenth), then press **Menu** to send it. **#** copies it into the
typing box instead, so you can add to it first, and **Exit** or **VFO/MR** closes the list. The radio
comes with these, which you can change from the PC (see [Programming channels](#programming-channels)):

`COPY THAT`, `YES`, `NO`, `ARE YOU OK?`, `ALL OK HERE`, `ON MY WAY`, `ARRIVED SAFE`, `WHERE ARE YOU?`,
`STAY PUT, I'M COMING TO YOU`, `EMERGENCY! NEED HELP NOW`

**Key lock:** while the keys are locked, the bottom line says so and only holding **\*** does
anything. Messages still arrive and are still confirmed, and **CALL** still goes back to the radio.

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

To fit the new features, the firmware turns seven app calls that none of the included apps use into
stubs: RF gain read and adjust, mic level, monitor, tone off, and the satellite apps' clock. They're
marked in `src/app/overlay.rs`. `firmware-source/apps/scramblelab` is a test bench for the RF chip's
scramble registers, kept for reference and not installed.

Bands are stored on the radio in the spare channel slots 1000 to 1015, and each channel's bands are a
16-bit mask in its record's bytes 17-18. `src/app/modes.rs` describes the format.

Quick messages are ten 64-byte records at the start of the app data area (0x194000): `Q`, a length,
then the text. A record without the `Q` reads as the built-in message in `textapp`. The page writes
them with the programming port's satellite-table command and reads them back with its raw flash read,
so they need no firmware change. To fit them in its 8 KB, `textapp` uses its own small byte-copy
routines in place of the library's faster but much larger ones.

`source-changes` holds the patches against upstream and the PC tools. `build_channels_page.py`
rebuilds `UV-5RM_Channels.html` from `channels_template.html`, taking its tone lists from the firmware. `source-changes/simulator` runs
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
