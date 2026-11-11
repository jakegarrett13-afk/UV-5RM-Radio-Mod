use super::side_ops;
use super::{
    digit_value, scan, App, ChVfoMode, Mode, MAX_CHANNEL_NUM,
    NO_CHANNELS_HOLD_TICKS, STEP_LIST_DECI_HZ,
};
use crate::device::keypad::{KeyEvent, KeyEventKind, KeyId};
use crate::device::radio::{FreqRanges, SubAudio};
use crate::device::storage::Storage;
use crate::flash_map::{addr, Settings};
use cortex_m::peripheral::SYST;

/// Bands are made on the PC page. Band `i` (0-15) is a 32-byte record in
/// the spare channel slot `BAND_SLOT + i`: its name in bytes 0-15, a short
/// name for the status bar in 16-20, and `BAND_MARK` in byte 31 while it's
/// in use. The bands in use come first, with no gaps.
///
/// A channel is in band `i` when bit `i` of its record bytes 17-18 (the
/// middle of the otherwise unused decoder code) is set.
pub(crate) const BAND_SLOT: u16 = 1000;
pub(crate) const MAX_BANDS: u8 = 16;
const BAND_MARK: u8 = b'B';
pub(crate) const BAND_LABEL: usize = 21;
pub(crate) const BAND_NAME: usize = 16;

/// `chan_mode` values: 0 = ALL, 1..=16 = band 0-15, `CUSTOM` = a typed
/// frequency range.
const CUSTOM: u8 = MAX_BANDS + 1;

fn read_band(storage: &mut Storage, i: u8) -> Option<[u8; 32]> {
    let mut b = [0u8; 32];
    let at = addr::CHAN_ADDR + (BAND_SLOT + i as u16) as u32 * addr::CHAN_SIZE;
    storage.read_raw(at, &mut b);
    (b[31] == BAND_MARK).then_some(b)
}

pub(crate) fn band_count(storage: &mut Storage) -> u8 {
    let mut n = 0;
    while n < MAX_BANDS && read_band(storage, n).is_some() {
        n += 1;
    }
    n
}

/// The band's name and short name, or `None` for ALL, CUSTOM or a band
/// that no longer exists.
pub(crate) fn band_label(storage: &mut Storage, mode: u8) -> Option<[u8; BAND_LABEL]> {
    if mode == 0 || mode > MAX_BANDS {
        return None;
    }
    let b = read_band(storage, mode - 1)?;
    let mut label = [0u8; BAND_LABEL];
    label.copy_from_slice(&b[..BAND_LABEL]);
    Some(label)
}

/// Menu rows: ALL, each band, then CUSTOM RANGE.
pub(crate) fn menu_mode(count: u8, row: usize) -> u8 {
    if row <= count as usize {
        row as u8
    } else {
        CUSTOM
    }
}

pub(crate) fn menu_row(count: u8, mode: u8) -> usize {
    if mode == CUSTOM {
        count as usize + 1
    } else {
        mode as usize
    }
}

/// Text up to the first blank (0x00 or 0xFF) byte.
pub(crate) fn label_str(bytes: &[u8]) -> &str {
    let end = bytes.iter().position(|&c| c == 0 || c == 0xFF).unwrap_or(bytes.len());
    crate::ui::ascii_str(&bytes[..end])
}

const DEFAULT_RANGE_HZ: (u32, u32) = (144_000_000, 148_000_000);

/// `boot_text_line2` isn't shown in logo boot mode, so it carries the mode
/// and range across power cycles without changing the settings record size
/// that the settings app and CPS tools depend on.
const STORE_MAGIC: u8 = b'M';

pub(super) fn restore(settings: &Settings, storage: &mut Storage) -> (u8, u32, u32) {
    let b = &settings.boot_text_line2;
    if b[0] != STORE_MAGIC || b[1] > CUSTOM {
        return (0, DEFAULT_RANGE_HZ.0, DEFAULT_RANGE_HZ.1);
    }
    // A band deleted from the PC since last time falls back to ALL.
    let mode = if b[1] == 0 || b[1] == CUSTOM || band_label(storage, b[1]).is_some() {
        b[1]
    } else {
        0
    };
    let lo = u32::from_le_bytes([b[2], b[3], b[4], b[5]]);
    let hi = u32::from_le_bytes([b[6], b[7], b[8], b[9]]);
    if range_valid(lo, hi) {
        (mode, lo, hi)
    } else {
        (mode, DEFAULT_RANGE_HZ.0, DEFAULT_RANGE_HZ.1)
    }
}

