#![allow(dead_code, unused_variables, unused_imports)]
#[path = "../../../apps/airband/src/db.rs"]
pub mod db;
#[path = "../../../apps/airband/src/logic.rs"]
mod logic;
use bf5rh_abi::{Api, AppEvent, AppResult};
use db::Flash;
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_5X8, FONT_6X10, FONT_9X18},
        MonoTextStyleBuilder,
    },
    pixelcolor::{raw::RawU16, Rgb565, Rgb888},
    prelude::*,
    text::Text,
};
use std::cell::RefCell;

#[path = "../../../src/hal/crc.rs"]
pub(crate) mod crc;
mod hal {
    pub(crate) use crate::crc;
}
#[path = "../../../src/cps/frame.rs"]
mod cps_frames;

struct Sim {
    time: u32,
    flash: Vec<u8>,
    handoff: Vec<u8>,
    freq: u32,
    am: u8,
    busy: bool,
    tx: usize,
    writes: Vec<u32>,
    fail: bool,
    pixels: Vec<Rgb565>,
    rects: Vec<(i16, i16, u16, u16)>,
    tunes: usize,
    labels: Vec<String>,
}
impl Sim {
    fn new() -> Self {
        Self {
            time: 0,
            flash: vec![255; 0x200000],
            handoff: vec![],
            freq: 0,
            am: 0,
            busy: false,
            tx: 0,
            writes: vec![],
            fail: false,
            pixels: vec![Rgb565::BLACK; 160 * 128],
            rects: vec![],
            tunes: 0,
            labels: vec![],
        }
    }
}
thread_local! {static SIM:RefCell<Sim>=RefCell::new(Sim::new());}
struct Memory;
impl Flash for Memory {
    fn read(&self, a: u32, b: &mut [u8]) -> bool {
        sim_nor_read(a, b.as_mut_ptr(), b.len() as u32)
    }
    fn write(&self, a: u32, b: &[u8]) -> bool {
        sim_nor_write(a, b.as_ptr(), b.len() as u32)
    }
    fn erase(&self, a: u32) -> bool {
        sim_nor_erase_sector(a)
    }
}
extern "C" fn sim_uptime_100us() -> u32 {
    SIM.with(|s| s.borrow().time)
}
extern "C" fn sim_nor_read(a: u32, p: *mut u8, n: u32) -> bool {
    SIM.with(|s| {
        let s = s.borrow();
        let a = a as usize;
        let n = n as usize;
        if a + n > s.flash.len() {
            return false;
        }
        unsafe { std::ptr::copy_nonoverlapping(s.flash[a..].as_ptr(), p, n) };
        true
    })
}
extern "C" fn sim_nor_write(a: u32, p: *const u8, n: u32) -> bool {
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        let a = a as usize;
        let n = n as usize;
        if a < db::PREF_BASE as usize || a + n > 0x1f0000 {
            return false;
        }
        s.writes.push(a as u32);
        let data = unsafe { std::slice::from_raw_parts(p, n) };
        let count = if s.fail { n / 2 } else { n };
        for i in 0..count {
            s.flash[a + i] &= data[i];
        }
        !s.fail
    })
}
extern "C" fn sim_nor_erase_sector(a: u32) -> bool {
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        if a != db::PREF_BASE && a != db::PREF_BASE + 4096 {
            return false;
        }
        if s.fail {
            return false;
        }
        s.flash[a as usize..a as usize + 4096].fill(255);
        true
    })
}
extern "C" fn sim_set_rx_freq(v: u32) {
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        s.freq = v;
        s.tunes += 1;
    })
}
extern "C" fn sim_set_modulation(v: u8) {
    SIM.with(|s| s.borrow_mut().am = v)
}
extern "C" fn sim_set_tx_enabled(v: bool) {
    if v {
        SIM.with(|s| s.borrow_mut().tx += 1)
    }
}
extern "C" fn sim_set_ptt(v: bool) {
    if v {
        SIM.with(|s| s.borrow_mut().tx += 1)
    }
}
extern "C" fn sim_tone_on(_: u16) {
    SIM.with(|s| s.borrow_mut().tx += 1)
}
extern "C" fn sim_settings_get(_: u16) -> u32 {
    3
}
extern "C" fn sim_squelch_open() -> bool {
    SIM.with(|s| s.borrow().busy)
}
extern "C" fn sim_read_rssi() -> u16 {
    if sim_squelch_open() {
        100
    } else {
        20
    }
}
extern "C" fn sim_handoff_write(p: *const u8, n: u16) -> bool {
    if n > 64 {
        return false;
    }
    SIM.with(|s| {
        s.borrow_mut().handoff =
            unsafe { std::slice::from_raw_parts(p, n as usize) }.to_vec()
    });
    true
}
extern "C" fn sim_handoff_read(p: *mut u8, n: u16) -> u16 {
    SIM.with(|s| {
        let s = s.borrow();
        let n = (n as usize).min(s.handoff.len());
        unsafe { std::ptr::copy_nonoverlapping(s.handoff.as_ptr(), p, n) };
        n as u16
    })
}
extern "C" fn sim_fmt_u32(v: u32, p: *mut u8) -> u16 {
    let s = v.to_string();
    unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), p, s.len()) };
    s.len() as u16
}
struct Screen<'a>(&'a mut Vec<Rgb565>);
impl OriginDimensions for Screen<'_> {
    fn size(&self) -> Size {
        Size::new(160, 128)
    }
}
impl DrawTarget for Screen<'_> {
    type Color = Rgb565;
    type Error = std::convert::Infallible;
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Rgb565>>,
    {
        for Pixel(p, c) in pixels {
            if (0..160).contains(&p.x) && (0..128).contains(&p.y) {
                self.0[p.y as usize * 160 + p.x as usize] = c;
            }
        }
        Ok(())
    }
}
extern "C" fn sim_fill_rect(x: i16, y: i16, w: u16, h: u16, c: u16) {
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        s.rects.push((x, y, w, h));
        for yy in y as i32..y as i32 + h as i32 {
            for xx in x as i32..x as i32 + w as i32 {
                if (0..160).contains(&xx) && (0..128).contains(&yy) {
                    s.pixels[yy as usize * 160 + xx as usize] =
                        RawU16::new(c).into();
                }
            }
        }
    })
}
extern "C" fn sim_draw_text(
    x: i16,
    y: i16,
    p: *const u8,
    n: u16,
    fg: u16,
    bg: u16,
    font: u8,
) -> u16 {
    let b = unsafe { std::slice::from_raw_parts(p, n as usize) };
    let text = std::str::from_utf8(b).unwrap();
    let font = match font {
        2 => &FONT_9X18,
        1 => &FONT_6X10,
        _ => &FONT_5X8,
    };
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        s.labels.push(text.to_owned());
        Text::new(
            text,
            Point::new(x as i32, y as i32 + font.baseline as i32),
            MonoTextStyleBuilder::new()
                .font(font)
                .text_color(RawU16::new(fg).into())
                .background_color(RawU16::new(bg).into())
                .build(),
        )
        .draw(&mut Screen(&mut s.pixels))
        .unwrap();
    });
    (x as i32 + font.character_size.width as i32 * n as i32) as u16
}
include!(concat!(env!("OUT_DIR"), "/api.rs"));
fn reset() {
    SIM.with(|s| {
        let mut v = Sim::new();
        let blob = include_bytes!("../fixtures/test.db");
        v.flash[db::BASE as usize..db::BASE as usize + blob.len()]
            .copy_from_slice(blob);
        *s.borrow_mut() = v;
    })
}
fn key(api: &Api, id: u8) -> AppResult {
    logic::entry(api, AppEvent::Key { id, kind: 2 })
}
fn handoff(idx: u16, hz: u32, svc: u8, action: u8) {
    let mut h = vec![];
    h.extend(idx.to_le_bytes());
    h.extend(hz.to_le_bytes());
    h.extend([svc, action]);
    SIM.with(|s| s.borrow_mut().handoff = h);
}
fn snapshot(api: &Api, name: &str) {
    logic::entry(api, AppEvent::Draw);
    if let Ok(dir) = std::env::var("AIRBAND_PREVIEW_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        SIM.with(|s| {
            let s = s.borrow();
            let mut b = b"P6\n160 128\n255\n".to_vec();
            for c in &s.pixels {
                let c = Rgb888::from(*c);
                b.extend([c.r(), c.g(), c.b()]);
            }
            std::fs::write(format!("{dir}/{name}.ppm"), b).unwrap();
        });
    }
}

