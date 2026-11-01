# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Synthetic format tests; optional real pinned installer and Windows resource-only API checks."""
import argparse,hashlib,json,lzma,os,struct,subprocess,sys,tempfile,unittest,zlib
from pathlib import Path
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'tools'))
import build_client
from client_format import Invalid,PE32,SolidInstaller
from client_patch import Edit,apply_edits,domain_edits,endpoint_text,patch_client,sha,CLIENT_SIZE

def synthetic_pe():
    data=bytearray(0x3000);data[:2]=b'MZ';struct.pack_into('<I',data,60,128);data[128:132]=b'PE\0\0'
    struct.pack_into('<HH',data,132,0x14c,1);struct.pack_into('<H',data,148,224)
    struct.pack_into('<H',data,152,0x10b);struct.pack_into('<I',data,180,0x400000);struct.pack_into('<I',data,244,16)
    struct.pack_into('<II',data,264,0x1000,0x1000)
    data[376:384]=b'.rsrc\0\0\0';struct.pack_into('<IIII',data,384,0x1000,0x1000,0x1000,0x1000)
    for offset,label,child in [(0,6,0x80000020),(32,1,0x80000040),(64,1045,96)]:
        struct.pack_into('<H',data,0x1000+offset+14,1);struct.pack_into('<II',data,0x1000+offset+16,label,child)
    strings=['Przed','http://appmsg.gadu-gadu.pl/path?x=1','Dostępny','<a href="https://www.gadu-gadu.pl">TRAIL</a>','pre\0retr.gadu-gadu.pl\0suffix']+['']*11
    block=b''.join(struct.pack('<H',len(t.encode('utf-16le'))//2)+t.encode('utf-16le') for t in strings)+bytes(6)
    struct.pack_into('<IIII',data,0x1060,0x1100,len(block),0,0);data[0x1100:0x1100+len(block)]=block
    marker=0x1100+len(block);data[marker:marker+4]=b'KEEP'
    ascii_url=b'http://www.gadu-gadu.pl" target="_blank">TRAIL\0';data[0x2100:0x2100+len(ascii_url)]=ascii_url
    wide_url='http://retr.gadu-gadu.pl/path\0'.encode('utf-16le');data[0x2200:0x2200+len(wide_url)]=wide_url
    return bytes(data),strings,marker

def synthetic_installer(stream=None):
    if stream is None:stream=struct.pack('<I',64)+b'S'*64+b'beforeFAKE-EXEafter'
    props=b'\x5d'+struct.pack('<I',4096)
    compressed=lzma.compress(stream,format=lzma.FORMAT_RAW,filters=SolidInstaller.filters(props))
    stub=b'MZ'+bytes(510)
    header=struct.pack('<7I',0,0xdeadbeef,0x6c6c754e,0x74666f73,0x74736e49,64,37+len(compressed))
    data=stub+header+props+compressed
    return data+struct.pack('<I',zlib.crc32(data[512:]))

class FormatTests(unittest.TestCase):
    def test_string_tables_c_strings_suffixes_and_boundaries(self):
        data,original,marker=synthetic_pe();pe=PE32(data);resource=pe.resources()[0]
        self.assertEqual(pe.strings(resource)[0],original)
        for destination in ['127.0.0.1','abcdefghijklmnop']:
            edits=domain_edits(data,destination);updated=apply_edits(data,edits)
            self.assertEqual(PE32(updated).strings(resource)[0],[endpoint_text(s,destination) for s in original])
            self.assertEqual(updated[marker:marker+4],b'KEEP');self.assertEqual(updated[:0x1000],data[:0x1000])
            self.assertIn(('http://'+destination+'" target="_blank">TRAIL\0').encode(),updated)
            self.assertIn(('http://'+destination+'/path\0').encode('utf-16le'),updated)
    def test_malformed_pe_and_patch_preconditions(self):
        data,_,_=synthetic_pe()
        for offset,value in [(60,0xffffffff),(0x1014,0x80000000),(0x1060,0xffffffff)]:
            bad=bytearray(data);struct.pack_into('<I',bad,offset,value)
            with self.assertRaises(Invalid):PE32(bad).resources()
        with self.assertRaises(Invalid):apply_edits(b'abcdef',[Edit(0,b'ab',b'AB','a'),Edit(1,b'bc',b'BC','b')])
        with self.assertRaises(Invalid):apply_edits(b'abcdef',[Edit(0,b'XX',b'AB','a')])
        with self.assertRaises(Invalid):patch_client(data,'127.0.0.1')
    def test_solid_installer_roundtrip_and_rejection(self):
        data=synthetic_installer();source=SolidInstaller.parse(data)
        result=source.replace_member(74,b'FAKE-EXE',b'NEW-EXE!');parsed=SolidInstaller.parse(result)
        self.assertEqual(result[:512],data[:512]);self.assertEqual(parsed.stream,source.stream.replace(b'FAKE-EXE',b'NEW-EXE!'))
        for bad in [data[:-1],data[:-4]+bytes(4),synthetic_installer(b'wrong header')]:
            with self.assertRaises(Invalid):SolidInstaller.parse(bad)
        with self.assertRaises(Invalid):source.replace_member(4,b'S',b'X')
        bomb=synthetic_installer(struct.pack('<I',64)+bytes(17*1024*1024))
        with self.assertRaises(Invalid):SolidInstaller.parse(bomb)
    def test_no_manifest_on_interrupted_publication_and_no_overwrite(self):
        with tempfile.TemporaryDirectory(prefix='rgg-client-write-') as tmp:
            root=Path(tmp);out=root/'partial';content={n:b'synthetic' for n in ['gg.exe','gg-setup.exe','README.txt','receipt.json','manifest.toml']}
            original=build_client.write_new
            def fail(path,value):
                if path.name=='receipt.json':raise OSError('synthetic write failure')
                original(path,value)
            with patch.object(build_client,'write_new',fail):
                with self.assertRaises(OSError):build_client.publish(out,content)
            self.assertFalse((out/'manifest.toml').exists())
            with self.assertRaises(FileExistsError):build_client.publish(out,content)
            complete=root/'complete';build_client.publish(complete,content);self.assertTrue((complete/'manifest.toml').is_file())
    def test_config_bounds_and_no_binary_fetch(self):
        base=(ROOT/'config/client.example.toml').read_text(encoding='utf-8')
        with tempfile.TemporaryDirectory(prefix='rgg-client-config-') as tmp:
            path=Path(tmp)/'client.toml';path.write_text(base,encoding='utf-8');self.assertEqual(build_client.config(path)['hostname'],'127.0.0.1')
            for value in [base.replace('127.0.0.1','a'*17),base.replace('127.0.0.1','evil/<x>'),base+'unknown="forbidden"',base.replace('https://','http://'),base.replace('https://','HTTPS://'),'a'*16385]:
                path.write_text(value,encoding='utf-8')
                with self.assertRaises((Invalid,ValueError)):build_client.config(path)

def windows_strings(path,ids):
    import ctypes
    from ctypes import wintypes
    kernel=ctypes.WinDLL('kernel32',use_last_error=True);ui=ctypes.WinDLL('user32',use_last_error=True)
    kernel.LoadLibraryExW.argtypes=[wintypes.LPCWSTR,wintypes.HANDLE,wintypes.DWORD];kernel.LoadLibraryExW.restype=wintypes.HMODULE
    kernel.FreeLibrary.argtypes=[wintypes.HMODULE];kernel.FreeLibrary.restype=wintypes.BOOL
    ui.LoadStringW.argtypes=[wintypes.HINSTANCE,wintypes.UINT,wintypes.LPWSTR,ctypes.c_int];ui.LoadStringW.restype=ctypes.c_int
    # DATAFILE_EXCLUSIVE + IMAGE_RESOURCE: no imports, DllMain or executable entry point.
    module=kernel.LoadLibraryExW(str(path.resolve()),None,0x40|0x20)
    if not module:raise ctypes.WinError(ctypes.get_last_error())
    try:
        result={}
        for identifier in ids:
            buf=ctypes.create_unicode_buffer(65536);length=ui.LoadStringW(module,identifier,buf,len(buf));result[identifier]=buf[:length]
        return result
    finally:assert kernel.FreeLibrary(module)

def real_installer(path):
    settings=build_client.config(ROOT/'config/client.example.toml');data=path.read_bytes();source=SolidInstaller.parse(data)
    original=source.stream[build_client.CLIENT_POSITION:build_client.CLIENT_POSITION+CLIENT_SIZE]
    pe=PE32(original);ids=sorted({(r.name-1)*16+i for r in pe.resources() if r.kind==6 for i in range(16)})
    with tempfile.TemporaryDirectory(prefix='rgg-real-client-') as tmp:
        root=Path(tmp);original_path=root/'original.exe';original_path.write_bytes(original)
        native_before=windows_strings(original_path,ids) if os.name=='nt' else None
        for destination in ['127.0.0.1','gg.example.org','abcdefghijklmnop']:
            settings['hostname']=destination;files,receipt=build_client.prepare(path,settings);client=files['gg.exe'];rebuilt=SolidInstaller.parse(files['gg-setup.exe'])
            assert source.stream[:build_client.CLIENT_POSITION]==rebuilt.stream[:build_client.CLIENT_POSITION]
            assert source.stream[build_client.CLIENT_POSITION+CLIENT_SIZE:]==rebuilt.stream[build_client.CLIENT_POSITION+CLIENT_SIZE:]
            assert rebuilt.stream[build_client.CLIENT_POSITION:build_client.CLIENT_POSITION+CLIENT_SIZE]==client
            assert files['gg-setup.exe'][:source.offset]==data[:source.offset] and client[:0x1000]==original[:0x1000]
            assert client[0x45f80:0x45f92]==bytes.fromhex('31c0c3909090')*3
            assert client[0x841bc:0x841cf]==b'wiadomosc\0dostepny\0' and client[0x841dc:0x841e2]==b'Online'
            assert client[0x22bc4:0x22bd1]==original[0x22bc4:0x22bd1] and client[0x33235:0x332bd]==original[0x33235:0x332bd]
            for name in [b'wiadomosc.wav\0',b'dostepny.wav\0']:assert name in source.stream[:4+source.header_size]
            allowed=bytearray(len(client))
            for change in receipt['patches']:allowed[change['offset']:change['offset']+change['length']]=b'1'*change['length']
            assert all(a==b or allowed[i] for i,(a,b) in enumerate(zip(original,client)))
            output=root/destination;build_client.publish(output,files)
            if native_before is not None:
                actual=windows_strings(output/'gg.exe',ids)
                assert actual=={i:endpoint_text(text,destination) for i,text in native_before.items()}
            with unittest.TestCase().assertRaises(Invalid):patch_client(client,destination)
        # Source damage, including DOS header bytes outside NSIS CRC, is refused by full SHA.
        damaged=root/'damaged.exe';raw=bytearray(data);raw[40]^=1;damaged.write_bytes(raw)
        with unittest.TestCase().assertRaises(Invalid):build_client.prepare(damaged,settings)
        target=root/'must-not-exist'
        p=subprocess.run([sys.executable,str(ROOT/'tools/build_client.py'),'--installer',str(damaged),'--config',str(ROOT/'config/client.example.toml'),'--output',str(target)],capture_output=True,timeout=15)
        assert p.returncode!=0 and not target.exists()
        assert sha(path.read_bytes())==sha(data)
    print('PASS: pinned real installer, three hosts, exact payload/stub preservation, updater/sounds, source unchanged'+('; '+str(len(ids))+' Windows LoadStringW resources without executing GG' if native_before is not None else ''),flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--installer',type=Path);args=parser.parse_args()
    result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(FormatTests))
    if not result.wasSuccessful():raise SystemExit(1)
    if args.installer:real_installer(args.installer)