fn persist(app: &mut App) {
    let b = &mut app.settings.boot_text_line2;
    b[0] = STORE_MAGIC;
    b[1] = app.chan_mode;
    b[2..6].copy_from_slice(&app.range_lo_hz.to_le_bytes());
    b[6..10].copy_from_slice(&app.range_hi_hz.to_le_bytes());
    app.save_settings();
}

fn range_valid(lo: u32, hi: u32) -> bool {
    let hw = FreqRanges::hardware();
    lo < hi && hw.allows(lo) && hw.allows(hi)
}

/// Whether channel `num` (a flash slot, from 0) is in the selected band.
pub(super) fn contains(app: &mut App, num: u16) -> bool {
    let mode = app.chan_mode;
    if mode == 0 || mode == CUSTOM {
        return true;
    }
    let mask = (app.storage.read_channel(num).decoder_code >> 8) as u16;
    mask & 1 << (mode - 1) != 0
}

pub(super) fn is_custom(mode: u8) -> bool {
    mode == CUSTOM
}

fn first_channel(app: &mut App) -> Option<u16> {
    (0..=MAX_CHANNEL_NUM).find(|&n| !app.storage.is_channel_empty(n) && contains(app, n))
}

pub(super) fn open_menu(app: &mut App) {
    let count = band_count(&mut app.storage);
    app.mode_menu_index = menu_row(count, app.chan_mode);
    app.input.clear();
    app.mode = Mode::ModeMenu;
}

fn open_range(app: &mut App) {
    app.input.clear();
    app.range_stage = 0;
    app.range_err = false;
    app.mode = Mode::RangeInput;
}

/// Two rising notes; the second climbs with the mode's place in the list.
fn play_mode_cue(app: &mut App, syst: &mut SYST) {
    let tones = [(88, 7), (60u8.wrapping_add(8u8.wrapping_mul(app.chan_mode)), 9)];
    app.radio.play_cue(syst, &tones);
}

fn select(app: &mut App, syst: &mut SYST, mode: u8, then_scan: bool) {
    app.chan_mode = mode;
    app.band_label = band_label(&mut app.storage, mode).unwrap_or([0; BAND_LABEL]);
    persist(app);
    app.mode = Mode::Standby;
    app.reset_key_idle();

    if is_custom(app.chan_mode) {
        play_mode_cue(app, syst);
        open_range(app);
        return;
    }

    let current = app.sides[app.master].channel_num;
    let keep = !app.storage.is_channel_empty(current) && contains(app, current);
    let target = if keep { Some(current) } else { first_channel(app) };
    match target {
        Some(num) => {
            side_ops::load_channel_num(app, num);
            app.save_channel_state();
        }
        None => app.no_channels_ticks = NO_CHANNELS_HOLD_TICKS,
    }
    app.sync_watching_to_master(syst);
    play_mode_cue(app, syst);
    if then_scan && target.is_some() {
        scan::enter(app, syst);
    }
}

pub(super) fn dispatch_menu(app: &mut App, syst: &mut SYST, ev: KeyEvent) {
    let count = band_count(&mut app.storage);
    let len = count as usize + 2;
    match (ev.kind, ev.key) {
        (KeyEventKind::Single | KeyEventKind::Repeat, KeyId::Up) => {
            app.mode_menu_index = (app.mode_menu_index + len - 1) % len;
        }
        (KeyEventKind::Single | KeyEventKind::Repeat, KeyId::Down) => {
            app.mode_menu_index = (app.mode_menu_index + 1) % len;
        }
        (KeyEventKind::Single, KeyId::Menu) => {
            select(app, syst, menu_mode(count, app.mode_menu_index), false);
        }
        // Hold MENU: pick the mode and start scanning it right away.
        (KeyEventKind::Long, KeyId::Menu) => {
            select(app, syst, menu_mode(count, app.mode_menu_index), true);
        }
        (KeyEventKind::Single, KeyId::Exit | KeyId::Band) => {
            app.mode = Mode::Standby;
            app.reset_key_idle();
        }
        _ => {}
    }
}

