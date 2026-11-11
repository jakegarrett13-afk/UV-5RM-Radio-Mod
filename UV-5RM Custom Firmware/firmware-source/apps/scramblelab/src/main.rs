//! SCRAMBLE LAB: a test bench for finding how this radio's RF chip scrambles voice.
//!
//! It shows the scramble register (0x31), the scramble carrier (0x71) and the TX
//! deviation (0x40), read back from the chip, and keeps re-applying the chosen bits
//! every tick, so they also hold while the firmware keys the transmitter. Press PTT
//! to transmit as usual; receive works as usual too. Exiting puts all three
//! registers back how they were. Talks to the chip over the same pins as TEXT.

#![no_std]
#![no_main]

use bf5rh_abi::{Api, AppEvent, AppResult};

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

const KIND_SINGLE: u8 = 2;
const KEY_MENU: u8 = 12;
const KEY_EXIT: u8 = 13;
const KEY_UP: u8 = 14;
const KEY_DOWN: u8 = 15;

const R_SCRAMBLE: u8 = 0x31;
const R_DEVIATION: u8 = 0x40;
const R_CARRIER: u8 = 0x71;

/// The eight scramble carriers in the factory firmware for the NRF version.
const CARRIERS: [u16; 8] = [0x6CE4, 0x70EC, 0x74F5, 0x78FD, 0x7D06, 0x810E, 0x8517, 0x891F];
/// Deviation choices for the low 12 bits: leave alone, the factory's scramble value,
/// the factory's normal value.
const DEVIATIONS: [u16; 3] = [0, 0x450, 0x4E0];

const BLACK: u16 = 0x0000;
const WHITE: u16 = 0xFFFF;
const GRAY: u16 = 0x8410;
const RED: u16 = 0xF800;
const GREEN: u16 = 0x07E0;
const F6: u8 = 1;

struct State {
    orig: [u16; 3],
    force_on: u16,
    force_off: u16,
    bit: u8,
    carrier: u8,
    dev: u8,
    can_tx: bool,
    seen: [u16; 3],
    tx: bool,
    drawn: Option<(u16, u16, u8, u8, u8, [u16; 3], bool)>,
}

static mut ST: State = State {
    orig: [0; 3],
    force_on: 0x0002,
    force_off: 0,
    bit: 1,
    carrier: 0,
    dev: 0,
    can_tx: false,
    seen: [0; 3],
    tx: false,
    drawn: None,
};

static mut API_PTR: *const Api = core::ptr::null();

fn st() -> &'static mut State {
    unsafe { &mut *core::ptr::addr_of_mut!(ST) }
}

#[no_mangle]
pub extern "C" fn app_entry(api: &Api, ev: AppEvent) -> AppResult {
    unsafe { API_PTR = api as *const Api };
    match ev {
        AppEvent::Enter => enter(api),
        AppEvent::Key { id, kind } => key(api, id, kind),
        AppEvent::Tick { .. } => tick(api),
        AppEvent::Draw => draw(api),
        AppEvent::Leave => {
            restore();
            AppResult::Continue
        }
    }
}

fn enter(api: &Api) -> AppResult {
    let s = st();
    (api.enter_rx)();
    s.orig = [bus::read(R_SCRAMBLE), bus::read(R_CARRIER), bus::read(R_DEVIATION)];
    s.force_on = 0x0002;
    s.force_off = 0;
    s.bit = 1;
    s.carrier = 0;
    s.dev = 0;
    s.drawn = None;
    // Never key a receive-only channel: an app's transmit skips that check.
    let shown = (api.settings_get)(8);
    let mut r = [0u8; 32];
    s.can_tx = !(shown > 0 && (api.chan_read)((shown - 1) as u16, r.as_mut_ptr(), 32) && r[4..8] == [0xFF; 4]);
    (api.set_tx_enabled)(s.can_tx);
    apply(s);
    AppResult::Continue
}

fn restore() {
    let s = st();
    bus::write(R_SCRAMBLE, s.orig[0]);
    bus::write(R_CARRIER, s.orig[1]);
    bus::write(R_DEVIATION, s.orig[2]);
}

fn apply(s: &mut State) {
    let r31 = bus::read(R_SCRAMBLE);
    let want = (r31 | s.force_on) & !s.force_off;
    if want != r31 {
        bus::write(R_SCRAMBLE, want);
    }
    let carrier = CARRIERS[s.carrier as usize];
    if bus::read(R_CARRIER) != carrier {
        bus::write(R_CARRIER, carrier);
    }
    if s.dev != 0 {
        let r40 = bus::read(R_DEVIATION);
        let want = (r40 & 0xF000) | DEVIATIONS[s.dev as usize];
        if want != r40 {
            bus::write(R_DEVIATION, want);
        }
    }
    s.seen = [bus::read(R_SCRAMBLE), bus::read(R_CARRIER), bus::read(R_DEVIATION)];
}

