# Flashing the UV-5RM by hand

Step-by-step commands for flashing the custom firmware yourself, one radio at a time.

> **Read this first.** No copy of the factory firmware exists for this hardware. Once a stock
> radio is flashed, it can't go back, and CHIRP stops working with it. Only flash a radio whose
> ID screen says `5RHNRFB-YP07` (turn it on while holding **8** to check).

## What you need

- Windows PC with Python and pyserial: `py -m pip install pyserial`
- Baofeng programming cable, pushed **all the way** into the radio's jack
- A charged battery. A radio that dies mid-flash is the easiest way to get into trouble.

Every command below is run from the `UV-5RM Custom Firmware` folder, not the `Radio Program`
folder above it. A terminal opened in VS Code starts in `Radio Program`, so move down first:

```
cd "UV-5RM Custom Firmware"
```

The quotes are needed because of the spaces. If a command says `No such file or directory`,
you're in the wrong folder.

## 0. Find the COM port

```
py -m serial.tools.list_ports
```

Or look in Device Manager under **Ports (COM & LPT)**. The commands below use `COM3`; change it
if yours is different.

## 1. Back up the radio

Flash mode can't read the radio, so do this **before** step 2, with the radio on at its main
screen.

**Radio already running the custom firmware:**

```
py source-changes\backup_radio.py COM3 my-backup
```

This saves the channels, settings and the list of installed apps, then restarts the radio.

**Stock radio (first flash):**

```
mkdir my-backup
py source-changes\radio_read.py COM3 my-backup\factory_memory.img
```

If it doesn't answer, turn the radio off and on again and retry.

## 2. Enter flash mode

1. Turn the radio **off**.
2. Hold **both small side buttons** next to PTT (not PTT itself), and keep holding them.
3. Turn the radio **on**. The screen stays **black**. That's flash mode. You can let go.

Check that the bootloader is answering. This only sends a handshake; it writes nothing.

```
py source-changes\bootloader_probe.py COM3
```

You want to see `BOOTLOADER ANSWERED`. If you get `NO BOOTLOADER RESPONSE`, see
[Troubleshooting](#troubleshooting).

## 3. Flash the firmware

This is the step that writes. It takes under a minute. Don't touch the radio or the cable until
it finishes.

```
py source-changes\flash_bf.py UV-5RM_Custom_Firmware.BF COM3 e76b5a605732f523
```

The last argument is the start of the file's SHA-256. The tool refuses to send a file that
doesn't match it. You should see `file ok`, `handshake ok`, a package counter, then
**`FLASH COMPLETE`**. The radio restarts on its own.

If you built a new `.BF` yourself, get its checksum with
`certutil -hashfile UV-5RM_Custom_Firmware.BF SHA256` and use the first 16 characters.

## 4. Install the logo and apps

Wait for the radio to reach its main screen, then:

```
py source-changes\setup_radio.py COM3 skull_logo.rgb565 64 apps
```

It only writes what's different and reads everything back, so it's safe to run again. On a radio
that already had the custom firmware, the logo and apps survive a reflash, so this usually finds
nothing to change. Run it anyway to confirm.

## 5. Check the radio

- **Transmit:** `SETTINGS` → set `TXINH` to off. A freshly flashed stock radio starts with it on.
- **TX lock:** `SETTINGS` → `SYSTEM` → `FLOCK` (default `FCC HAM`).
- **Bands:** a radio flashed with this firmware for the first time only shows `ALL` and
  `CUSTOM RANGE` under **Band** until you write bands from `UV-5RM_Channels.html` (Chrome or Edge).
- **Key test:** receive on a known channel, key up briefly on low power, and open `TEXT`.

## Updating a single app (no flash needed)

With the radio on at its main screen, replace one app by name. The tool refuses if that slot
holds a different app.

```
py source-changes\replace_app.py COM3 skull_logo.rgb565 64 TEXT apps\text.app
```

## Troubleshooting

| Problem | What to try |
|---|---|
| `NO BOOTLOADER RESPONSE` | Make sure the screen is black (not booted normally). Power off, hold **both** side buttons firmly, power on. Reseat the cable; Baofeng jacks need a hard push. Check the COM port. |
| `could not open port` / access denied | Something else has the port: close CHIRP, the channel page, or another terminal. Unplug and replug the cable. |
| `file hash ... != ...` | The `.BF` file isn't the one you meant to flash, or the checksum was typed wrong. Nothing was written. |
| `FLASH FAILED` partway through | Don't panic, and don't unplug the battery to "fix" it. The bootloader isn't overwritten by this tool, so turn the radio off, go back to step 2 and run step 3 again. |
| Radio boots but the menu or apps are missing | Run step 4 again. |
| Backup in step 1 doesn't answer | Power-cycle the radio, make sure it's on its main screen (not a menu), and retry. |

## Flashing a second radio

Repeat steps 1–5 on each radio. Both radios need the same firmware and the same `TEXT` app to
message each other.
