# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Real loopback GG/HTTP observations of image buffers, never production data."""
import argparse
import re
import tempfile
import time
from pathlib import Path

from gg6_sessions import Lab, until
from gg6_presence_contacts import barrier
from gg6_images import DATA, offer, text, control, recv, download
from http_metrics import metrics, endpoint
from http_discovery import request
from lifecycle import Server

NAME = b'PrivateImageFilenameCanary.png'
STATES = ['requested', 'collecting', 'ready', 'missing']


def observed(lab, states, reserved, buffered):
    saved = []
    def matches():
        sample = metrics(lab)
        if not sample['rgg_image_measurements_available']:
            return False
        actual = [sample['rgg_image_transfers{state="'+state+'"}'] for state in STATES]
        if (actual != states or sample['rgg_image_reserved_bytes'] != reserved
                or sample['rgg_image_buffered_bytes'] != buffered):
            return False
        assert buffered <= sample['rgg_image_buffer_capacity_bytes']
        if buffered == 0:
            assert sample['rgg_image_buffer_capacity_bytes'] == 0
        if reserved == 0:
            assert sample['rgg_image_oldest_transfer_age_seconds'] == 0
        saved.append(sample)
        return True
    until(matches)
    return saved[-1]


def clients(lab):
    a, b = lab.client(1), lab.client(2)
    barrier(a); barrier(b)
    return a, b


def begin(a, b, data=DATA):
    offer(a, data=data); text(b, data)
    assert control(b, 1, 4, data=data) == 2
    recv(a, 2, 4, b'', data)


def completed_and_restart(lab):
    lab.start(metrics=True)
    a, b = clients(lab)
    observed(lab, [0]*4, 0, 0)
    begin(a, b)
    observed(lab, [1, 0, 0, 0], len(DATA), 0)
    assert control(a, 2, 5, NAME+b'\0'+DATA[:100]) == 2
    before = observed(lab, [0, 1, 0, 0], len(DATA), len(NAME)+100)
    assert before['rgg_image_buffer_capacity_bytes'] == len(DATA)+len(NAME)
    assert NAME not in request(endpoint(lab), '/metrics')[2]
    for chunk in [DATA[i:i+1800] for i in range(100, len(DATA), 1800)]:
        assert control(a, 2, 6, chunk) == 2
    name, result = recv(b, 1, 5).split(b'\0', 1)
    assert name == NAME
    while len(result) < len(DATA):
        result += recv(b, 1, 6)
    assert result == DATA
    observed(lab, [0]*4, 0, 0)
    # An incomplete second retrieval is ephemeral; the durable offer survives restart.
    assert control(b, 1, 4) == 2; recv(a, 2, 4, b'')
    assert control(a, 2, 5, NAME+b'\0'+DATA[:100]) == 2
    observed(lab, [0, 1, 0, 0], len(DATA), len(NAME)+100)
    lab.stop(hard=True); lab.start(metrics=True)
    observed(lab, [0]*4, 0, 0)
    a, b = clients(lab)
    download(a, b)
    observed(lab, [0]*4, 0, 0)
    assert lab.sql('PRAGMA integrity_check') == [('ok',)]


def quota_and_disconnect(lab):
    lab.start(metrics=True)
    a, b = clients(lab)
    for n in range(4):
        data = DATA + bytes([n])
        begin(a, b, data)
        assert control(a, 2, 5, NAME+b'\0'+data[:100], data) == 2
    observed(lab, [0, 4, 0, 0], 4*(len(DATA)+1), 4*(len(NAME)+100))
    fifth = DATA+b'\x04'
    offer(a, data=fifth); text(b, fifth)
    assert control(b, 1, 4, data=fifth) == 4
    a.quiet(); b.quiet()  # Quota creates neither a sender request nor a missing notification.
    observed(lab, [0, 4, 0, 0], 4*(len(DATA)+1), 4*(len(NAME)+100))
    b.close()
    observed(lab, [0]*4, 0, 0)
    b = lab.client(2); barrier(b)
    begin(a, b)
    assert control(a, 2, 5, NAME+b'\0'+DATA[:100]) == 2
    observed(lab, [0, 1, 0, 0], len(DATA), len(NAME)+100)
    a.close(); recv(b, 1, 5, b'')
    observed(lab, [0]*4, 0, 0)


def deadline(lab):
    lab.start(metrics=True)
    a, b = clients(lab)
    started = time.monotonic()
    begin(a, b)
    assert control(a, 2, 5, NAME+b'\0'+DATA[:100]) == 2
    initial = observed(lab, [0, 1, 0, 0], len(DATA), len(NAME)+100)
    ages = [initial['rgg_image_oldest_transfer_age_seconds']]
    # Continue both observation and real GG activity. Neither may refresh the fixed deadline.
    while time.monotonic() - started < 28:
        sample = observed(lab, [0, 1, 0, 0], len(DATA), len(NAME)+100)
        ages.append(sample['rgg_image_oldest_transfer_age_seconds'])
        a.ping(); b.ping()
        time.sleep(.2)
    assert ages == sorted(ages) and ages[-1] > 27
    assert control(a, 2, 6, DATA[100:200]) == 2
    observed(lab, [0, 1, 0, 0], len(DATA), len(NAME)+200)
    recv(b, 1, 5, b'')
    elapsed = time.monotonic() - started
    assert 29.5 <= elapsed <= 32, elapsed
    observed(lab, [0]*4, 0, 0)
    a.ping(); b.ping()
    print(f'  fixed deadline observed after {elapsed:.3f}s with {len(ages)} snapshots', flush=True)


def no_gg(lab):
    config = lab.root/'http-only.toml'
    config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\nmetrics_bind='127.0.0.1:0'\n", encoding='utf-8')
    lab.server = Server(lab.binary, config, lab.root/'server.log')
    lab.server.ready()
    until(lambda: re.search(r'metrics_endpoint=127\.0\.0\.1:\d+', lab.server.log.read_text()))
    sample = metrics(lab)
    assert sample['rgg_image_measurements_available'] == 0
    assert not any(key.startswith('rgg_image_') and key != 'rgg_image_measurements_available' for key in sample)
    assert 'gg_endpoint=' not in lab.server.log.read_text()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    args = parser.parse_args()
    for name, test in [
        ('reservation, actual buffer bytes, verified image, private filename and crash restart', completed_and_restart),
        ('quota rejection and both disconnect paths release buffers', quota_and_disconnect),
        ('scraping, pings and late continuation preserve the 30-second deadline', deadline),
        ('HTTP-only service declares image observations unavailable', no_gg),
    ]:
        with tempfile.TemporaryDirectory(prefix='rgg-image-metrics-') as root:
            lab = Lab(args.server.resolve(), args.admin.resolve(), Path(root))
            try:
                test(lab)
                print('PASS:', name, flush=True)
            finally:
                lab.stop()


if __name__ == '__main__':
    main()