fn tick(api: &Api) -> AppResult {
    let s = st();
    s.tx = (api.tx_state)() & 1 != 0;
    apply(s);
    AppResult::Continue
}

fn key(api: &Api, id: u8, kind: u8) -> AppResult {
    if kind != KIND_SINGLE {
        return AppResult::Continue;
    }
    let s = st();
    let mask = 1u16 << s.bit;
    match id {
        KEY_UP => s.bit = (s.bit + 1) % 16,
        KEY_DOWN => s.bit = (s.bit + 15) % 16,
        // Each press: free -> forced on -> forced off -> free.
        KEY_MENU => {
            if s.force_on & mask != 0 {
                s.force_on &= !mask;
                s.force_off |= mask;
            } else if s.force_off & mask != 0 {
                s.force_off &= !mask;
            } else {
                s.force_on |= mask;
            }
        }
        2 => s.carrier = (s.carrier + 1) % CARRIERS.len() as u8,
        3 => s.dev = (s.dev + 1) % DEVIATIONS.len() as u8,
        KEY_EXIT => {
            (api.set_tx_enabled)(false);
            restore();
            return AppResult::Exit;
        }
        _ => {}
    }
    apply(s);
    AppResult::Continue
}

// ui

struct Line {
    buf: [u8; 26],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Line { buf: [b' '; 26], len: 0 }
    }
    fn push(&mut self, s: &[u8]) {
        for &b in s {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
    }
    fn hex(&mut self, v: u16) {
        for shift in [12, 8, 4, 0] {
            self.push(&[b"0123456789ABCDEF"[(v >> shift) as usize & 15]]);
        }
    }
    fn num(&mut self, v: u8) {
        if v >= 10 {
            self.push(&[b'0' + v / 10]);
        }
        self.push(&[b'0' + v % 10]);
    }
}

fn text(api: &Api, y: i16, l: &Line, fg: u16) {
    (api.draw_text)(4, y, l.buf.as_ptr(), l.len as u16, fg, BLACK, F6);
}

fn draw(api: &Api) -> AppResult {
    let s = st();
    let snap = (s.force_on, s.force_off, s.bit, s.carrier, s.dev, s.seen, s.tx);
    if s.drawn == Some(snap) {
        return AppResult::Continue;
    }
    s.drawn = Some(snap);
    (api.fill_rect)(0, 0, 160, 128, BLACK);

    let mut l = Line::new();
    l.push(b"SCRAMBLE LAB");
    text(api, 2, &l, WHITE);
    let mut l = Line::new();
    l.push(if s.tx { b"TRANSMITTING" } else if s.can_tx { b"RECEIVING" } else { b"RX ONLY: NO TX" });
    (api.draw_text)(88, 2, l.buf.as_ptr(), l.len as u16, if s.tx { RED } else { GREEN }, BLACK, F6);

    let mut l = Line::new();
    l.push(b"R31 ");
    l.hex(s.seen[0]);
    l.push(b"  SCRAMBLE ");
    l.push(if s.seen[0] & 2 != 0 { b"ON" } else { b"OFF" });
    text(api, 18, &l, WHITE);

    let mut l = Line::new();
    for b in (0..16).rev() {
        l.push(if s.seen[0] >> b & 1 != 0 { b"1" } else { b"0" });
    }
    text(api, 30, &l, GRAY);
    // A caret under the selected bit (MSB first).
    let x = 4 + (15 - s.bit as i16) * 6;
    (api.draw_text)(x, 39, b"^".as_ptr(), 1, RED, BLACK, F6);

    let mut l = Line::new();
    l.push(b"BIT ");
    l.num(s.bit);
    l.push(b": ");
    let mask = 1u16 << s.bit;
    l.push(if s.force_on & mask != 0 { b"FORCED ON" } else if s.force_off & mask != 0 { b"FORCED OFF" } else { b"LEFT ALONE" });
    text(api, 50, &l, WHITE);

    let mut l = Line::new();
    l.push(b"R71 ");
    l.hex(s.seen[1]);
    l.push(b"  CARRIER ");
    l.num(s.carrier + 1);
    text(api, 64, &l, WHITE);

    let mut l = Line::new();
    l.push(b"R40 ");
    l.hex(s.seen[2]);
    l.push(match s.dev {
        0 => b"  DEV UNTOUCHED" as &[u8],
        1 => b"  DEV 0450",
        _ => b"  DEV 04E0",
    });
    text(api, 76, &l, WHITE);

    let mut l = Line::new();
    l.push(b"UP/DN BIT   MENU SET");
    text(api, 96, &l, GRAY);
    let mut l = Line::new();
    l.push(b"2 CARRIER  3 DEV  EXIT");
    text(api, 108, &l, GRAY);
    AppResult::Continue
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        let ptr = API_PTR;
        if !ptr.is_null() {
            ((*ptr).app_fault)(0);
        }
    }
    loop {}
}
