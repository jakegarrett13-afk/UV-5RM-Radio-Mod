//! AIR1 on-flash format. All reads are checked against the declared database.
pub const BASE: u32 = 0x194000;
pub const LIMIT: u32 = 0x5a000;
pub const PREF_BASE: u32 = BASE + LIMIT;
pub const MAX_FREQS: usize = 64;
pub const NONE: u16 = 0xffff;
pub trait Flash {
    fn read(&self, addr: u32, out: &mut [u8]) -> bool;
    fn write(&self, addr: u32, data: &[u8]) -> bool;
    fn erase(&self, addr: u32) -> bool;
}
pub fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
pub fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}
fn u24(b: &[u8]) -> u32 {
    b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16
}
fn i24(b: &[u8]) -> i32 {
    ((u24(b) << 8) as i32) >> 8
}
pub fn crc(mut c: u32, b: &[u8]) -> u32 {
    for &v in b {
        c ^= v as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(c & 1));
        }
    }
    c
}
#[derive(Clone, Copy, Default)]
pub struct Facility {
    pub id: [u8; 4],
    pub nav: bool,
    pub k_alias: bool,
    pub lat: i32,
    pub lon: i32,
    pub start: u16,
    pub count: u8,
    pub name: u32,
}
impl Facility {
    pub fn display_id(&self) -> [u8; 4] {
        if self.k_alias {
            [b'K', self.id[0], self.id[1], self.id[2]]
        } else {
            self.id
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct Frequency {
    pub step: u16,
    pub service: u8,
}
impl Frequency {
    pub fn hz(&self) -> u32 {
        108_000_000 + self.step as u32 * 5000
    }
}
#[derive(Clone, Copy, Default)]
pub struct Db {
    pub facilities: u16,
    pub airports: u16,
    pub aliases: u16,
    pub frequencies: u16,
    pub records: u32,
    pub freq: u32,
    pub alias: u32,
    pub names: u32,
    pub end: u32,
    pub date: [u8; 8],
}
impl Db {
    pub fn load(f: &impl Flash, verify_payload: bool) -> Option<Self> {
        let mut b = [0; 64];
        if !f.read(BASE, &mut b)
            || &b[..4] != b"AIR1"
            || u16le(&b[4..]) != 1
            || u16le(&b[14..]) != 64
            || !crc(!0, &b[..60]) != u32le(&b[60..])
        {
            return None;
        }
        let db = Self {
            facilities: u16le(&b[6..]),
            frequencies: u16le(&b[8..]),
            aliases: u16le(&b[10..]),
            airports: u16le(&b[12..]),
            records: u32le(&b[16..]),
            freq: u32le(&b[20..]),
            alias: u32le(&b[24..]),
            names: u32le(&b[28..]),
            end: u32le(&b[32..]),
            date: b[36..44].try_into().ok()?,
        };
        if db.facilities == 0
            || db.airports > db.facilities
            || db.records != 64
            || db.freq != 64 + db.facilities as u32 * 16
            || db.alias != db.freq + db.frequencies as u32 * 3
            || db.names != db.alias + db.aliases as u32 * 6
            || db.names > db.end
            || db.end > LIMIT
            || !db.date.iter().all(u8::is_ascii_digit)
        {
            return None;
        }
        if !verify_payload {
            return Some(db);
        }
        let mut c = !0;
        let mut off = 64;
        let mut buf = [0; 128];
        while off < db.end {
            let n = (db.end - off).min(128) as usize;
            if !db.read(f, off, &mut buf[..n]) {
                return None;
            }
            c = crc(c, &buf[..n]);
            off += n as u32;
        }
        if !c != u32le(&b[44..]) {
            return None;
        }
        Some(db)
    }
    fn read(&self, f: &impl Flash, off: u32, b: &mut [u8]) -> bool {
        off.checked_add(b.len() as u32)
            .is_some_and(|e| e <= self.end)
            && f.read(BASE + off, b)
    }
    pub fn facility(&self, f: &impl Flash, idx: u16) -> Option<Facility> {
        if idx >= self.facilities {
            return None;
        }
        let mut b = [0; 16];
        if !self.read(f, self.records + idx as u32 * 16, &mut b) {
            return None;
        }
        let nav = b[0] & 128 != 0;
        let k_alias = b[1] & 128 != 0;
        let mut id = [0; 4];
        for i in 0..4 {
            id[i] = b[i] & 127;
        }
        let v = Facility {
            id,
            nav,
            k_alias,
            lat: i24(&b[4..]),
            lon: i24(&b[7..]),
            start: u16le(&b[10..]),
            count: b[12],
            name: u24(&b[13..]),
        };
        if nav != (idx >= self.airports)
            || v.lat.abs() > 900000
            || v.lon.abs() > 1800000
            || v.count == 0
            || v.count as usize > MAX_FREQS
            || v.start as u32 + v.count as u32 > self.frequencies as u32
            || self.names + v.name >= self.end
            || !id.iter().all(|c| {
                *c == 0 || c.is_ascii_uppercase() || c.is_ascii_digit()
            })
        {
            return None;
        }
        Some(v)
    }
    pub fn frequency(&self, f: &impl Flash, idx: u16) -> Option<Frequency> {
        if idx >= self.frequencies {
            return None;
        }
        let mut b = [0; 3];
        if !self.read(f, self.freq + idx as u32 * 3, &mut b) {
            return None;
        }
        let v = Frequency {
            step: u16le(&b),
            service: b[2],
        };
        if v.service > 15 || !(108_000_000..=136_975_000).contains(&v.hz()) {
            None
        } else {
            Some(v)
        }
    }
    pub fn name(
        &self,
        f: &impl Flash,
        fac: &Facility,
        out: &mut [u8; 22],
    ) -> usize {
        let off = self.names + fac.name;
        let mut len = [0];
        if !self.read(f, off, &mut len) || len[0] > 22 {
            return 0;
        }
        let n = len[0] as usize;
        let mut bits = [0; 17];
        if !self.read(f, off + 1, &mut bits[..(n * 6 + 7) / 8]) {
            return 0;
        }
        for (i, c) in out[..n].iter_mut().enumerate() {
            let bit = i * 6;
            let j = bit / 8;
            let pair = bits[j] as u16
                | ((bits.get(j + 1).copied().unwrap_or(0) as u16) << 8);
            *c = 32 + ((pair >> (bit % 8)) & 63) as u8;
        }
        n
    }
    pub fn exact(&self, f: &impl Flash, id: [u8; 4], nav: bool) -> Option<u16> {
        let (mut lo, mut hi) = if nav {
            (self.airports, self.facilities)
        } else {
            (0, self.airports)
        };
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let v = self.facility(f, mid)?;
            match v.id.cmp(&id) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => return Some(mid),
            }
        }
        None
    }
    pub fn search(
        &self,
        f: &impl Flash,
        q: &[u8],
        nav: bool,
        skip: usize,
        out: &mut [u16; 24],
    ) -> usize {
        let mut n = 0;
        if q.is_empty() {
            return 0;
        }
        let (lo, hi) = if nav {
            (self.airports, self.facilities)
        } else {
            (0, self.airports)
        };
        for idx in lo..hi {
            if let Some(v) = self.facility(f, idx) {
                if matches(q, &v.id)
                    || (v.k_alias && matches(q, &v.display_id()))
                {
                    if n >= skip && n - skip < 24 {
                        out[n - skip] = idx;
                    }
                    n += 1;
                }
            }
        }
        if !nav {
            for i in 0..self.aliases {
                let mut b = [0; 6];
                if self.read(f, self.alias + i as u32 * 6, &mut b)
                    && matches(q, &b[..4])
                {
                    let idx = u16le(&b[4..]);
                    if let Some(v) = self.facility(f, idx) {
                        if v.nav
                            || matches(q, &v.id)
                            || (v.k_alias && matches(q, &v.display_id()))
                        {
                            continue;
                        }
                        if n >= skip && n - skip < 24 {
                            out[n - skip] = idx;
                        }
                        n += 1;
                    }
                }
            }
        }
        n
    }
    pub fn nearest(
        &self,
        f: &impl Flash,
        center: &Facility,
        nav: bool,
        radius: u16,
        out: &mut [u16; 24],
    ) -> usize {
        let mut distances = [u32::MAX; 24];
        let mut n = 0;
        let (lo, hi) = if nav {
            (self.airports, self.facilities)
        } else {
            (0, self.airports)
        };
        let max = (radius as u32 * 10 / 6).pow(2);
        for idx in lo..hi {
            let Some(v) = self.facility(f, idx) else {
                continue;
            };
            if v.id == center.id && v.nav == center.nav {
                continue;
            }
            let d = distance2(center, &v);
            if d > max {
                continue;
            }
            let pos = distances.iter().position(|x| d < *x).unwrap_or(24);
            if pos < 24 {
                for j in (pos + 1..24).rev() {
                    distances[j] = distances[j - 1];
                    out[j] = out[j - 1];
                }
                distances[pos] = d;
                out[pos] = idx;
                n = (n + 1).min(24);
            }
        }
        n
    }
}
pub fn matches(q: &[u8], code: &[u8]) -> bool {
    code.starts_with(q)
}

/// Four-position identifier editor. Up/Down cycles A-Z then 0-9.
pub struct Input {
    pub chars: [u8; 4],
    pub len: u8,
    pub cursor: u8,
}
impl Input {
    pub const fn new() -> Self {
        Self {
            chars: [b'A', 0, 0, 0],
            len: 1,
            cursor: 0,
        }
    }
    pub fn cycle(&mut self, up: bool) {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let current = self.chars[self.cursor as usize];
        let index = CHARS.iter().position(|c| *c == current).unwrap_or(0);
        let next = if up {
            (index + 1) % CHARS.len()
        } else {
            (index + CHARS.len() - 1) % CHARS.len()
        };
        self.chars[self.cursor as usize] = CHARS[next];
    }
    pub fn next(&mut self) {
        if self.cursor < 3 {
            self.cursor += 1;
            if self.cursor == self.len {
                self.chars[self.cursor as usize] = b'A';
                self.len += 1;
            }
        }
    }
    pub fn previous(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }
    pub fn delete(&mut self) -> bool {
        if self.len <= 1 {
            return false;
        }
        self.len -= 1;
        self.chars[self.len as usize] = 0;
        self.cursor = self.cursor.min(self.len - 1);
        true
    }
}
pub fn distance2(a: &Facility, b: &Facility) -> u32 {
    const COS: [i32; 19] = [
        1024, 1020, 1008, 989, 962, 928, 887, 839, 784, 724, 658, 587, 512,
        433, 350, 265, 178, 89, 0,
    ];
    let lat = (a.lat - b.lat) / 100;
    let mut lon = a.lon - b.lon;
    if lon > 1800000 {
        lon -= 3600000;
    }
    if lon < -1800000 {
        lon += 3600000;
    }
    let cos =
        COS[(((a.lat + b.lat).abs() / 2 + 25000) / 50000).min(18) as usize];
    let x = (lon / 100) * cos / 1024;
    (lat * lat + x * x) as u32
}
pub fn distance_nm(a: &Facility, b: &Facility) -> u32 {
    let n = distance2(a, b);
    let mut lo = 0;
    let mut hi = 40000;
    while lo < hi {
        let m = (lo + hi + 1) / 2;
        if m * m <= n {
            lo = m;
        } else {
            hi = m - 1;
        }
    }
    lo * 6 / 10
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Favorite {
    pub id: [u8; 4],
    pub hz: u32,
    pub nav: bool,
    pub service: u8,
}
#[derive(Clone, Copy)]
pub struct Prefs {
    pub favorites: [Favorite; 8],
    pub count: u8,
    pub anchor: [u8; 4],
    pub(crate) generation: u32,
    pub(crate) next: u8,
    pub(crate) sector: u8,
}
impl Default for Prefs {
    fn default() -> Self {
        Self {
            favorites: [Favorite::default(); 8],
            count: 0,
            anchor: [0; 4],
            generation: 0,
            next: 0,
            sector: 0,
        }
    }
}
impl Prefs {
    pub fn load(f: &impl Flash) -> Self {
        let mut result = Self::default();
        let mut found = false;
        for i in 0..64u8 {
            let mut b = [0; 128];
            if !f.read(PREF_BASE + i as u32 * 128, &mut b)
                || &b[..4] != b"AFV1"
                || !crc(!0, &b[..124]) != u32le(&b[124..])
                || b[8] > 8
            {
                continue;
            }
            if (0..b[8] as usize).any(|j| {
                let o = 16 + j * 12;
                let hz = u32le(&b[o + 4..]);
                b[o + 8] > 1
                    || b[o + 9] > 15
                    || !(108_000_000..=136_975_000).contains(&hz)
                    || !b[o..o + 4].iter().all(|c| {
                        *c == 0 || c.is_ascii_uppercase() || c.is_ascii_digit()
                    })
            }) {
                continue;
            }
            let generation = u32le(&b[4..]);
            if found && (generation.wrapping_sub(result.generation) as i32) <= 0
            {
                continue;
            }
            found = true;
            result.generation = generation;
            result.sector = i / 32;
            result.next = i % 32 + 1;
            result.count = b[8];
            result.anchor = b[12..16].try_into().unwrap_or([0; 4]);
            for j in 0..result.count as usize {
                let o = 16 + j * 12;
                result.favorites[j] = Favorite {
                    id: b[o..o + 4].try_into().unwrap_or([0; 4]),
                    hz: u32le(&b[o + 4..]),
                    nav: b[o + 8] != 0,
                    service: b[o + 9],
                };
            }
        }
        // Never program over an interrupted, non-erased record after reboot.
        if found {
            for i in result.next..32 {
                let mut b = [0; 128];
                if f.read(
                    PREF_BASE + result.sector as u32 * 4096 + i as u32 * 128,
                    &mut b,
                ) && b.iter().any(|v| *v != 255)
                {
                    result.next = i + 1;
                }
            }
        }
        result
    }
    pub fn save(&mut self, f: &impl Flash) -> bool {
        let mut sector = self.sector;
        let mut next = self.next;
        if next >= 32 {
            sector ^= 1;
            next = 0;
        }
        let addr = PREF_BASE + sector as u32 * 4096 + next as u32 * 128;
        if next == 0 && !f.erase(PREF_BASE + sector as u32 * 4096) {
            return false;
        }
        let mut b = [0xff; 128];
        b[..4].copy_from_slice(b"AFV1");
        let gen = self.generation.wrapping_add(1);
        b[4..8].copy_from_slice(&gen.to_le_bytes());
        b[8] = self.count;
        b[12..16].copy_from_slice(&self.anchor);
        for i in 0..self.count as usize {
            let v = self.favorites[i];
            let o = 16 + i * 12;
            b[o..o + 4].copy_from_slice(&v.id);
            b[o + 4..o + 8].copy_from_slice(&v.hz.to_le_bytes());
            b[o + 8] = v.nav as u8;
            b[o + 9] = v.service;
        }
        let check = !crc(!0, &b[..124]);
        b[124..].copy_from_slice(&check.to_le_bytes());
        if !f.write(addr, &b) {
            self.next = 32;
            return false;
        }
        let mut got = [0; 128];
        if !f.read(addr, &mut got) || got != b {
            self.next = 32;
            return false;
        }
        self.sector = sector;
        self.next = next + 1;
        self.generation = gen;
        true
    }
    pub fn toggle(&mut self, f: &impl Flash, v: Favorite) -> Option<bool> {
        let old = *self;
        let added = if let Some(i) = self.favorites[..self.count as usize]
            .iter()
            .position(|x| *x == v)
        {
            for j in i..self.count as usize - 1 {
                self.favorites[j] = self.favorites[j + 1];
            }
            self.count -= 1;
            false
        } else {
            if self.count == 8 {
                return None;
            }
            self.favorites[self.count as usize] = v;
            self.count += 1;
            true
        };
        if !self.save(f) {
            let next = self.next;
            *self = old;
            self.next = next;
            return None;
        }
        Some(added)
    }
}