#[test]
fn database_crc_aliases_types_and_names() {
    reset();
    let d = db::Db::load(&Memory, true).unwrap();
    assert_eq!(d.airports, 5);
    let mut out = [db::NONE; 24];
    assert!(d.search(&Memory, b"A", false, 0, &mut out) >= 2);
    assert_eq!(d.search(&Memory, b"AAA", true, 0, &mut out), 1);
    assert_eq!(d.search(&Memory, b"PANC", false, 0, &mut out), 1);
    let f = d.facility(&Memory, out[0]).unwrap();
    assert_eq!(&f.id, b"ANC\0");
    let mut name = [0; 22];
    let n = d.name(&Memory, &f, &mut name);
    assert_eq!(&name[..n], b"TEST ALASKA");
    SIM.with(|s| s.borrow_mut().flash[db::BASE as usize + 100] ^= 1);
    assert!(db::Db::load(&Memory, true).is_none());
}
#[test]
fn proximity_and_dateline() {
    reset();
    let d = db::Db::load(&Memory, true).unwrap();
    let f = d
        .facility(&Memory, d.exact(&Memory, *b"AAA\0", false).unwrap())
        .unwrap();
    let mut out = [db::NONE; 24];
    let n = d.nearest(&Memory, &f, false, 50, &mut out);
    assert!(n > 0);
    assert_eq!(&d.facility(&Memory, out[0]).unwrap().id, b"BBB\0");
    let a = db::Facility {
        lat: 600000,
        lon: 1799000,
        ..Default::default()
    };
    let b = db::Facility {
        lat: 600000,
        lon: -1799000,
        ..Default::default()
    };
    assert!(db::distance_nm(&a, &b) < 10);
}
#[test]
fn favorites_survive_reload_rotation_and_failed_write() {
    reset();
    let mut p = db::Prefs::load(&Memory);
    let f = db::Favorite {
        id: *b"AAA\0",
        hz: 123450000,
        nav: false,
        service: 0,
    };
    assert_eq!(p.toggle(&Memory, f), Some(true));
    for _ in 0..70 {
        assert!(p.save(&Memory));
    }
    assert!(db::Prefs::load(&Memory).favorites[0] == f);
    SIM.with(|s| s.borrow_mut().fail = true);
    assert_eq!(p.toggle(&Memory, f), None);
    SIM.with(|s| s.borrow_mut().fail = false);
    assert!(db::Prefs::load(&Memory).favorites[0] == f);
}
#[test]
fn malformed_header_never_tunes() {
    reset();
    SIM.with(|s| s.borrow_mut().flash[db::BASE as usize] = 0);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    key(&api, 12);
    snapshot(&api, "invalid");
    SIM.with(|s| {
        assert_eq!(s.borrow().freq, 0);
        assert_eq!(s.borrow().tx, 0)
    });
}
#[cfg(not(any(feature = "receiver", feature = "nearby")))]
#[test]
fn directory_navigation_and_handoff() {
    reset();
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    snapshot(&api, "home");
    key(&api, 1);
    for _ in 0..10 {
        key(&api, 14);
    }
    for _ in 0..3 {
        key(&api, 11);
    }
    snapshot(&api, "lookup");
    assert!(matches!(key(&api, 12), AppResult::Continue));
    logic::entry(&api, AppEvent::Draw);
    snapshot(&api, "results");
    assert!(matches!(key(&api, 12), AppResult::Chain(1)));
    SIM.with(|s| {
        let s = s.borrow();
        assert_eq!(db::u16le(&s.handoff), 0);
        assert_eq!(s.tx, 0);
        assert_eq!(s.freq, 0);
    });
}
#[cfg(all(feature = "receiver", not(feature = "listener")))]
#[test]
fn service_filter_and_chain() {
    reset();
    handoff(0, 0, 255, 0);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    snapshot(&api, "services");
    key(&api, 10);
    snapshot(&api, "tower-filter");
    assert!(matches!(key(&api, 12), AppResult::Chain(3)));
    SIM.with(|s| {
        assert_eq!(s.borrow().handoff[6], 0);
        assert_eq!(s.borrow().freq, 0);
        assert_eq!(s.borrow().tx, 0);
    });
}
#[cfg(feature = "nearby")]
#[test]
fn nearby_view_uses_selected_airport_not_inferred_position() {
    reset();
    let mut p = db::Prefs::load(&Memory);
    p.anchor = *b"AAA\0";
    assert!(p.save(&Memory));
    handoff(db::NONE, 0, 0, 2);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    snapshot(&api, "nearby");
    assert!(matches!(key(&api, 12), AppResult::Chain(1)));
    SIM.with(|s| assert_eq!(db::u16le(&s.borrow().handoff), 2));
}
#[cfg(feature = "listener")]
#[test]
fn scan_hold_favorites_and_receive_only() {
    reset();
    handoff(0, 123450000, 255, 4);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    SIM.with(|s| {
        assert_eq!(s.borrow().am, 1);
        assert_eq!(s.borrow().freq, 123450000);
        s.borrow_mut().busy = true;
    });
    tick(&api, 3000);
    SIM.with(|s| s.borrow_mut().busy = false);
    tick(&api, 14000);
    SIM.with(|s| assert_eq!(s.borrow().freq, 123450000));
    tick(&api, 1000);
    SIM.with(|s| assert_ne!(s.borrow().freq, 123450000));
    key(&api, 10);
    assert_eq!(db::Prefs::load(&Memory).count, 1);
    snapshot(&api, "listen");
    key(&api, 11);
    key(&api, 0);
    assert!(matches!(key(&api, 13), AppResult::Chain(1)));
    SIM.with(|s| assert_eq!(s.borrow().tx, 0));
}

