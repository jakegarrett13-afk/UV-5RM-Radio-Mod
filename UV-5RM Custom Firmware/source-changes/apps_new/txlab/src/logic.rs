//! TX LAB: change the RF chip's transmit-audio settings live, then key up with PTT
//! and have someone listen, to find what fixes muffled transmit audio.
//!
//! Nothing here is saved. Leaving puts every register back as found. Like TEXT, the
//! app talks to the chip over the firmware driver's three GPIO pins, since the
//! firmware has no calls for these registers. The firmware rewrites some of them on
//! every key-up and every return to receive, so the app checks them every 20 ms and
//! puts its own values back.

use bf5rh_abi::{Api, AppEvent, AppResult};

#[cfg(target_os = "none")]
mod bus {
    use core::ptr::{read_volatile, write_volatile};

    const A: usize = 0x4800_0000;
    const C: usize = 0x4800_0800;
    const CFGR: usize = 0x00;
    const PULL: usize = 0x0C;
    const IDT: usize = 0x10;
    const SCR: usize = 0x18;
    const CLR: usize = 0x28;
    const LE: u32 = 1 << 13; // PC13
    const SCK: u32 = 1 << 2; // PA2
    const SDA: u32 = 1 << 3; // PA3
    const SDA_FIELD: u32 = 3 << 6;

    fn set(port: usize, bit: u32, high: bool) {
        let reg = port + if high { SCR } else { CLR };
        unsafe { write_volatile(reg as *mut u32, bit) };
    }

    /// At least the driver's 1 us at 120 MHz.
    fn pause() {
        for _ in 0..40 {
            unsafe { core::arch::asm!("nop") };
        }
    }

    fn bits(byte: u8) {
        let mut mask = 0x80u8;
        while mask != 0 {
            set(A, SCK, false);
            set(A, SDA, byte & mask != 0);
            pause();
            set(A, SCK, true);
            pause();
            mask >>= 1;
        }
    }

    fn sda_output(output: bool) {
        unsafe {
            let cfg = read_volatile((A + CFGR) as *const u32) & !SDA_FIELD;
            write_volatile((A + CFGR) as *mut u32, cfg | if output { 1 << 6 } else { 0 });
            let pull = read_volatile((A + PULL) as *const u32) & !SDA_FIELD;
            write_volatile((A + PULL) as *mut u32, pull | if output { 0 } else { 1 << 6 });
        }
    }

    pub fn write(addr: u8, value: u16) {
        set(C, LE, false);
        pause();
        bits(addr & 0x7F);
        bits((value >> 8) as u8);
        bits(value as u8);
        set(C, LE, true);
        pause();
        set(A, SCK, false);
    }

    pub fn read(addr: u8) -> u16 {
        set(C, LE, false);
        pause();
        bits(addr | 0x80);
        set(A, SCK, false);
        pause();
        sda_output(false);
        pause();
        let mut data = 0u16;
        for _ in 0..16 {
            data <<= 1;
            set(A, SCK, true);
            if unsafe { read_volatile((A + IDT) as *const u32) } & SDA != 0 {
                data |= 1;
            }
            pause();
            set(A, SCK, false);
            pause();
        }
        sda_output(true);
        set(C, LE, true);
        pause();
        set(A, SDA, false);
        set(A, SCK, false);
        data
    }
}

#[cfg(not(target_os = "none"))]
mod bus {
    pub fn write(addr: u8, value: u16) {
        crate::sim_rf_write(addr, value);
    }
    pub fn read(addr: u8) -> u16 {
        crate::sim_rf_read(addr)
    }
}

const KIND_SINGLE: u8 = 2;
const KIND_REPEAT: u8 = 4;
const KEY_0: u8 = 0;
const KEY_STAR: u8 = 10;
const KEY_POUND: u8 = 11;
const KEY_MENU: u8 = 12;
const KEY_EXIT: u8 = 13;
const KEY_UP: u8 = 14;
const KEY_DOWN: u8 = 15;

const BLACK: u16 = 0x0000;
const WHITE: u16 = 0xFFFF;
const GRAY: u16 = 0x8410;
const GREEN: u16 = 0x07E0;
const RED: u16 = 0xF800;
const TITLE_BG: u16 = 0x5800;
const SELECT_BG: u16 = 0x8000;
const YELLOW: u16 = 0xFFE0;

const F5: u8 = 0; // 5x8
const F6: u8 = 1; // 6x10
const SCREEN_W: u16 = 160;

const R_FILTER: u8 = 0x2B;
const R_DEV: u8 = 0x40;
const R_TX3K: u8 = 0x74;
const R_MIC: u8 = 0x7D;
const R_DC: u8 = 0x7E;

