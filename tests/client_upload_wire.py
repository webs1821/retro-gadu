# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Replay the independent CPU tool's captured packets through actual local TCP."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile

from gg6_presence_contacts import get_archive, put_archive, take
from gg6_sessions import Lab


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    parser.add_argument('--transcript', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists()
    with args.transcript.open('rb') as source:
        raw = source.read(1024 * 1024 + 1)
    assert len(raw) <= 1024 * 1024
    fixture = json.loads(raw)
    assert fixture['format'] == 1 and fixture['passed'] and fixture['null_unchanged']
    cases = fixture['transcripts']
    assert [case['input_length'] for case in cases] == [0, 2047, 2048, 4094, 4095, 49128, 50000, 50001]
    completed = []
    with tempfile.TemporaryDirectory(prefix='rgg-client-upload-wire-') as temporary:
        lab = Lab(args.server.resolve(), args.admin.resolve(), Path(temporary))
        try:
            lab.start()
            client = lab.client(1)
            previous = b'previous committed archive'
            put_archive(client, previous)
            for case in cases:
                frames = [bytes.fromhex(value) for value in case['frames_hex']]
                assert 1 <= len(frames) <= 26
                assert all(1 <= len(frame) <= 2048 for frame in frames)
                assert [frame[0] for frame in frames] == [0] + [1] * (len(frames) - 1)
                payload = b''.join(frame[1:] for frame in frames)
                assert len(payload) == min(case['input_length'], 50000)
                assert hashlib.sha256(payload).hexdigest() == case['payload_sha256']
                # Observe the old version while the actual captured prefix is incomplete.
                for frame in frames[:-1]:
                    assert len(frame) == 2048
                    client.send(0x16, frame)
                if len(frames) > 1:
                    client.quiet(.03)
                    assert get_archive(client) == previous
                client.send(0x16, frames[-1])
                for index in range(len(frames)):
                    assert take(client, 0x10) == bytes([0 if index == 0 else 2])
                assert get_archive(client) == payload
                previous = payload
                completed.append({'input_length': case['input_length'], 'frames': len(frames),
                                  'payload_bytes': len(payload), 'exact_roundtrip': True})
            lab.stop()
            lab.start()
            assert get_archive(lab.client(1)) == previous
            lab.stop()
            assert lab.sql('PRAGMA integrity_check') == [('ok',)]
            assert lab.sql('PRAGMA foreign_key_check') == []
            assert lab.sql('SELECT count(*) FROM contact_staging') == [(0,)]
            assert lab.sql('SELECT version FROM contact_archives WHERE owner=1') == [(9,)]
        finally:
            lab.stop()
    report = {'format': 1, 'passed': True, 'cases': completed,
              'cpu_transcript_sha256': hashlib.sha256(raw).hexdigest(),
              'patched_client_sha256': fixture['patched_sha256'],
              'server_sha256': hashlib.sha256(args.server.read_bytes()).hexdigest(),
              'driver_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'restart_verified': True, 'native_client_qualified': False}
    with args.report.open('x', encoding='utf-8', newline='\n') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    print(json.dumps({'passed': True, 'captured_sequences': len(completed), 'restart_verified': True}))


if __name__ == '__main__':
    main()
