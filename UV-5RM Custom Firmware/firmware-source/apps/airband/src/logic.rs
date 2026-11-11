//! Offline, receive-only airport / VOR directory. No transmit API is called.
use crate::db::{
    self, Db, Facility, Favorite, Flash, Frequency, Prefs, MAX_FREQS, NONE,
};
use bf5rh_abi::{Api, AppEvent, AppResult};
const MENU: u8 = 12;
const EXIT: u8 = 13;
const UP: u8 = 14;
const DOWN: u8 = 15;
const AB: u8 = 17;
const STAR: u8 = 10;
const HASH: u8 = 11;
const WHITE: u16 = 0xffff;
const GRAY: u16 = 0x9cf3;
const GOLD: u16 = 0xfd20;
const BLUE: u16 = 0x1910;
pub const SERVICES: [&[u8]; 16] = [
    b"TOWER",
    b"GROUND",
    b"ATIS",
    b"AWOS/ASOS",
    b"CTAF",
    b"UNICOM",
    b"CLEARANCE",
    b"APPROACH",
    b"DEPARTURE",
    b"APP/DEP",
    b"CENTER",
    b"FSS/RCO",
    b"OTHER",
    b"VOR",
    b"WEATHER",
    b"EMERGENCY",
];
struct ApiFlash<'a>(&'a Api);
impl Flash for ApiFlash<'_> {
    fn read(&self, a: u32, b: &mut [u8]) -> bool {
        (self.0.nor_read)(a, b.as_mut_ptr(), b.len() as u32)
    }
    fn write(&self, a: u32, b: &[u8]) -> bool {
        a >= db::PREF_BASE
            && a.checked_add(b.len() as u32).is_some_and(|e| e <= 0x1f0000)
            && (self.0.nor_write)(a, b.as_ptr(), b.len() as u32)
    }
    fn erase(&self, a: u32) -> bool {
        (a == db::PREF_BASE || a == db::PREF_BASE + 4096)
            && (self.0.nor_erase_sector)(a)
    }
}
#[derive(Clone, Copy, PartialEq)]
enum View {
    Home,
    Search,
    Results,
    Near,
    Freqs,
    Listen,
    Favorites,
    Info,
    Error,
}
struct State {
    db: Db,
    prefs: Prefs,
    valid: bool,
    view: View,
    selected: u8,
    dirty: bool,
    notice: &'static [u8],
    #[cfg(not(feature = "receiver"))]
    nav: bool,
    #[cfg(not(feature = "receiver"))]
    input: db::Input,
    #[cfg(not(feature = "receiver"))]
    rows: [u16; 24],
    #[cfg(not(feature = "receiver"))]
    count: u8,
    #[cfg(not(feature = "receiver"))]
    more: bool,
    #[cfg(not(feature = "receiver"))]
    skip: u16,
    #[cfg(not(feature = "receiver"))]
    center: Facility,
    #[cfg(not(feature = "receiver"))]
    has_center: bool,
    #[cfg(not(feature = "receiver"))]
    radius: u16,
    #[cfg(feature = "receiver")]
    facility: u16,
    #[cfg(feature = "receiver")]
    fac: Facility,
    #[cfg(feature = "receiver")]
    frequencies: [Frequency; MAX_FREQS],
    #[cfg(feature = "receiver")]
    filtered: [u8; MAX_FREQS],
    #[cfg(feature = "receiver")]
    freq_count: u8,
    #[cfg(feature = "receiver")]
    filter: u8,
    #[cfg(feature = "receiver")]
    scan: bool,
    #[cfg(feature = "receiver")]
    monitor: bool,
    #[cfg(feature = "receiver")]
    elapsed: u32,
    #[cfg(feature = "receiver")]
    quiet: u32,
    #[cfg(feature = "receiver")]
    held: bool,
    #[cfg(feature = "receiver")]
    refresh: u32,
    #[cfg(feature = "receiver")]
    stamp: u32,
    #[cfg(feature = "receiver")]
    rssi: u16,
    #[cfg(feature = "listener")]
    meter_dirty: bool,
    #[cfg(feature = "listener")]
    painted: bool,
}
impl State {
    const fn empty() -> Self {
        Self {
            db: Db {
                facilities: 0,
                airports: 0,
                aliases: 0,
                frequencies: 0,
                records: 0,
                freq: 0,
                alias: 0,
                names: 0,
                end: 0,
                date: [0; 8],
            },
            prefs: Prefs {
                favorites: [Favorite {
                    id: [0; 4],
                    hz: 0,
                    nav: false,
                    service: 0,
                }; 8],
                count: 0,
                anchor: [0; 4],
                generation: 0,
                next: 0,
                sector: 0,
            },
            valid: false,
            view: View::Home,
            selected: 0,
            dirty: true,
            notice: b"",
            #[cfg(not(feature = "receiver"))]
            nav: false,
            #[cfg(not(feature = "receiver"))]
            input: db::Input::new(),
            #[cfg(not(feature = "receiver"))]
            rows: [NONE; 24],
            #[cfg(not(feature = "receiver"))]
            count: 0,
            #[cfg(not(feature = "receiver"))]
            more: false,
            #[cfg(not(feature = "receiver"))]
            skip: 0,
            #[cfg(not(feature = "receiver"))]
            center: Facility {
                id: [0; 4],
                nav: false,
                k_alias: false,
                lat: 0,
                lon: 0,
                start: 0,
                count: 0,
                name: 0,
            },
            #[cfg(not(feature = "receiver"))]
            has_center: false,
            #[cfg(not(feature = "receiver"))]
            radius: 100,
            #[cfg(feature = "receiver")]
            facility: NONE,
            #[cfg(feature = "receiver")]
            fac: Facility {
                id: [0; 4],
                nav: false,
                k_alias: false,
                lat: 0,
                lon: 0,
                start: 0,
                count: 0,
                name: 0,
            },
            #[cfg(feature = "receiver")]
            frequencies: [Frequency {
                step: 0,
                service: 0,
            }; MAX_FREQS],
            #[cfg(feature = "receiver")]
            filtered: [0; MAX_FREQS],
            #[cfg(feature = "receiver")]
            freq_count: 0,
            #[cfg(feature = "receiver")]
            filter: 255,
            #[cfg(feature = "receiver")]
            scan: false,
            #[cfg(feature = "receiver")]
            monitor: false,
            #[cfg(feature = "receiver")]
            elapsed: 0,
            #[cfg(feature = "receiver")]
            quiet: 0,
            #[cfg(feature = "receiver")]
            held: false,
            #[cfg(feature = "receiver")]
            refresh: 0,
            #[cfg(feature = "receiver")]
            stamp: 0,
            #[cfg(feature = "receiver")]
            rssi: 0,
            #[cfg(feature = "listener")]
            meter_dirty: false,
            #[cfg(feature = "listener")]
            painted: false,
        }
    }
    #[cfg(feature = "receiver")]
    fn current(&self) -> Frequency {
        self.frequencies[self.filtered[self.selected as usize] as usize]
    }
    #[cfg(feature = "receiver")]
    fn favorite(&self) -> Favorite {
        let v = self.current();
        Favorite {
            id: self.fac.id,
            hz: v.hz(),
            nav: self.fac.nav,
            service: v.service,
        }
    }
}
static mut STATE: State = State::empty();
fn state() -> &'static mut State {
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}
fn text(api: &Api, x: i16, y: i16, b: &[u8], color: u16, font: u8) {
    (api.draw_text)(x, y, b.as_ptr(), b.len() as u16, color, 0, font);
}
fn code_len(b: &[u8]) -> usize {
    b.iter().position(|x| *x == 0).unwrap_or(b.len())
}
fn number(api: &Api, x: i16, y: i16, v: u32) {
    let mut b = [0; 16];
    let n = (api.fmt_u32)(v, b.as_mut_ptr()) as usize;
    text(api, x, y, &b[..n.min(16)], WHITE, 0);
}
fn frequency(api: &Api, x: i16, y: i16, hz: u32, font: u8) {
    let mut b = [b'0'; 7];
    let mhz = hz / 1_000_000;
    let khz = hz / 1000 % 1000;
    b[0] = b'0' + (mhz / 100) as u8;
    b[1] = b'0' + (mhz / 10 % 10) as u8;
    b[2] = b'0' + (mhz % 10) as u8;
    b[3] = b'.';
    b[4] = b'0' + (khz / 100) as u8;
    b[5] = b'0' + (khz / 10 % 10) as u8;
    b[6] = b'0' + (khz % 10) as u8;
    text(api, x, y, &b, WHITE, font);
}
fn mute(api: &Api, s: &mut State) {
    #[cfg(feature = "receiver")]
    {
        s.scan = false;
        s.monitor = false;
    }
    (api.set_monitor)(false);
    (api.set_holds_rx)(false);
    (api.set_af_out)(false);
}
fn finish(api: &Api, s: &mut State) -> AppResult {
    mute(api, s);
    (api.set_tx_enabled)(false);
    (api.set_backlight_hold)(false);
    AppResult::Exit
}
#[cfg(feature = "receiver")]
fn tune(api: &Api, s: &mut State) {
    if s.freq_count == 0 {
        return;
    }
    let hz = s.current().hz();
    if !(108_000_000..=136_975_000).contains(&hz) {
        return;
    }
    (api.set_tx_enabled)(false);
    (api.set_modulation)(1);
    (api.set_rx_freq)(hz);
    (api.set_bandwidth)(true);
    (api.set_subaudio_rx)(0);
    (api.set_subaudio_tx)(0);
    let sql = (api.settings_get)(0) as u8;
    (api.set_sql_level)(if s.scan { sql.max(1) } else { sql });
    (api.set_monitor)(s.monitor);
    (api.enter_rx)();
    s.stamp = (api.uptime_100us)();
    s.elapsed = 0;
    s.quiet = 0;
    s.held = false;
    s.dirty = true;
}
#[cfg(feature = "receiver")]
fn filter(s: &mut State) {
    s.freq_count = 0;
    for i in 0..s.fac.count {
        if s.filter == 255 || s.frequencies[i as usize].service == s.filter {
            s.filtered[s.freq_count as usize] = i;
            s.freq_count += 1;
        }
    }
    s.selected = 0;
    s.dirty = true;
}
#[cfg(feature = "receiver")]
fn open(api: &Api, s: &mut State, idx: u16) -> bool {
    let flash = ApiFlash(api);
    let Some(fac) = s.db.facility(&flash, idx) else {
        s.notice = b"INVALID FACILITY";
        return false;
    };
    for i in 0..fac.count {
        let Some(v) = s.db.frequency(&flash, fac.start + i as u16) else {
            s.notice = b"INVALID FREQUENCY";
            return false;
        };
        if (v.service == 13) != fac.nav
            || (!fac.nav && v.hz() < 118_000_000)
            || (fac.nav && v.hz() > 117_950_000)
        {
            s.notice = b"INVALID BAND";
            return false;
        }
        s.frequencies[i as usize] = v;
    }
    s.facility = idx;
    s.fac = fac;
    s.filter = 255;
    filter(s);
    s.view = View::Freqs;
    s.notice = b"";
    true
}
#[cfg(not(feature = "receiver"))]
fn search(api: &Api, s: &mut State) {
    s.rows = [NONE; 24];
    let n = s.db.search(
        &ApiFlash(api),
        &s.input.chars[..s.input.len as usize],
        s.nav,
        s.skip as usize,
        &mut s.rows,
    );
    s.count = n.saturating_sub(s.skip as usize).min(24) as u8;
    s.more = n > s.skip as usize + 24;
    s.selected = 0;
    s.view = View::Search;
    s.dirty = true;
}
#[cfg(not(feature = "receiver"))]
fn nearby(api: &Api, s: &mut State) {
    if !s.has_center {
        s.notice = b"SELECT AIRPORT, A/B NEAR";
        s.dirty = true;
        return;
    }
    s.rows = [NONE; 24];
    s.count =
        s.db.nearest(&ApiFlash(api), &s.center, s.nav, s.radius, &mut s.rows)
            as u8;
    s.more = false;
    s.selected = 0;
    s.view = View::Near;
    s.dirty = true;
}
#[cfg(feature = "receiver")]
fn favorite_open(api: &Api, s: &mut State, idx: usize) -> bool {
    let fav = s.prefs.favorites[idx];
    let Some(id) = s.db.exact(&ApiFlash(api), fav.id, fav.nav) else {
        s.notice = b"FAVORITE NOT IN THIS DB";
        return false;
    };
    if !open(api, s, id) {
        return false;
    }
    for j in 0..s.freq_count {
        s.selected = j;
        if s.favorite() == fav {
            return true;
        }
    }
    s.view = View::Favorites;
    s.notice = b"FREQUENCY NO LONGER LISTED";
    false
}
fn chain(api: &Api, idx: u16, hz: u32, service: u8, action: u8) -> AppResult {
    let mut b = [0; 8];
    b[..2].copy_from_slice(&idx.to_le_bytes());
    b[2..6].copy_from_slice(&hz.to_le_bytes());
    b[6] = service;
    b[7] = action;
    if !(api.handoff_write)(b.as_ptr(), 8) {
        return AppResult::Fault(1);
    }
    AppResult::Chain(match action {
        1 | 4 | 5 => 3,
        2 | 7 => 2,
        3 => 0,
        _ => 1,
    })
}
pub fn entry(api: &Api, event: AppEvent) -> AppResult {
    let s = state();
    match event {
        AppEvent::Enter => {
            *s = State::empty();
            (api.set_tx_enabled)(false);
            (api.set_agc_fix)(-1);
            (api.set_backlight_hold)(true);
            mute(api, s);
            if let Some(db) =
                Db::load(&ApiFlash(api), !cfg!(feature = "receiver"))
            {
                s.db = db;
                s.valid = true;
                s.prefs = Prefs::load(&ApiFlash(api));
                #[cfg(not(feature = "receiver"))]
                if let Some(idx) =
                    db.exact(&ApiFlash(api), s.prefs.anchor, false)
                {
                    if let Some(v) = db.facility(&ApiFlash(api), idx) {
                        s.center = v;
                        s.has_center = true;
                    }
                }
            } else {
                s.view = View::Error;
            }
            s.dirty = true;
            if s.valid {
                let mut h = [0; 8];
                let n = (api.handoff_read)(h.as_mut_ptr(), 8);
                #[cfg(feature = "receiver")]
                {
                    if n != 8 || !open(api, s, db::u16le(&h)) {
                        s.view = View::Error;
                    } else {
                        if h[7] != 1 {
                            s.filter = h[6];
                            filter(s);
                            if s.freq_count == 0 {
                                s.filter = 255;
                                filter(s);
                            }
                        }
                        let hz = db::u32le(&h[2..]);
                        let mut found = hz == 0;
                        for j in 0..s.freq_count {
                            s.selected = j;
                            let f = s.current();
                            if f.hz() == hz && (h[7] != 1 || f.service == h[6])
                            {
                                found = true;
                                break;
                            }
                        }
                        if hz == 0 || !found {
                            s.selected = 0;
                        }
                        #[cfg(feature = "listener")]
                        {
                            if found {
                                s.scan = h[7] == 4;
                                s.view = View::Listen;
                                tune(api, s);
                            } else {
                                s.view = View::Error;
                            }
                        }
                    }
                }
                #[cfg(feature = "nearby")]
                {
                    let _ = (n, h);
                    s.nav = false;
                    if n == 8 && h[7] == 7 {
                        s.view = View::Info;
                    } else {
                        s.view = View::Near;
                        nearby(api, s);
                    }
                }
            }
        }
        AppEvent::Leave => {
            mute(api, s);
            (api.set_monitor)(false);
            (api.set_tx_enabled)(false);
            (api.set_backlight_hold)(false);
        }
        AppEvent::Draw => {
            if s.dirty {
                draw(api, s);
                s.dirty = false;
                #[cfg(feature = "listener")]
                {
                    s.meter_dirty = false;
                    s.painted = s.view == View::Listen;
                }
            }
            #[cfg(feature = "listener")]
            if s.meter_dirty {
                (api.fill_rect)(122, 66, 38, 8, 0);
                number(api, 122, 66, s.rssi as u32);
                s.meter_dirty = false;
            }
        }
        AppEvent::Tick { dt_100us } =>
        {
            #[cfg(feature = "listener")]
            if s.view == View::Listen {
                let now = (api.uptime_100us)();
                let dt_100us = now.wrapping_sub(s.stamp);
                s.stamp = now;
                s.refresh = s.refresh.saturating_add(dt_100us);
                s.elapsed = s.elapsed.saturating_add(dt_100us);
                if s.scan && s.freq_count > 1 {
                    if s.elapsed >= 1000 && (api.squelch_open)() {
                        s.quiet = 0;
                        s.held = true;
                    } else {
                        s.quiet = s.quiet.saturating_add(dt_100us);
                    }
                    if s.elapsed >= 3000
                        && s.quiet >= if s.held { 15000 } else { 3000 }
                    {
                        s.selected = (s.selected + 1) % s.freq_count;
                        tune(api, s);
                    }
                }
                if s.refresh >= 5000 {
                    s.refresh = 0;
                    let rssi = (api.read_rssi)();
                    if rssi != s.rssi {
                        s.rssi = rssi;
                        s.meter_dirty = true;
                    }
                }
            }
        }
        AppEvent::Key { id, kind } => {
            if id == EXIT && kind == 3 {
                return finish(api, s);
            }
            if kind != 2 && kind != 4 {
                return AppResult::Continue;
            }
            s.notice = b"";
            s.dirty = true;
            if (id == UP || id == DOWN)
                && !matches!(
                    s.view,
                    View::Home | View::Error | View::Info | View::Search
                )
            {
                #[cfg(feature = "receiver")]
                let n = s.freq_count;
                #[cfg(not(feature = "receiver"))]
                let n = if s.view == View::Favorites {
                    s.prefs.count
                } else {
                    s.count
                };
                if n > 0 {
                    s.selected = if id == UP {
                        (s.selected + n - 1) % n
                    } else {
                        (s.selected + 1) % n
                    };
                    #[cfg(feature = "listener")]
                    if s.view == View::Listen {
                        tune(api, s);
                    }
                }
                return AppResult::Continue;
            }
            match s.view {
                #[cfg(not(any(feature = "receiver", feature = "nearby")))]
                View::Home => match id {
                    1 | 2 => {
                        s.nav = id == 2;
                        s.input = db::Input::new();
                        s.skip = 0;
                        s.more = false;
                        s.count = 0;
                        search(api, s);
                    }
                    3 => {
                        s.selected = 0;
                        s.view = View::Favorites;
                    }
                    4 => {
                        if s.prefs.anchor == [0; 4] {
                            s.notice = b"SELECT AIRPORT, A/B NEAR";
                        } else {
                            return chain(api, NONE, 0, 0, 2);
                        }
                    }
                    5 => return chain(api, NONE, 0, 0, 7),
                    EXIT => return finish(api, s),
                    _ => {}
                },
                #[cfg(not(any(feature = "receiver", feature = "nearby")))]
                View::Search => match id {
                    UP | DOWN => {
                        s.input.cycle(id == UP);
                        s.skip = 0;
                        search(api, s);
                    }
                    STAR => s.input.previous(),
                    HASH => {
                        s.input.next();
                        search(api, s);
                    }
                    EXIT => {
                        if s.input.delete() {
                            s.skip = 0;
                            search(api, s);
                        } else {
                            s.view = View::Home;
                        }
                    }
                    MENU => {
                        s.view = View::Results;
                    }
                    _ => {}
                },
                #[cfg(not(any(feature = "receiver", feature = "nearby")))]
                View::Results => match id {
                    HASH => {
                        s.skip = if s.more { s.skip + 24 } else { 0 };
                        search(api, s);
                        s.view = View::Results;
                    }
                    EXIT => s.view = View::Search,
                    MENU => {
                        if s.count > 0 {
                            return chain(
                                api,
                                s.rows[s.selected as usize],
                                0,
                                255,
                                0,
                            );
                        }
                    }
                    _ => {}
                },
                #[cfg(feature = "nearby")]
                View::Near => match id {
                    MENU => {
                        if s.count > 0 {
                            return chain(
                                api,
                                s.rows[s.selected as usize],
                                0,
                                255,
                                0,
                            );
                        }
                    }
                    AB => {
                        s.nav = !s.nav;
                        nearby(api, s);
                    }
                    HASH => {
                        s.radius = match s.radius {
                            50 => 100,
                            100 => 200,
                            _ => 50,
                        };
                        nearby(api, s);
                    }
                    EXIT => return chain(api, NONE, 0, 0, 3),
                    _ => {}
                },
                #[cfg(all(feature = "receiver", not(feature = "listener")))]
                View::Freqs => match id {
                    MENU | HASH => {
                        return chain(
                            api,
                            s.facility,
                            s.current().hz(),
                            s.filter,
                            if id == HASH { 4 } else { 5 },
                        );
                    }
                    STAR => {
                        let mut f =
                            if s.filter == 255 { 0 } else { s.filter + 1 };
                        while f < 16
                            && !s.frequencies[..s.fac.count as usize]
                                .iter()
                                .any(|v| v.service == f)
                        {
                            f += 1;
                        }
                        s.filter = if f >= 16 { 255 } else { f };
                        filter(s);
                    }
                    AB => {
                        if !s.fac.nav {
                            s.prefs.anchor = s.fac.id;
                            if !s.prefs.save(&ApiFlash(api)) {
                                s.notice = b"CENTER SAVE FAILED";
                                return AppResult::Continue;
                            }
                            return chain(api, NONE, 0, 0, 2);
                        } else {
                            s.notice = b"CHOOSE AN AIRPORT CENTER";
                        }
                    }
                    EXIT => {
                        return {
                            mute(api, s);
                            chain(api, NONE, 0, 0, 3)
                        }
                    }
                    _ => {}
                },
                #[cfg(feature = "listener")]
                View::Listen => match id {
                    HASH => {
                        s.scan = !s.scan;
                        s.monitor = false;
                        tune(api, s);
                    }
                    STAR => {
                        let v = s.favorite();
                        s.notice = match s.prefs.toggle(&ApiFlash(api), v) {
                            Some(true) => b"FAVORITE SAVED",
                            Some(false) => b"FAVORITE REMOVED",
                            None => b"FAVORITES FULL / SAVE ERROR",
                        };
                    }
                    0 => {
                        s.scan = false;
                        s.monitor = !s.monitor;
                        tune(api, s);
                    }
                    EXIT | MENU => {
                        mute(api, s);
                        return chain(
                            api,
                            s.facility,
                            s.current().hz(),
                            s.filter,
                            6,
                        );
                    }
                    _ => {}
                },
                #[cfg(not(any(feature = "receiver", feature = "nearby")))]
                View::Favorites => match id {
                    MENU => {
                        if s.prefs.count > 0 {
                            let v = s.prefs.favorites[s.selected as usize];
                            if let Some(idx) =
                                s.db.exact(&ApiFlash(api), v.id, v.nav)
                            {
                                return chain(api, idx, v.hz, v.service, 1);
                            } else {
                                s.notice = b"FAVORITE NOT IN THIS DB";
                            }
                        }
                    }
                    EXIT => s.view = View::Home,
                    _ => {}
                },
                View::Info | View::Error => {
                    if id == EXIT {
                        if !s.valid {
                            return finish(api, s);
                        }
                        return chain(api, NONE, 0, 0, 3);
                    }
                }
                _ => {}
            }
        }
    }
    AppResult::Continue
}
fn draw(api: &Api, s: &State) {
    #[cfg(feature = "listener")]
    let full = !s.painted;
    #[cfg(not(feature = "listener"))]
    let full = true;
    if full {
        (api.fill_rect)(0, 0, 160, 128, 0);
        (api.fill_rect)(0, 0, 160, 14, BLUE);
        text(api, 2, 2, b"AIRBAND  RX ONLY", WHITE, 1);
    }
    match s.view {
        #[cfg(not(any(feature = "receiver", feature = "nearby")))]
        View::Home => {
            for (i, line) in [
                b"1 AIRPORT ID".as_slice(),
                b"2 VOR ID",
                b"3 FAVORITES",
                b"4 NEAR SELECTED AIRPORT",
                b"5 DATABASE",
            ]
            .iter()
            .enumerate()
            {
                text(api, 2, 17 + i as i16 * 14, line, WHITE, 1);
            }
            text(api, 2, 91, &s.db.date, GRAY, 1);
            text(api, 2, 117, b"EXIT Close   Hold EXIT Close", GRAY, 0);
        }
        View::Error => {
            text(api, 4, 26, b"DATABASE MISSING / INVALID", GOLD, 0);
            text(api, 4, 46, b"Upload a verified AIR1 file", WHITE, 0);
            text(api, 4, 66, b"No frequency will be tuned", WHITE, 0);
        }
        #[cfg(feature = "nearby")]
        View::Info => {
            text(api, 4, 20, b"FAA NASR effective", GRAY, 0);
            text(api, 4, 32, &s.db.date, WHITE, 1);
            text(api, 4, 51, b"Airports", GRAY, 0);
            number(api, 90, 51, s.db.airports as u32);
            text(api, 4, 64, b"VORs", GRAY, 0);
            number(api, 90, 64, (s.db.facilities - s.db.airports) as u32);
            text(api, 4, 77, b"Frequencies", GRAY, 0);
            number(api, 90, 77, s.db.frequencies as u32);
            text(api, 4, 97, b"Offline published frequencies", GOLD, 0);
            text(api, 4, 111, b"No live NOTAM / navigation fix", GRAY, 0);
        }
        #[cfg(not(feature = "receiver"))]
        View::Search | View::Results | View::Near => {
            if cfg!(feature = "nearby") {
                text(api, 3, 16, b"NEAR", GRAY, 0);
                let c = s.center.display_id();
                text(api, 29, 16, &c[..code_len(&c)], WHITE, 0);
                number(api, 58, 16, s.radius as u32);
                text(api, 78, 16, b"nm~", GRAY, 0);
                text(
                    api,
                    100,
                    16,
                    if s.nav { b"VORs" } else { b"Airports" },
                    GOLD,
                    0,
                );
            } else {
                text(
                    api,
                    2,
                    16,
                    if s.nav { b"VOR ID:" } else { b"AIRPORT ID:" },
                    GRAY,
                    1,
                );
                text(
                    api,
                    76,
                    16,
                    &s.input.chars[..s.input.len as usize],
                    GOLD,
                    1,
                );
                if s.view == View::Search {
                    text(
                        api,
                        76 + s.input.cursor as i16 * 6,
                        26,
                        b"^",
                        GOLD,
                        0,
                    );
                }
            }
            let start = (s.selected as usize / 6) * 6;
            for j in 0..6 {
                let k = start + j;
                if k >= s.count as usize {
                    break;
                }
                let Some(v) = s.db.facility(&ApiFlash(api), s.rows[k]) else {
                    continue;
                };
                let y = 30 + j as i16 * 11;
                text(
                    api,
                    0,
                    y,
                    if k == s.selected as usize { b">" } else { b" " },
                    GOLD,
                    0,
                );
                let id = v.display_id();
                text(api, 7, y, &id[..code_len(&id)], WHITE, 0);
                if cfg!(feature = "nearby") {
                    number(api, 34, y, db::distance_nm(&s.center, &v));
                    let mut name = [0; 22];
                    let n = s.db.name(&ApiFlash(api), &v, &mut name);
                    text(api, 57, y, &name[..n.min(20)], GRAY, 0);
                } else {
                    let mut name = [0; 22];
                    let n = s.db.name(&ApiFlash(api), &v, &mut name);
                    text(api, 33, y, &name[..n], GRAY, 0);
                }
            }
            if s.count == 0 {
                text(
                    api,
                    4,
                    40,
                    if s.view == View::Search {
                        b"Adjust ID, then MENU results"
                    } else {
                        b"No matching entries"
                    },
                    GRAY,
                    0,
                );
            }
            if cfg!(feature = "nearby") {
                text(api, 3, 105, b"MENU Select AB Type # Radius", GRAY, 0);
            } else {
                text(
                    api,
                    3,
                    95,
                    if s.view == View::Search {
                        b"UP/DN Char  */# Position"
                    } else {
                        b"UP/DN Select  MENU Open"
                    },
                    GRAY,
                    0,
                );
                text(
                    api,
                    3,
                    105,
                    if s.view == View::Search {
                        b"A-Z then 0-9 at each slot"
                    } else {
                        b"# Next page (if shown)"
                    },
                    GRAY,
                    0,
                );
                text(
                    api,
                    3,
                    116,
                    if s.view == View::Search {
                        b"MENU Results  EXIT Delete"
                    } else {
                        b"EXIT Back to ID editor"
                    },
                    GRAY,
                    0,
                );
            }
        }
        #[cfg(all(feature = "receiver", not(feature = "listener")))]
        View::Freqs => {
            let id = s.fac.display_id();
            text(api, 2, 16, &id[..code_len(&id)], GOLD, 1);
            let mut name = [0; 22];
            let n = s.db.name(&ApiFlash(api), &s.fac, &mut name);
            text(api, 34, 18, &name[..n], WHITE, 0);
            text(
                api,
                2,
                29,
                if s.filter == 255 {
                    b"ALL SERVICES"
                } else {
                    SERVICES[s.filter as usize]
                },
                GRAY,
                1,
            );
            let start = (s.selected as usize / 5) * 5;
            for j in 0..5 {
                let k = start + j;
                if k >= s.freq_count as usize {
                    break;
                }
                let v = s.frequencies[s.filtered[k] as usize];
                let y = 42 + j as i16 * 12;
                text(
                    api,
                    0,
                    y,
                    if k == s.selected as usize { b">" } else { b" " },
                    GOLD,
                    0,
                );
                text(api, 7, y, SERVICES[v.service as usize], WHITE, 1);
                frequency(api, 111, y, v.hz(), 1);
            }
            text(api, 3, 105, b"MENU Tune # Scan  * Service", GRAY, 0);
            text(api, 3, 116, b"A/B Nearby    EXIT Home", GRAY, 0);
        }
        #[cfg(feature = "listener")]
        View::Listen => {
            if s.painted {
                // Keep the header/footer steady across scan steps and key actions.
                for (y, h) in [(16, 42), (66, 8), (82, 8), (94, 10)] {
                    (api.fill_rect)(0, y, 160, h, 0);
                }
            }
            let id = s.fac.display_id();
            text(api, 4, 20, &id[..code_len(&id)], GOLD, 1);
            text(
                api,
                40,
                20,
                SERVICES[s.current().service as usize],
                WHITE,
                0,
            );
            frequency(api, 12, 40, s.current().hz(), 2);
            text(
                api,
                4,
                66,
                if s.scan {
                    b"SCAN / HOLD"
                } else if s.monitor {
                    b"MONITOR"
                } else {
                    b"LISTEN"
                },
                GOLD,
                0,
            );
            text(api, 93, 66, b"RSSI", GRAY, 0);
            number(api, 122, 66, s.rssi as u32);
            text(
                api,
                4,
                82,
                if s.prefs.favorites[..s.prefs.count as usize]
                    .contains(&s.favorite())
                {
                    b"SAVED FAVORITE"
                } else {
                    b"* Add favorite"
                },
                GRAY,
                0,
            );
            text(api, 4, 105, b"# Scan/Pause 0 Monitor", GRAY, 0);
            text(api, 4, 116, b"UP/DN Step   EXIT Services", GRAY, 0);
        }
        #[cfg(not(any(feature = "receiver", feature = "nearby")))]
        View::Favorites => {
            text(api, 4, 17, b"FAVORITES", GOLD, 0);
            for i in 0..s.prefs.count as usize {
                let v = s.prefs.favorites[i];
                let y = 30 + i as i16 * 9;
                text(
                    api,
                    0,
                    y,
                    if i == s.selected as usize { b">" } else { b" " },
                    GOLD,
                    0,
                );
                text(api, 6, y, &v.id[..code_len(&v.id)], WHITE, 0);
                text(
                    api,
                    30,
                    y,
                    SERVICES[(v.service as usize).min(15)],
                    GRAY,
                    0,
                );
                frequency(api, 103, y, v.hz, 0);
            }
            if s.prefs.count == 0 {
                text(api, 4, 44, b"Use * while listening to save", GRAY, 0);
            }
            text(api, 3, 116, b"MENU Tune; remove in listener", GRAY, 0);
        }
        _ => {}
    }
    if !s.notice.is_empty() {
        (api.fill_rect)(0, 94, 160, 10, 0);
        text(api, 2, 95, s.notice, GOLD, 0);
    }
}
