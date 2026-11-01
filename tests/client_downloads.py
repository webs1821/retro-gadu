# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Opt-in loopback test of operator-supplied built artifacts. Never executes EXE files."""
import argparse
import hashlib
import json
import tempfile
from pathlib import Path
from gg6_sessions import Lab
from http_discovery import request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    parser.add_argument('--manifest', type=Path, required=True)
    args = parser.parse_args()
    manifest = args.manifest.resolve()
    receipt = json.loads(manifest.with_name('receipt.json').read_text(encoding='utf-8'))
    expected = {item['file']: item for item in receipt['artifacts']}
    assert set(expected) == {'gg.exe', 'gg-setup.exe'}
    with tempfile.TemporaryDirectory(prefix='rgg-client-downloads-') as tmp:
        lab = Lab(args.server.resolve(), args.admin.resolve(), Path(tmp))
        try:
            lab.start(downloads=str(manifest), statistics=True)
            code, _, body = request(lab, '/api/downloads')
            assert code == 200 and str(manifest.parent).encode() not in body
            public = json.loads(body)
            assert len(public['artifacts']) == 2
            for item in public['artifacts']:
                filename = {'gg6-client': 'gg.exe', 'gg6-installer': 'gg-setup.exe'}[item['id']]
                entry = expected[filename]
                assert item['sha256'] == entry['sha256'] and item['size'] == entry['size']
                path = '/downloads/' + item['id']
                code, headers, content = request(lab, path)
                assert code == 200 and len(content) == entry['size']
                assert hashlib.sha256(content).hexdigest() == entry['sha256']
                assert content == manifest.with_name(filename).read_bytes()
                assert headers['content-disposition'] == 'attachment; filename="' + filename + '"'
                assert headers['content-type'] == 'application/octet-stream'
                code, headers, content = request(lab, path, method='HEAD')
                assert code == 200 and not content and int(headers['content-length']) == entry['size']
            code, _, page = request(lab, '/downloads')
            assert code == 200 and 'To nie jest instalator'.encode() in page
            assert receipt['hostname'].encode() in page and b'href="/downloads"' in request(lab, '/')[2]
            lab.client(1).ping()
            print('PASS: built client and installer served byte-exact over loopback, hashes, HEAD, instructions and concurrent GG login; no EXE execution', flush=True)
        finally:
            lab.stop()


if __name__ == '__main__':
    main()