/// REG 0x2B: each bit turns a filter off. TX: 0 pre-emphasis, 1 low-pass, 2 300 Hz
/// high-pass. RX: 8 de-emphasis, 9 3 kHz low-pass.
const OFF_PRE: u16 = 1 << 0;
const OFF_TX_LPF: u16 = 1 << 1;
const OFF_TX_HPF: u16 = 1 << 2;
const OFF_DEEMPH: u16 = 1 << 8;
const OFF_RX_LPF: u16 = 1 << 9;
const FILTER_BITS: u16 = OFF_PRE | OFF_TX_LPF | OFF_TX_HPF | OFF_DEEMPH | OFF_RX_LPF;

/// REG 0x74 words for the TX 3 kHz corner, from the firmware's AF trim table,
/// ordered -3 dB .. +4 dB. Index 3 (0 dB) is what the firmware writes at start-up.
const TX3K: [u16; 8] = [0xFEF0, 0xFCFA, 0xFA02, 0xF50B, 0xE61C, 0xDF22, 0xD42D, 0xCC35];
const TX3K_FLAT: u8 = 3;

const CHECK_100US: u32 = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Mic,
    Dev,
    Pre,
    TxLpf,
    TxHpf,
    Tx3k,
    MicDc,
    Deemph,
    RxLpf,
}

const ROWS: [Row; 9] = [
    Row::Mic,
    Row::Dev,
    Row::Pre,
    Row::TxLpf,
    Row::TxHpf,
    Row::Tx3k,
    Row::MicDc,
    Row::Deemph,
    Row::RxLpf,
];

#[derive(Clone, Copy)]
struct Values {
    mic: u8,      // REG 0x7D low 7 bits
    dev: u16,     // REG 0x40 low 12 bits
    pre: bool,    // on = filter in use
    tx_lpf: bool,
    tx_hpf: bool,
    tx3k: u8,     // index into TX3K
    mic_dc: u8,   // REG 0x7E bits 5:3
    deemph: bool,
    rx_lpf: bool,
}

struct State {
    sel: usize,
    now: Values,
    found: Values,
    saved: [u16; 5],
    since_check: u32,
    was_tx: bool,
    refused: bool,
    dirty: bool,
}

static mut STATE: State = State {
    sel: 0,
    now: ZERO,
    found: ZERO,
    saved: [0; 5],
    since_check: 0,
    was_tx: false,
    refused: false,
    dirty: true,
};

const ZERO: Values = Values {
    mic: 0,
    dev: 0,
    pre: true,
    tx_lpf: true,
    tx_hpf: true,
    tx3k: TX3K_FLAT,
    mic_dc: 0,
    deemph: false,
    rx_lpf: false,
};

const SAVED_REGS: [u8; 5] = [R_FILTER, R_DEV, R_TX3K, R_MIC, R_DC];

fn st() -> &'static mut State {
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}

pub fn entry(api: &Api, ev: AppEvent) -> AppResult {
    match ev {
        AppEvent::Enter => enter(api),
        AppEvent::Key { id, kind } => key(api, id, kind),
        AppEvent::Tick { dt_100us } => tick(api, dt_100us),
        AppEvent::Draw => draw(api),
        AppEvent::Leave => {
            restore();
            AppResult::Continue
        }
    }
}

fn enter(api: &Api) -> AppResult {
    let s = st();
    let hz = (api.get_master_freq)();
    let wide = (api.get_master_wide)();
    (api.set_modulation)(0);
    (api.set_rx_freq)(hz);
    (api.set_tx_freq)(hz);
    (api.set_bandwidth)(wide);
    (api.set_power)(1);
    (api.set_subaudio_rx)(0);
    (api.set_subaudio_tx)(0);
    (api.set_sql_level)((api.settings_get)(0) as u8);
    (api.enter_rx)();
    (api.set_tx_enabled)(true);
    (api.set_backlight_hold)(true);

    for (slot, &reg) in s.saved.iter_mut().zip(SAVED_REGS.iter()) {
        *slot = bus::read(reg);
    }
    let [filter, dev, tx3k, mic, dc] = s.saved;
    s.found = Values {
        mic: (mic & 0x7F) as u8,
        dev: dev & 0x0FFF,
        pre: filter & OFF_PRE == 0,
        tx_lpf: filter & OFF_TX_LPF == 0,
        tx_hpf: filter & OFF_TX_HPF == 0,
        tx3k: TX3K.iter().position(|&w| w == tx3k).unwrap_or(TX3K_FLAT as usize) as u8,
        mic_dc: ((dc >> 3) & 7) as u8,
        deemph: filter & OFF_DEEMPH == 0,
        rx_lpf: filter & OFF_RX_LPF == 0,
    };
    // The firmware only writes the mic register on key-up, with the MIC GAIN
    // setting; before the first key-up it still holds the start-up value.
    s.now = s.found;
    s.sel = 0;
    s.since_check = 0;
    s.was_tx = false;
    s.refused = false;
    s.dirty = true;
    apply();
    AppResult::Continue
}