#[test]
fn national_database_cross_language_lookup() {
    reset();
    let blob = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../airband/airband.db"),
    )
    .unwrap();
    SIM.with(|s| {
        s.borrow_mut().flash[db::BASE as usize..db::BASE as usize + blob.len()]
            .copy_from_slice(&blob)
    });
    let d = db::Db::load(&Memory, true).unwrap();
    assert_eq!(d.airports, 7740);
    assert_eq!(d.facilities - d.airports, 783);
    let mut out = [db::NONE; 24];
    let n = d.search(&Memory, b"KORD", false, 0, &mut out);
    assert!(n > 0);
    assert!(out[..n.min(24)].iter().any(|i| d
        .facility(&Memory, *i)
        .unwrap()
        .id
        == *b"ORD\0"));
    assert!(d.search(&Memory, b"PANC", false, 0, &mut out) > 0);
    assert!(out.iter().filter(|i| **i != db::NONE).any(|i| d
        .facility(&Memory, *i)
        .unwrap()
        .id
        == *b"ANC\0"));
    // Every ambiguous prefix result is reachable, without duplicate alias rows.
    let total = d.search(&Memory, b"K", false, 0, &mut out);
    assert!(total > 24);
    let mut seen = std::collections::BTreeSet::new();
    for skip in (0..total).step_by(24) {
        out = [db::NONE; 24];
        assert_eq!(d.search(&Memory, b"K", false, skip, &mut out), total);
        for idx in &out[..(total - skip).min(24)] {
            assert!(seen.insert(*idx));
        }
    }
    assert_eq!(seen.len(), total);
    for i in 0..d.facilities {
        assert!(d.facility(&Memory, i).is_some());
    }
}
#[cfg(feature = "listener")]
#[test]
fn stale_favorite_cannot_tune() {
    reset();
    handoff(0, 136975000, 12, 1);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    SIM.with(|s| {
        assert_eq!(s.borrow().freq, 0);
        assert_eq!(s.borrow().tx, 0);
    });
}
#[cfg(feature = "listener")]
#[test]
fn upper_airband_forces_am() {
    reset();
    let d = db::Db::load(&Memory, true).unwrap();
    SIM.with(|sim| {
        let mut s = sim.borrow_mut();
        let base = db::BASE as usize;
        let off = base + d.freq as usize;
        s.flash[off..off + 2].copy_from_slice(&5795u16.to_le_bytes());
        let crc = !db::crc(!0, &s.flash[base + 64..base + d.end as usize]);
        s.flash[base + 44..base + 48].copy_from_slice(&crc.to_le_bytes());
        let crc = !db::crc(!0, &s.flash[base..base + 60]);
        s.flash[base + 60..base + 64].copy_from_slice(&crc.to_le_bytes());
    });
    assert!(db::Db::load(&Memory, true).is_some());
    handoff(0, 136975000, 255, 5);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    SIM.with(|s| {
        assert_eq!(s.borrow().freq, 136975000);
        assert_eq!(s.borrow().am, 1);
        assert_eq!(s.borrow().tx, 0);
    });
}

