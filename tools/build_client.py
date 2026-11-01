# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Build a separate GG6 client/installer catalog without executing Windows code (Python >=3.11)."""
import argparse
import json
import lzma
import os
import re
from pathlib import Path
import stat
import tomllib
from urllib.parse import urlsplit
from client_format import Invalid, SolidInstaller, region, MAX_FILE
from client_patch import INSTALLER_SHA256, CLIENT_SHA256, CLIENT_SIZE, hostname, patch_client, sha

ROOT = Path(__file__).resolve().parents[1]
CLIENT_POSITION = 1432618  # Measured only for the exact source installer SHA-256.

def bounded_file(path, maximum):
    meta = path.lstat()
    if not stat.S_ISREG(meta.st_mode) or path.is_symlink() or meta.st_size > maximum:
        raise Invalid('input is not a bounded regular file')
    with path.open('rb') as stream:
        data = stream.read(maximum + 1)
    if len(data) > maximum:
        raise Invalid('input grew beyond its limit')
    return data

def config(path):
    settings = tomllib.loads(bounded_file(path, 16384).decode('utf-8'))
    if set(settings) != {'schema', 'hostname', 'source_url', 'license'} or type(settings['schema']) is not int or settings['schema'] != 1:
        raise Invalid('unsupported client configuration')
    hostname(settings['hostname'])
    source = settings['source_url']
    if not isinstance(source, str) or not source.startswith('https://') or not source.isascii() or len(source) > 512 or not re.fullmatch(r"[A-Za-z0-9\-._~:/?\[\]!$&'()*+,;=%]+", source):
        raise Invalid('source URL must be bounded HTTPS without credentials')
    url = urlsplit(source)
    if url.scheme != 'https' or not url.netloc or not url.hostname or url.fragment:
        raise Invalid('source URL must be HTTPS without a fragment')
    _ = url.port  # Reject a malformed port before creating output.
    license_text = settings['license']
    if not isinstance(license_text, str) or not license_text.strip() or len(license_text.encode()) > 500 or any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in license_text):
        raise Invalid('license notice must contain 1-500 UTF-8 bytes without controls')
    return settings