fn step_frequency_index(current: u8, up: bool) -> u8 {
    let len = STEP_LIST_DECI_HZ.len() as u8;
    if up {
        (current + 1) % len
    } else {
        (current + len - 1) % len
    }
}

fn commit_range_field(app: &mut App) {
    if !app.input.is_empty() {
        let hz = app.input.value() * 1000;
        if app.range_stage == 0 {
            app.range_lo_hz = hz;
        } else {
            app.range_hi_hz = hz;
        }
    }
    app.input.clear();
}

fn start_range_scan(app: &mut App, syst: &mut SYST) {
    let lo = app.range_lo_hz;
    persist(app);
    let s = &mut app.sides[app.master];
    s.vfo_chan = ChVfoMode::Vfo;
    s.rx_only = false;
    s.reversed = false;
    s.name = [0; 12];
    s.rx_freq_hz = lo;
    s.freq_dir = 0;
    s.offset_hz = 0;
    s.cfg.subaudio_rx = SubAudio::None;
    s.cfg.subaudio_tx = SubAudio::None;
    s.auto_modulation();
    s.refresh_cfg_freqs();
    app.save_vfo();
    app.mode = Mode::Standby;
    app.reset_key_idle();
    app.sync_watching_to_master(syst);
    scan::enter(app, syst);
}

pub(super) fn dispatch_range(app: &mut App, syst: &mut SYST, ev: KeyEvent) {
    if !matches!(ev.kind, KeyEventKind::Single | KeyEventKind::Repeat) {
        return;
    }
    if ev.kind == KeyEventKind::Single {
        if let Some(d) = digit_value(ev.key) {
            app.range_err = false;
            app.input.push(d);
            return;
        }
    }
    match ev.key {
        KeyId::Up | KeyId::Down => {
            let s = &mut app.sides[app.master];
            s.freq_step = step_frequency_index(s.freq_step, ev.key == KeyId::Up);
        }
        KeyId::Menu if ev.kind == KeyEventKind::Single => {
            commit_range_field(app);
            if app.range_stage == 0 {
                app.range_stage = 1;
            } else if range_valid(app.range_lo_hz, app.range_hi_hz) {
                start_range_scan(app, syst);
            } else {
                app.range_err = true;
                app.range_stage = 0;
                app.radio.play_cue(syst, &[(40, 20)]);
            }
        }
        KeyId::Exit if ev.kind == KeyEventKind::Single => {
            if app.input.is_empty() {
                app.mode = Mode::Standby;
                app.reset_key_idle();
            } else {
                app.input.clear();
            }
        }
        _ => {}
    }
}

impl<'a> App<'a> {
    pub fn chan_mode_index(&self) -> usize {
        self.chan_mode as usize
    }
    pub fn chan_mode_name(&self) -> &str {
        match self.chan_mode {
            0 => "ALL",
            CUSTOM => "CUSTOM RANGE",
            _ => label_str(&self.band_label[..BAND_NAME]),
        }
    }
    pub fn chan_mode_short(&self) -> &str {
        match self.chan_mode {
            0 => "ALL",
            CUSTOM => "RANGE",
            _ => label_str(&self.band_label[BAND_NAME..]),
        }
    }
    pub fn mode_menu_index(&self) -> usize {
        self.mode_menu_index
    }
    pub fn range_hz(&self) -> (u32, u32) {
        (self.range_lo_hz, self.range_hi_hz)
    }
    pub fn range_stage(&self) -> u8 {
        self.range_stage
    }
    pub fn range_err(&self) -> bool {
        self.range_err
    }
    pub fn range_step_deci_hz(&self) -> u32 {
        self.sides[self.master].step_deci_hz()
    }
    pub fn range_input_len(&self) -> usize {
        self.input.len
    }
    pub fn range_input_write(&self, w: &mut dyn core::fmt::Write) {
        self.input.write_display(3, w);
    }
}