fn tick(api: &Api, dt: u32) {
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        s.time = s.time.wrapping_add(dt);
    });
    logic::entry(api, AppEvent::Tick { dt_100us: dt });
}

#[test]
fn arrow_identifier_editor_covers_letters_digits_positions_and_delete() {
    let mut i = db::Input::new();
    assert_eq!(&i.chars[..i.len as usize], b"A");
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    for expected in &alphabet[1..] {
        i.cycle(true);
        assert_eq!(i.chars[0], *expected);
    }
    i.cycle(true);
    assert_eq!(i.chars[0], b'A');
    i.cycle(false);
    assert_eq!(i.chars[0], b'9');
    i.next();
    i.next();
    i.next();
    i.next();
    assert_eq!(i.len, 4);
    assert_eq!(i.cursor, 3);
    i.cycle(false);
    assert_eq!(&i.chars, b"9AA9");
    i.previous();
    i.cycle(true);
    assert_eq!(&i.chars, b"9AB9");
    assert!(i.delete());
    assert_eq!(&i.chars[..i.len as usize], b"9AB");
    assert!(i.delete());
    assert!(i.delete());
    assert!(!i.delete());
    assert_eq!(&i.chars[..i.len as usize], b"9");
}

#[cfg(feature = "listener")]
#[test]
fn stable_vor_meter_updates_do_not_clear_screen_or_retune() {
    reset();
    handoff(5, 114000000, 255, 4);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    logic::entry(&api, AppEvent::Draw);
    SIM.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(s.freq, 114000000);
        s.rects.clear();
    });
    let before = SIM.with(|s| s.borrow().tunes);
    for n in 0..20 {
        SIM.with(|s| s.borrow_mut().busy = n % 2 == 0);
        tick(&api, 5000);
        logic::entry(&api, AppEvent::Draw);
    }
    SIM.with(|s| {
        let s = s.borrow();
        assert_eq!(s.tunes, before);
        assert!(!s.rects.is_empty());
        assert!(s.rects.iter().all(|r| *r == (122, 66, 38, 8)));
    });
    key(&api, 0);
    logic::entry(&api, AppEvent::Draw);
    SIM.with(|s| assert!(!s.borrow().rects.contains(&(0, 0, 160, 128))));
}

