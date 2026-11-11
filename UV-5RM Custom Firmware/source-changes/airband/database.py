"""FAA NASR -> AIR1 compact offline radio database (stdlib only)."""
from __future__ import annotations
import argparse
import csv
from datetime import date
from decimal import Decimal, InvalidOperation
import hashlib
import io
import json
from pathlib import Path
import re
import struct
import unicodedata
import urllib.request
import zipfile
import zlib

BASE, LIMIT = 0x194000, 0x5A000
FAVORITES_BASE, TEXT_BASE = 0x1EE000, 0x1F0000
HEADER, RECORD = 64, 16
SERVICES = ('TOWER', 'GROUND', 'ATIS', 'AWOS/ASOS', 'CTAF', 'UNICOM',
            'CLEARANCE', 'APPROACH', 'DEPARTURE', 'APP/DEP', 'CENTER',
            'FSS/RCO', 'OTHER', 'VOR', 'WEATHER', 'EMERGENCY')


def ascii_name(text: str, limit=22) -> str:
    s = unicodedata.normalize('NFKD', text).encode('ascii', 'ignore').decode().upper()
    return ''.join(c if 32 <= ord(c) <= 95 else ' ' for c in s).strip()[:limit]


def pack_name(name: str) -> bytes:
    name = ascii_name(name)
    bits = 0
    for i, c in enumerate(name):
        bits |= (ord(c) - 32) << (6 * i)
    return bytes([len(name)]) + bits.to_bytes((len(name) * 6 + 7) // 8, 'little')


def unpack_name(data: bytes, off: int) -> str:
    length = data[off]
    if length > 22 or off + 1 + (length * 6 + 7) // 8 > len(data):
        raise ValueError('Invalid name')
    bits = int.from_bytes(data[off + 1:off + 1 + (length * 6 + 7) // 8], 'little')
    return ''.join(chr(32 + ((bits >> (i * 6)) & 63)) for i in range(length))


def classify(use: str) -> int:
    u = use.upper()
    if 'ATIS' in u: return 2
    if 'AWOS' in u or 'ASOS' in u: return 3
    if 'CTAF' in u: return 4
    if 'UNICOM' in u: return 5
    if 'EMERG' in u or 'GUARD' in u: return 15
    if 'GND' in u or 'GROUND' in u: return 1
    if 'LCL' in u or 'TWR' in u or 'TOWER' in u: return 0
    if 'APCH' in u or 'APPROACH' in u:
        return 9 if 'DEP' in u else 7
    if 'DEP' in u: return 8
    if u.startswith('CD') or 'CLEARANCE' in u: return 6
    if 'ARTCC' in u or 'CENTER' in u or 'CLASS A' in u: return 10
    if 'FSS' in u or 'RCO' in u or 'FISE' in u: return 11
    if 'WX' in u or 'WEATHER' in u or 'METRO' in u: return 14
    return 12


def frequency(value: str, nav=False) -> int | None:
    try:
        hz = Decimal(value.strip()) * 1_000_000
    except InvalidOperation:
        return None
    if not hz.is_finite(): return None
    low, high = (108_000_000, 117_950_000) if nav else (118_000_000, 136_975_000)
    # US 25/50 kHz assignments are exact. Do not round channel designators,
    # UHF, telephone numbers, or free-text/8.33 kHz entries into a frequency.
    if not low <= hz <= high or hz % 5000:
        return None
    return int(hz)


def table(folder: Path, group: str, name: str):
    with zipfile.ZipFile(folder / (group + '.zip')) as archive:
        with archive.open(name) as f:
            yield from csv.DictReader(io.TextIOWrapper(f, encoding='utf-8-sig'))


def collect(folder: Path, states=()):
    airports = {}
    for r in table(folder, 'APT', 'APT_BASE.csv'):
        if r['COUNTRY_CODE'] != 'US' or (states and r['STATE_CODE'] not in states):
            continue
        if r.get('ARPT_STATUS', r.get('STATUS_CODE', 'O')) in ('C', 'CLOSED'):
            continue
        try:
            lat, lon = float(r['LAT_DECIMAL']), float(r['LONG_DECIMAL'])
        except (KeyError, ValueError):
            continue
        airports[r['ARPT_ID']] = {'id': r['ARPT_ID'], 'icao': r.get('ICAO_ID', ''),
            'kind': 0, 'name': r['ARPT_NAME'], 'lat': lat, 'lon': lon, 'frequencies': set()}
    for r in table(folder, 'FRQ', 'FRQ.csv'):
        a = airports.get(r['SERVICED_FACILITY'])
        hz = frequency(r['FREQ'])
        if a is not None and hz is not None and r['SERVICED_COUNTRY'] == 'US':
            a['frequencies'].add((classify(r['FREQ_USE']), hz))
    facilities = [a for a in airports.values() if a['frequencies']]
    navs = {}
    for r in table(folder, 'NAV', 'NAV_BASE.csv'):
        hz = frequency(r['FREQ'], nav=True)
        if (r['COUNTRY_CODE'] != 'US' or 'VOR' not in r['NAV_TYPE'] or hz is None
            or not r['NAV_STATUS'].startswith('OPERATIONAL')
            or (states and r['STATE_CODE'] not in states)):
            continue
        try:
            lat, lon = float(r['LAT_DECIMAL']), float(r['LONG_DECIMAL'])
        except ValueError:
            continue
        if r['NAV_ID'] in navs:
            raise ValueError('Ambiguous duplicate VOR identifier: ' + r['NAV_ID'])
        navs[r['NAV_ID']] = {'id': r['NAV_ID'], 'icao': '', 'kind': 1, 'name': r['NAME'],
            'lat': lat, 'lon': lon, 'frequencies': {(13, hz)}}
    return facilities + list(navs.values())


def encode(facilities, effective: str):
    date.fromisoformat(effective)
    facilities = sorted(facilities, key=lambda f: (f['kind'], f['id']))
    records, freqs, aliases, names = bytearray(), bytearray(), [], bytearray()
    name_offsets, seen = {}, set()
    airports = sum(f['kind'] == 0 for f in facilities)
    if len(facilities) > 65535: raise ValueError('Too many facilities')
    for idx, f in enumerate(facilities):
        ident, kind = f['id'], f['kind']
        if not re.fullmatch('[A-Z0-9]{1,4}', ident) or kind not in (0, 1):
            raise ValueError('Invalid identifier/kind')
        if (kind, ident) in seen: raise ValueError('Duplicate identifier')
        seen.add((kind, ident))
        lat, lon = f['lat'], f['lon']
        if not -90 <= lat <= 90 or not -180 <= lon <= 180: raise ValueError('Invalid coordinate')
        fs = sorted(set(f['frequencies']))
        if not 1 <= len(fs) <= 64 or len(freqs) // 3 + len(fs) > 65535:
            raise ValueError('Too many frequencies')
        start = len(freqs) // 3
        for svc, hz in fs:
            if not 0 <= svc < len(SERVICES) or hz % 5000 or not 108_000_000 <= hz <= 136_975_000:
                raise ValueError('Unsupported frequency/service')
            if (kind == 1) != (svc == 13) or (kind == 0 and hz < 118_000_000) or (kind == 1 and hz > 117_950_000):
                raise ValueError('Frequency does not match facility type')
            freqs += struct.pack('<HB', (hz - 108_000_000) // 5000, svc)
        name = ascii_name(f['name'])
        if name not in name_offsets:
            name_offsets[name] = len(names); names += pack_name(name)
        code = bytearray(ident.encode().ljust(4, b'\0'))
        code[0] |= kind << 7
        icao = f.get('icao', '').strip().upper()
        if icao and icao != ident:
            if not re.fullmatch('[A-Z0-9]{4}', icao): raise ValueError('Invalid ICAO alias')
            if len(ident) == 3 and icao == 'K' + ident:
                code[1] |= 0x80
            else:
                aliases.append((icao, idx))
        records += code
        records += round(lat * 10000).to_bytes(3, 'little', signed=True)
        records += round(lon * 10000).to_bytes(3, 'little', signed=True)
        records += struct.pack('<HB', start, len(fs))
        records += name_offsets[name].to_bytes(3, 'little')
    alias_data = b''.join(k.encode() + struct.pack('<H', idx) for k, idx in sorted(aliases))
    offsets = (HEADER, HEADER + len(records), HEADER + len(records) + len(freqs),
               HEADER + len(records) + len(freqs) + len(alias_data))
    payload = bytes(records + freqs + alias_data + names)
    if HEADER + len(payload) > LIMIT:
        raise ValueError(f'Database {HEADER+len(payload)} bytes exceeds {LIMIT}; select --states')
    h = bytearray(HEADER)
    struct.pack_into('<4s6H5I', h, 0, b'AIR1', 1, len(facilities), len(freqs)//3,
                     len(aliases), airports, HEADER, *offsets, HEADER+len(payload))
    h[36:44] = effective.replace('-', '').encode()
    struct.pack_into('<I', h, 44, zlib.crc32(payload))
    struct.pack_into('<I', h, 60, zlib.crc32(h[:60]))
    blob = bytes(h) + payload
    validate(blob)
    return blob


def validate(blob: bytes):
    if len(blob) < HEADER: raise ValueError('Short database')
    magic, ver, nf, nq, na, nap, size, ro, fo, ao, no, end = struct.unpack_from('<4s6H5I', blob)
    if magic != b'AIR1' or ver != 1 or size != HEADER or not nf or nap > nf:
        raise ValueError('Invalid database header')
    if (ro != HEADER or fo != ro+nf*RECORD or ao != fo+nq*3 or no != ao+na*6
        or not no <= end <= LIMIT or end != len(blob)):
        raise ValueError('Invalid database bounds')
    if zlib.crc32(blob[:60]) != struct.unpack_from('<I', blob, 60)[0] or zlib.crc32(blob[HEADER:]) != struct.unpack_from('<I', blob, 44)[0]:
        raise ValueError('Database CRC mismatch')
    previous = None
    for i in range(nf):
        off = ro + i*RECORD; code = bytes(v & 127 for v in blob[off:off+4]).rstrip(b'\0').decode('ascii')
        kind = blob[off] >> 7
        if not re.fullmatch('[A-Z0-9]{1,4}', code) or kind != (0 if i < nap else 1): raise ValueError('Invalid facility')
        if previous is not None and (kind, code) <= previous: raise ValueError('Unsorted facilities')
        previous = (kind, code)
        lat = int.from_bytes(blob[off+4:off+7], 'little', signed=True)
        lon = int.from_bytes(blob[off+7:off+10], 'little', signed=True)
        start, count = struct.unpack_from('<HB', blob, off+10)
        if abs(lat)>900000 or abs(lon)>1800000 or not count or start+count>nq: raise ValueError('Invalid facility fields')
        nameoff = no + int.from_bytes(blob[off+13:off+16], 'little')
        if nameoff >= end: raise ValueError('Invalid name offset')
        unpack_name(blob, nameoff)
        for j in range(start, start+count):
            step, svc = struct.unpack_from('<HB', blob, fo+j*3)
            hz = 108_000_000 + step*5000
            if svc>15 or (kind==0 and not 118_000_000<=hz<=136_975_000) or (kind==1 and not 108_000_000<=hz<=117_950_000) or (svc==13)!=(kind==1): raise ValueError('Invalid frequency')
    previous = None
    for i in range(na):
        code, idx = struct.unpack_from('<4sH', blob, ao+i*6)
        if not re.fullmatch(b'[A-Z0-9]{4}', code) or idx >= nap or (previous and code < previous): raise ValueError('Invalid alias')
        previous = code
    return {'facilities': nf, 'airports': nap, 'navaids': nf-nap, 'frequencies': nq,
            'aliases': na, 'bytes': end, 'capacity': LIMIT, 'effective': blob[36:44].decode(),
            'sha256': hashlib.sha256(blob).hexdigest()}


def download(folder: Path, effective: str):
    d = date.fromisoformat(effective); folder.mkdir(parents=True, exist_ok=True)
    tag = f'{d.day:02d}_{d.strftime("%b")}_{d.year}'
    for group in ('APT', 'FRQ', 'NAV'):
        url = f'https://nfdc.faa.gov/webContent/28DaySub/extra/{tag}_{group}_CSV.zip'
        data = urllib.request.urlopen(url, timeout=60).read()
        with zipfile.ZipFile(io.BytesIO(data)) as z:
            # A valid ZIP for a different cycle must not get a current label.
            filename = {'APT':'APT_BASE.csv','FRQ':'FRQ.csv','NAV':'NAV_BASE.csv'}[group]
            sample = next(csv.DictReader(io.StringIO(z.read(filename).decode('utf-8-sig'))))
            if sample['EFF_DATE'].replace('/', '-') != effective: raise ValueError('Wrong FAA effective cycle')
        (folder/(group+'.zip')).write_bytes(data)
        print('downloaded', group, len(data), 'bytes')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', type=Path, required=True, help='directory containing APT.zip, FRQ.zip, NAV.zip')
    p.add_argument('--effective', required=True, help='FAA effective date YYYY-MM-DD')
    p.add_argument('--download', action='store_true')
    p.add_argument('--states', nargs='*', default=[])
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    if args.download: download(args.source, args.effective)
    # Validate every source group before labeling a locally cached build.
    for group, name in [('APT','APT_BASE.csv'),('FRQ','FRQ.csv'),('NAV','NAV_BASE.csv')]:
        if next(table(args.source, group, name))['EFF_DATE'].replace('/', '-') != args.effective:
            raise ValueError('Source data effective date mismatch')
    blob = encode(collect(args.source, [s.upper() for s in args.states]), args.effective)
    args.output.parent.mkdir(parents=True, exist_ok=True); args.output.write_bytes(blob)
    info = validate(blob); info.update({'states':args.states,'source':'FAA 28-day NASR',
        'source_sha256':{g:hashlib.sha256((args.source/(g+'.zip')).read_bytes()).hexdigest() for g in ('APT','FRQ','NAV')},
        'coverage':'US airports with supported published VHF frequencies and operational VOR-family navaids; names abbreviated to 22 characters; duplicate service/frequency entries merged; sector/remarks omitted'})
    args.output.with_suffix('.json').write_text(json.dumps(info, indent=2)+'\n')
    print(json.dumps(info, indent=2))


if __name__ == '__main__': main()
