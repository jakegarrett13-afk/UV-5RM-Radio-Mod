"""Back up, upload and verify AIRBAND data/app on AIRBAND-enabled firmware."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib
from database import BASE, LIMIT, FAVORITES_BASE, validate

ROOT = Path(__file__).resolve().parents[2]
SLOT_BASE, SLOT_BYTES, CHUNK = 0x14C000, 32768, 512


def validate_app(app: bytes):
    if len(app)<108 or len(app)>SLOT_BYTES: raise ValueError('Invalid app length')
    magic, abi, count = struct.unpack_from('<4sIB', app)
    expected = zlib.crc32((ROOT/'firmware-source/abi/src/lib.rs').read_bytes())
    if magic!=b'OVL1' or abi!=expected or count!=4 or app[12:28].split(b'\0')[0]!=b'AIRBAND':
        raise ValueError('Wrong AIRBAND app/ABI')
    end=108
    for i in range(count):
        off,n,bss,entry,crc=struct.unpack_from('<5I',app,28+i*20)
        if off<end or off%4 or n+bss>8192 or entry>=n or off+n>len(app): raise ValueError('Invalid segment bounds')
        if zlib.crc32(app[off:off+n])!=crc: raise ValueError('App segment CRC mismatch')
        end=off+n
    if end!=len(app): raise ValueError('Unexpected app trailing bytes')


def request(session,cmd,addr,data=b'',timeout=2.0):
    got,wire,payload=session.send(cmd,addr,data,timeout=timeout)
    if got!=cmd or wire!=addr: raise RuntimeError('Wrong response command/address')
    return payload


def raw(session,start,length):
    out=bytearray()
    for addr in range(start,start+length,64):
        b=request(session,0x44,addr//64)
        if len(b)!=64: raise RuntimeError('Truncated flash read')
        out+=b
    return bytes(out[:length])


def install(session,app,blob,backup,slot=5,database_only=False,progress=print):
    # Finish all host validation and the read-only capability query before
    # any erase/write, including before a potentially long backup.
    info=validate(blob)
    if not database_only: validate_app(app)
    if slot!=5: raise ValueError('The AIRBAND Band-menu profile reserves app slot 5')
    if request(session,0x53,0)!=b'AIR1': raise RuntimeError('AIRBAND firmware capability missing')
    backup=Path(backup);backup.mkdir(parents=True,exist_ok=False)
    old=raw(session,BASE,LIMIT+8192)
    (backup/'previous-data-and-favorites.bin').write_bytes(old)
    if not database_only:
        oldapp=raw(session,SLOT_BASE+slot*SLOT_BYTES,SLOT_BYTES)
        (backup/'previous-slot.app').write_bytes(oldapp)
    (backup/'manifest.json').write_text(json.dumps({'slot':slot,'data_address':BASE,
        'data_bytes':LIMIT,'favorites_address':FAVORITES_BASE,'previous_data_sha256':hashlib.sha256(old).hexdigest(),
        'new_database':info},indent=2)+'\n')
    progress('Backups saved; uploading database')
    padded=blob.ljust(-(-len(blob)//CHUNK)*CHUNK,b'\xff')
    for off in range(0,len(padded),CHUNK):
        if off+CHUNK>LIMIT: raise ValueError('Database exceeds reserved region')
        if request(session,0x53,off//CHUNK,padded[off:off+CHUNK],timeout=30 if off==0 else 2):
            raise RuntimeError('Unexpected write response')
        if off%(64*CHUNK)==0: progress(f'Database {off+CHUNK}/{len(padded)} bytes')
    got=raw(session,BASE,len(blob));validate(got)
    if got!=blob: raise RuntimeError('Database readback mismatch')
    if raw(session,FAVORITES_BASE,8192)!=old[LIMIT:]: raise RuntimeError('Favorites changed during upload')
    progress('Database CRC/readback verified; favorites preserved')
    if not database_only:
        request(session,0x41,slot,timeout=30)
        padded=app.ljust(-(-len(app)//CHUNK)*CHUNK,b'\xff')
        for off in range(0,len(padded),CHUNK): request(session,0x61,off,padded[off:off+CHUNK])
        if raw(session,SLOT_BASE+slot*SLOT_BYTES,len(app))!=app: raise RuntimeError('App readback mismatch')
        progress('AIRBAND app readback verified')
    (backup/'installed.json').write_text(json.dumps({'database_sha256':info['sha256'],
        'app_sha256':None if database_only else hashlib.sha256(app).hexdigest(),'verified':True},indent=2)+'\n')


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('port');p.add_argument('--database',type=Path,default=ROOT/'airband/airband.db')
    p.add_argument('--app',type=Path,default=ROOT/'apps/airband.app')
    p.add_argument('--backup',type=Path,required=True,help='new backup directory; existing directories are refused')
    p.add_argument('--slot',type=int,choices=[5],default=5,help='fixed zero-based AIRBAND slot 5; replaces FOX HUNT')
    p.add_argument('--database-only',action='store_true')
    args=p.parse_args();blob=args.database.read_bytes();validate(blob)
    app=b'' if args.database_only else args.app.read_bytes()
    if not args.database_only: validate_app(app)
    if args.backup.exists():p.error('Backup directory already exists')
    sys.path.insert(0,str(ROOT/'firmware-source/tools'))
    from cps_proto import CpsSession
    with CpsSession(args.port) as session: install(session,app,blob,args.backup,args.slot,args.database_only)
    print('Done; radio restarting. Unplug the programming cable to hear the speaker.')


if __name__=='__main__': main()