#[cfg(not(any(feature = "receiver", feature = "nearby")))]
#[test]
fn database_info_chains_to_roomier_segment() {
    reset();
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    assert!(matches!(key(&api, 5), AppResult::Chain(2)));
    SIM.with(|s| assert_eq!(s.borrow().handoff[7], 7));
}

#[cfg(feature = "nearby")]
#[test]
fn database_info_view_and_return_home() {
    reset();
    handoff(db::NONE, 0, 0, 7);
    let api = generated::api();
    logic::entry(&api, AppEvent::Enter);
    logic::entry(&api, AppEvent::Draw);
    SIM.with(|s| {
        assert!(s.borrow().labels.iter().any(|l| l == "FAA NASR effective"))
    });
    assert!(matches!(key(&api, 13), AppResult::Chain(0)));
}

#[test]
fn firmware_cps_wire_crc_and_frame_roundtrip() {
    assert_eq!(crc::crc16_xmodem(b"123456789"), 0x31c3);
    let independent = |bytes: &[u8]| -> u16 {
        let mut c = 0u16;
        for byte in bytes {
            c ^= (*byte as u16) << 8;
            for _ in 0..8 {
                c = if c & 0x8000 == 0 {
                    c << 1
                } else {
                    (c << 1) ^ 0x1021
                };
            }
        }
        c
    };
    for n in 0..=512 {
        let data: Vec<u8> = (0..n).map(|i| (i * 37 + n) as u8).collect();
        let mut out = vec![0; 520];
        let size = cps_frames::encode(&mut out, 0x53, 0x1234, &data);
        assert_eq!(size, n + 8);
        assert_eq!(
            u16::from_be_bytes([out[size - 2], out[size - 1]]),
            independent(&out[1..size - 2])
        );
        for split in [0, n / 2, n] {
            let crc = crc::crc16_xmodem_update(
                crc::crc16_xmodem(&data[..split]),
                &data[split..],
            );
            assert_eq!(crc, independent(&data));
        }
        let mut reader = cps_frames::FrameReader::new();
        for (i, byte) in out[..size].iter().enumerate() {
            match reader.feed(*byte) {
                cps_frames::FeedResult::Frame(frame) => {
                    assert_eq!(i, size - 1);
                    assert_eq!(frame.cmd, 0x53);
                    assert_eq!(frame.addr, 0x1234);
                    assert_eq!(&frame.data[..frame.len], data);
                }
                cps_frames::FeedResult::Pending => assert_ne!(i, size - 1),
                _ => panic!("valid frame rejected"),
            }
        }
        out[size - 1] ^= 1;
        let mut reader = cps_frames::FrameReader::new();
        for (i, byte) in out[..size].iter().enumerate() {
            if i == size - 1 {
                assert!(matches!(
                    reader.feed(*byte),
                    cps_frames::FeedResult::CrcError { addr: 0x1234 }
                ));
            } else {
                assert!(matches!(
                    reader.feed(*byte),
                    cps_frames::FeedResult::Pending
                ));
            }
        }
    }
}
