"""Build/package AIRBAND without requiring LLVM command-line utilities."""
import argparse
import hashlib
import os
from pathlib import Path
import struct
import subprocess
import zlib

ROOT = Path(__file__).resolve().parents[2]
FW = ROOT / 'firmware-source'
APP = FW / 'apps/airband'
ORIGIN = 0x200001D0


def extract(path):
    data = path.read_bytes()
    if data[:6] != b'\x7fELF\x01\x01': raise ValueError('Expected little-endian ELF32')
    offset = struct.unpack_from('<I', data, 32)[0]
    size, count, names_idx = struct.unpack_from('<HHH', data, 46)
    sections = [struct.unpack_from('<10I', data, offset+i*size) for i in range(count)]
    name_base = sections[names_idx][4]
    def string(base, index): return data[base+index:data.index(b'\0', base+index)].decode()
    image = bytearray(); bss = 0; entry = None
    for section in sections:
        _, typ, flags, addr, start, length, link, *_ = section
        if flags & 2 and length:
            if typ == 8: bss += length; continue
            rel = addr-ORIGIN
            if rel < 0: raise ValueError('Unexpected allocated section address')
            image.extend(bytes(max(0, rel+length-len(image))))
            image[rel:rel+length] = data[start:start+length]
        if typ == 2:
            for i in range(length//16):
                n, value, *_ = struct.unpack_from('<IIIBBH', data, start+i*16)
                if string(sections[link][4], n) == 'app_entry': entry = value-ORIGIN
    if entry is None or not 0<=entry<len(image): raise ValueError('Missing entry point')
    if len(image)+bss > 8192: raise ValueError(f'App needs {len(image)+bss}/8192 bytes (image {len(image)}, bss {bss})')
    return bytes(image), bss, entry


def build(target, output):
    env = dict(os.environ, RUSTFLAGS='-C link-arg=-Tapp.x -C link-arg=--nmagic')
    body=bytearray();entries=[]
    for name,folder in [('airband',APP),('airband-receiver',APP/'receiver'),('airband-nearby',APP/'nearby'),('airband-listener',APP/'listener')]:
        dest=target/name
        subprocess.run(['cargo','build','--release','--locked','--target-dir',str(dest)],cwd=folder,env=env,check=True)
        image,bss,entry=extract(dest/'thumbv7em-none-eabi/release'/name)
        body += bytes(-len(body)%4)
        entries.append(struct.pack('<5I',108+len(body),len(image),bss,entry,zlib.crc32(image)))
        body += image
        print(f'{name}: image={len(image)} bss={bss} total={len(image)+bss}/8192')
    abi=zlib.crc32((FW/'abi/src/lib.rs').read_bytes())
    package=struct.pack('<4sIB3x16s',b'OVL1',abi,4,b'AIRBAND')+b''.join(entries)+body
    if len(package)>32768:raise ValueError('Package exceeds slot')
    output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(package)
    print(f'AIRBAND package={len(package)} SHA256={hashlib.sha256(package).hexdigest()}')



if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--target-dir',type=Path,default=Path('/tmp/uv5rm-airband-app'));p.add_argument('--output',type=Path,default=ROOT/'apps/airband.app');args=p.parse_args();build(args.target_dir.resolve(),args.output.resolve())
