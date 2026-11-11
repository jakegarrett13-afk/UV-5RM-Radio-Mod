import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
import struct
import zlib
from database import *
from install import install,validate_app,ROOT


def fixture():
    return encode([dict(id='AAA',icao='KAAA',kind=0,name='TEST AIRPORT',lat=40,lon=-90,
                        frequencies=[(0,123450000),(1,121900000)]),
                   dict(id='ANC',icao='PANC',kind=0,name='TEST ALASKA',lat=61,lon=-150,frequencies=[(2,118300000)]),
                   dict(id='AAA',icao='',kind=1,name='TEST VOR',lat=40.2,lon=-90,frequencies=[(13,114000000)])], '2026-10-29')


class Fake:
    def __init__(self,caps=True,fail=False):self.mem=bytearray(b'\xff')*0x200000;self.caps=caps;self.writes=[];self.slot=None;self.fail=fail
    def send(self,cmd,addr,data=b'',timeout=2):
        if cmd==0x44:out=bytes(self.mem[addr*64:addr*64+64])
        elif cmd==0x53 and not data:out=b'AIR1' if self.caps else b'OLD'
        elif cmd==0x53:
            off=addr*512
            if len(data)!=512 or off+512>LIMIT:raise ValueError('Out of bounds')
            if self.fail:raise RuntimeError('Disconnected')
            if off==0:self.mem[BASE:BASE+LIMIT]=b'\xff'*LIMIT
            self.writes.append(BASE+off);self.mem[BASE+off:BASE+off+512]=data;out=b''
        elif cmd==0x41:self.slot=addr;self.mem[0x14c000+addr*32768:0x14c000+(addr+1)*32768]=b'\xff'*32768;out=b''
        elif cmd==0x61:
            a=0x14c000+self.slot*32768+addr;self.writes.append(a);self.mem[a:a+512]=data;out=b''
        else:raise ValueError('Unexpected command')
        return cmd,addr,out


class DatabaseTests(unittest.TestCase):
    def test_roundtrip_and_aliases(self):
        b=fixture();v=validate(b);self.assertEqual((v['airports'],v['navaids'],v['aliases']),(2,1,1))
        no=struct.unpack_from('<I',b,28)[0];self.assertTrue(unpack_name(b,no).startswith('TEST'))
    def test_corruption_and_truncation(self):
        b=fixture()
        for x in [b[:50],b[:-1],b[:70]+bytes([b[70]^1])+b[71:]]:
            with self.assertRaises(ValueError):validate(x)
    def test_rejects_rounded_or_wrong_band_frequencies(self):
        for x in ['121.50833','243.0','not a frequency','137.0']:self.assertIsNone(frequency(x))
        self.assertEqual(frequency('136.975'),136975000)
        self.assertIsNone(frequency('114.0'));self.assertEqual(frequency('114.0',nav=True),114000000)
    def test_service_labels(self):
        for name,svc in [('LCL/P',0),('GND/S',1),('ATIS',2),('AWOS-3',3),('CTAF',4),('CD/P',6),('APCH/P DEP/P',9)]:self.assertEqual(classify(name),svc)
    def test_duplicate_id_in_distinct_namespaces(self):
        self.assertEqual(validate(fixture())['facilities'],3)
    def test_bad_alias_and_geometry_rejected(self):
        for kwargs in [dict(icao='BAD'),dict(lat=91)]:
            f=dict(id='AAA',icao='KAAA',kind=0,name='TEST',lat=40,lon=-90,frequencies=[(0,123450000)]);f.update(kwargs)
            with self.assertRaises(ValueError):encode([f],'2026-10-29')
    def test_band_menu_slot_cannot_be_misrouted(self):
        f=Fake()
        with TemporaryDirectory() as d:
            with self.assertRaises(ValueError):install(f,b'',fixture(),Path(d)/'backup',slot=4,database_only=True,progress=lambda x:None)
        self.assertEqual(f.writes,[])
    def test_wrong_firmware_rejected_without_writes(self):
        f=Fake(caps=False)
        with TemporaryDirectory() as d:
            with self.assertRaises(RuntimeError):install(f,b'',fixture(),Path(d)/'backup',database_only=True,progress=lambda x:None)
        self.assertEqual(f.writes,[])
    def test_upload_readback_and_reserved_regions(self):
        f=Fake();f.mem[FAVORITES_BASE:FAVORITES_BASE+8192]=b'F'*8192;f.mem[0x1f0000:0x1f4000]=b'T'*16384
        with TemporaryDirectory() as d:install(f,b'',fixture(),Path(d)/'backup',database_only=True,progress=lambda x:None)
        self.assertEqual(f.mem[FAVORITES_BASE:FAVORITES_BASE+8192],b'F'*8192);self.assertEqual(f.mem[0x1f0000:0x1f4000],b'T'*16384)
        self.assertTrue(all(BASE<=a<BASE+LIMIT for a in f.writes))
    def test_interruption_retains_backups_and_app(self):
        f=Fake(fail=True);start=0x14c000+5*32768;before=bytes(f.mem[start:start+32768])
        with TemporaryDirectory() as d:
            backup=Path(d)/'backup'
            with self.assertRaises(RuntimeError):install(f,b'',fixture(),backup,database_only=True,progress=lambda x:None)
            self.assertTrue((backup/'previous-data-and-favorites.bin').exists())
        self.assertEqual(bytes(f.mem[start:start+32768]),before)
    def test_full_app_install_and_slot_preservation(self):
        f=Fake();app=(ROOT/'apps/airband.app').read_bytes();start=0x14c000+5*32768
        with TemporaryDirectory() as d:install(f,app,fixture(),Path(d)/'backup',progress=lambda x:None)
        self.assertEqual(bytes(f.mem[start:start+len(app)]),app)
        self.assertTrue(all(BASE<=a<BASE+LIMIT or start<=a<start+32768 for a in f.writes))

    def test_real_app_package(self):
        app=(ROOT/'apps/airband.app').read_bytes();validate_app(app)
        broken=bytearray(app);broken[-1]^=1
        with self.assertRaises(ValueError):validate_app(bytes(broken))


if __name__=='__main__':unittest.main()