def prepare(installer_path, settings):
    source = bounded_file(installer_path, MAX_FILE)
    if sha(source) != INSTALLER_SHA256:
        raise Invalid('source installer is not the pinned original GG 6.1 installer')
    archive = SolidInstaller.parse(source)
    original = region(archive.stream, CLIENT_POSITION, CLIENT_SIZE)
    if sha(original) != CLIENT_SHA256:
        raise Invalid('pinned client member is missing')
    client, edits = patch_client(original, settings['hostname'])
    installer = archive.replace_member(CLIENT_POSITION, original, client)
    items = [
        ('gg6-client', 'client', 'gg.exe', 'GG 6.1 — plik klienta', client,
         'Zamknij GG. W katalogu jego instalacji zrób kopię pliku gg.exe i podmień go na ten plik. Zachowaj DLL i folder sounds. To nie jest instalator.'),
        ('gg6-installer', 'installer', 'gg-setup.exe', 'GG 6.1 — instalator', installer,
         'Instalator zawiera przygotowany gg.exe, biblioteki i dźwięki. Zachowuje oryginalny kreator oraz licencję. Przed próbą użyj oddzielnego środowiska testowego z osobnym profilem GG.'),
    ]
    artifacts = []
    for identifier, kind, filename, title, content, instructions in items:
        artifacts.append(dict(id=identifier, kind=kind, file=filename, title=title,
                              size=len(content), sha256=sha(content), source_url=settings['source_url'],
                              license=settings['license'], instructions=instructions + ' Adres serwera: ' + settings['hostname'] + '.'))
    manifest = 'schema = 1\n'
    for item in artifacts:
        manifest += '\n[[artifacts]]\n' + ''.join(key + ' = ' + json.dumps(value, ensure_ascii=False) + '\n' for key, value in item.items())
    if len(manifest.encode()) > 16384 or tomllib.loads(manifest)['artifacts'] != artifacts:
        raise Invalid('generated download manifest is invalid')
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    receipt = dict(format=1, builder_version=version, hostname=settings['hostname'],
                   source_installer_sha256=INSTALLER_SHA256, source_client_sha256=CLIENT_SHA256,
                   source_url=settings['source_url'], artifacts=[{k:v for k,v in item.items() if k in ('file','size','sha256')} for item in artifacts],
                   patches=[dict(offset=e.start, length=len(e.before), reason=e.reason) for e in sorted(edits,key=lambda e:e.start)],
                   installer_preserved='stub, script, original license and all payload bytes outside gg.exe',
                   executed=False, native_client_qualified=False)
    readme = ('Pakiet lokalny Retro Gadu (rGG)\nAdres klienta: ' + settings['hostname'] +
              '\n\nKlient i instalator nie zostały uruchomione przez generator. Przed próbą\n'
              'użyj oddzielnego środowiska/profilu testowego. Oryginalny instalator używa\n'
              'standardowych ustawień i profilu Gadu-Gadu; nie zapewnia izolacji od istniejącego GG.\n\n'
              'gg.exe: plik do podmiany w kompletnym katalogu instalacji; zachowaj DLL i sounds.\n'
              'gg-setup.exe: oryginalny kreator z wymienionym gg.exe.\n'
              'manifest.toml: katalog pobierania dla opcji downloads_manifest w rGG.\n'
              'receipt.json: hashe, zakresy poprawek i stan kwalifikacji.\n\n'
              'Pakiet poprawia zapis listy kontaktów na serwerze rGG, domyślne nazwy\n'
              'dźwięków i wyłącza dawny aktualizator. Zgodność całej aplikacji wymaga\n'
              'osobnej próby; innych serwerów nie kwalifikowano.\n\n'
              'Legacy klient używa HTTP huba na porcie 80. Profil 127.0.0.1 wymaga\n'
              'osiągalnego lokalnego huba na tym porcie; serwer wskazuje właściwy port GG.\n'
              'Generator nie zmienia portów, DNS, hosts, instalacji ani uruchomionych usług.\n\n' + settings['license'] + '\n')
    return {'gg.exe':client, 'gg-setup.exe':installer,
            'receipt.json':(json.dumps(receipt,ensure_ascii=False,indent=2)+'\n').encode(),
            'README.txt':readme.encode(), 'manifest.toml':manifest.encode()}, receipt

def write_new(path, content):
    with path.open('xb') as stream:
        stream.write(content)
        stream.flush()
        os.fsync(stream.fileno())

def sync_directory(path):
    if os.name != 'nt':
        fd = os.open(path, os.O_RDONLY)
        try: os.fsync(fd)
        finally: os.close(fd)

def publish(output, contents):
    output.mkdir(mode=0o700)  # Never reuse an existing directory, including an empty one.
    for name in ('gg.exe', 'gg-setup.exe', 'receipt.json', 'README.txt'):
        write_new(output / name, contents[name])
    for name, expected in contents.items():
        if name != 'manifest.toml' and bounded_file(output / name, MAX_FILE) != expected:
            raise Invalid('written artifact verification failed')
    pending = output / 'manifest.pending'
    write_new(pending, contents['manifest.toml'])
    os.replace(pending, output / 'manifest.toml')  # Only this final name publishes a complete catalog.
    sync_directory(output)
    sync_directory(output.parent)

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--installer',type=Path,required=True);p.add_argument('--config',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    args=p.parse_args()
    try:
        settings=config(args.config)
        if args.output.exists() or args.output.is_symlink():raise Invalid('output directory must not exist')
        contents,receipt=prepare(args.installer,settings);publish(args.output,contents)
    except (Invalid,OSError,ValueError,lzma.LZMAError) as error:
        print(json.dumps({'built':False,'error':str(error) if isinstance(error,Invalid) else 'file_or_configuration_error'}))
        return 2
    print(json.dumps({'built':True,'hostname':receipt['hostname'],'artifacts':receipt['artifacts'],'executed':False}))
    return 0

if __name__=='__main__':raise SystemExit(main())