/// Writes `value` into the `mask` bits of `reg`, only if they differ.
fn ensure(reg: u8, mask: u16, value: u16) {
    let cur = bus::read(reg);
    let new = (cur & !mask) | (value & mask);
    if new != cur {
        bus::write(reg, new);
    }
}

fn apply() {
    let v = st().now;
    let mut off = 0;
    if !v.pre {
        off |= OFF_PRE;
    }
    if !v.tx_lpf {
        off |= OFF_TX_LPF;
    }
    if !v.tx_hpf {
        off |= OFF_TX_HPF;
    }
    if !v.deemph {
        off |= OFF_DEEMPH;
    }
    if !v.rx_lpf {
        off |= OFF_RX_LPF;
    }
    ensure(R_FILTER, FILTER_BITS, off);
    ensure(R_DEV, 0x0FFF, v.dev);
    ensure(R_TX3K, 0xFFFF, TX3K[v.tx3k as usize]);
    ensure(R_MIC, 0x007F, v.mic as u16);
    ensure(R_DC, 0x0038, (v.mic_dc as u16) << 3);
}

fn restore() {
    let s = st();
    for (&value, &reg) in s.saved.iter().zip(SAVED_REGS.iter()) {
        bus::write(reg, value);
    }
}

fn tick(api: &Api, dt_100us: u32) -> AppResult {
    let s = st();
    let state = (api.tx_state)();
    let tx = state & 1 != 0;
    s.since_check += dt_100us;
    if tx != s.was_tx || s.since_check >= CHECK_100US {
        s.since_check = 0;
        apply();
    }
    if tx != s.was_tx {
        s.was_tx = tx;
        s.dirty = true;
    }
    let refused = state & 2 != 0;
    if refused != s.refused {
        s.refused = refused;
        s.dirty = true;
    }
    AppResult::Continue
}

fn step(v: &mut Values, row: Row, up: bool) {
    fn num<T: Copy + PartialOrd + core::ops::Add<Output = T> + core::ops::Sub<Output = T>>(
        x: &mut T,
        by: T,
        lo: T,
        hi: T,
        up: bool,
    ) {
        if up {
            if *x <= hi - by {
                *x = *x + by;
            } else {
                *x = hi;
            }
        } else if *x >= lo + by {
            *x = *x - by;
        } else {
            *x = lo;
        }
    }
    match row {
        Row::Mic => num(&mut v.mic, 4, 0, 127, up),
        Row::Dev => num(&mut v.dev, 100, 200, 3000, up),
        Row::Pre => v.pre = !v.pre,
        Row::TxLpf => v.tx_lpf = !v.tx_lpf,
        Row::TxHpf => v.tx_hpf = !v.tx_hpf,
        Row::Tx3k => num(&mut v.tx3k, 1, 0, 7, up),
        Row::MicDc => num(&mut v.mic_dc, 1, 0, 7, up),
        Row::Deemph => v.deemph = !v.deemph,
        Row::RxLpf => v.rx_lpf = !v.rx_lpf,
    }
}

fn reset_row(s: &mut State, row: Row) {
    let f = s.found;
    let v = &mut s.now;
    match row {
        Row::Mic => v.mic = f.mic,
        Row::Dev => v.dev = f.dev,
        Row::Pre => v.pre = f.pre,
        Row::TxLpf => v.tx_lpf = f.tx_lpf,
        Row::TxHpf => v.tx_hpf = f.tx_hpf,
        Row::Tx3k => v.tx3k = f.tx3k,
        Row::MicDc => v.mic_dc = f.mic_dc,
        Row::Deemph => v.deemph = f.deemph,
        Row::RxLpf => v.rx_lpf = f.rx_lpf,
    }
}

fn key(_api: &Api, id: u8, kind: u8) -> AppResult {
    if kind != KIND_SINGLE && kind != KIND_REPEAT {
        return AppResult::Continue;
    }
    let s = st();
    let row = ROWS[s.sel];
    match id {
        KEY_UP => s.sel = (s.sel + ROWS.len() - 1) % ROWS.len(),
        KEY_DOWN => s.sel = (s.sel + 1) % ROWS.len(),
        KEY_POUND => step(&mut s.now, row, true),
        KEY_STAR => step(&mut s.now, row, false),
        KEY_0 => reset_row(s, row),
        KEY_MENU => s.now = s.found,
        KEY_EXIT if kind == KIND_SINGLE => {
            restore();
            return AppResult::Exit;
        }
        _ => return AppResult::Continue,
    }
    s.dirty = true;
    apply();
    AppResult::Continue
}

// ui

struct Buf {
    b: [u8; 26],
    n: usize,
}

impl Buf {
    fn new() -> Buf {
        Buf { b: [0; 26], n: 0 }
    }
    fn s(&mut self, t: &[u8]) {
        for &c in t {
            if self.n < self.b.len() {
                self.b[self.n] = c;
                self.n += 1;
            }
        }
    }
    fn u(&mut self, v: u32) {
        let mut tmp = [0u8; 10];
        let mut i = tmp.len();
        let mut v = v;
        loop {
            i -= 1;
            tmp[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        self.s(&tmp[i..]);
    }
    fn on_off(&mut self, on: bool) {
        self.s(if on { b"ON" } else { b"OFF" });
    }
}

fn text(api: &Api, x: i16, y: i16, s: &[u8], fg: u16, bg: u16, font: u8) {
    (api.draw_text)(x, y, s.as_ptr(), s.len() as u16, fg, bg, font);
}

const ROW_Y: i16 = 26;
const ROW_H: i16 = 10;
const VALUE_X: i16 = 100;

fn label(row: Row) -> &'static [u8] {
    match row {
        Row::Mic => b"MIC GAIN",
        Row::Dev => b"DEVIATION",
        Row::Pre => b"PRE-EMPHASIS",
        Row::TxLpf => b"TX LOW-PASS",
        Row::TxHpf => b"TX HIGH-PASS",
        Row::Tx3k => b"TX 3K TRIM",
        Row::MicDc => b"MIC DC FILTER",
        Row::Deemph => b"RX DE-EMPH",
        Row::RxLpf => b"RX LOW-PASS",
    }
}

fn value(row: Row, v: &Values, b: &mut Buf) {
    match row {
        Row::Mic => b.u(v.mic as u32),
        Row::Dev => b.u(v.dev as u32),
        Row::Pre => b.on_off(v.pre),
        Row::TxLpf => b.on_off(v.tx_lpf),
        Row::TxHpf => b.on_off(v.tx_hpf),
        Row::Tx3k => {
            let db = v.tx3k as i32 - TX3K_FLAT as i32;
            b.s(if db < 0 { b"-" } else if db > 0 { b"+" } else { b"" });
            b.u(db.unsigned_abs());
            b.s(b" DB");
        }
        Row::MicDc => b.u(v.mic_dc as u32),
        Row::Deemph => b.on_off(v.deemph),
        Row::RxLpf => b.on_off(v.rx_lpf),
    }
}

fn changed(row: Row, a: &Values, b: &Values) -> bool {
    let (mut x, mut y) = (Buf::new(), Buf::new());
    value(row, a, &mut x);
    value(row, b, &mut y);
    x.b[..x.n] != y.b[..y.n]
}

fn draw(api: &Api) -> AppResult {
    let s = st();
    if !s.dirty {
        return AppResult::Continue;
    }
    s.dirty = false;
    (api.fill_rect)(0, 0, SCREEN_W, 128, BLACK);
    (api.fill_rect)(0, 0, SCREEN_W, 12, TITLE_BG);
    text(api, 4, 1, b"TX AUDIO LAB", WHITE, TITLE_BG, F6);
    if s.was_tx {
        text(api, 136, 1, b"TX", RED, TITLE_BG, F6);
    }

    let mut f = [0u8; 12];
    let n = (api.fmt_freq)((api.get_master_freq)(), f.as_mut_ptr()) as usize;
    text(api, 4, 14, &f[..n.min(f.len())], WHITE, BLACK, F6);
    let wide: &[u8] = if (api.get_master_wide)() { b"WIDE" } else { b"NARROW" };
    text(api, 100, 14, wide, GRAY, BLACK, F6);

    for (i, &row) in ROWS.iter().enumerate() {
        let y = ROW_Y + i as i16 * ROW_H;
        let bg = if i == s.sel { SELECT_BG } else { BLACK };
        (api.fill_rect)(0, y, SCREEN_W, ROW_H as u16, bg);
        text(api, 4, y + 1, label(row), WHITE, bg, F5);
        let mut b = Buf::new();
        value(row, &s.now, &mut b);
        let fg = if changed(row, &s.now, &s.found) { YELLOW } else { GREEN };
        text(api, VALUE_X, y + 1, &b.b[..b.n], fg, bg, F5);
    }

    let hint: &[u8] = if s.refused {
        b"TX REFUSED: CHECK TXINH/FLOCK"
    } else {
        b"*/# CHANGE 0 UNDO MENU ALL"
    };
    text(api, 4, 118, hint, if s.refused { RED } else { GRAY }, BLACK, F5);
    AppResult::Continue
}

// Read-only views for the PC simulator; not part of the radio build.
#[cfg(not(target_os = "none"))]
pub fn debug_select(row: usize) {
    st().sel = row;
}
