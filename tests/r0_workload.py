# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Synthetic GG6 load on explicit loopback ports; no native-client qualification."""
import argparse
import asyncio
import http.client
import ipaddress
import json
from pathlib import Path
import re
import struct
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from gg32_vectors import oracle

PASSWORD = b'LocalTest9!'

def packet(kind, body=b''):
    return struct.pack('<II', kind, len(body)) + body

async def frame(reader):
    kind, size = struct.unpack('<II', await reader.readexactly(8))
    assert size <= 65536, 'oversized server frame'
    return kind, await reader.readexactly(size)

def quantile(values, fraction):
    import math
    assert values
    return round(sorted(values)[max(0, math.ceil(len(values)*fraction)-1)]*1000, 3)

def metric_snapshot(host, port):
    connection = http.client.HTTPConnection(host, port, timeout=2)
    try:
        connection.request('GET', '/metrics')
        response = connection.getresponse()
        assert response.status == 200, 'metrics unavailable'
        body = response.read(8193)
        assert len(body) <= 8192
        result = {}
        for line in body.decode().splitlines():
            if line and not line.startswith('#'):
                name, value = line.split(); result[name] = float(value)
        return result
    finally:
        connection.close()

class Wire:
    def __init__(self, user, reader, writer, expected, errors):
        self.user, self.reader, self.writer = user, reader, writer
        self.expected, self.errors = expected, errors
        self.acks, self.received = {}, []
        self.closing = False
        self.task = asyncio.create_task(self.read())

    @classmethod
    async def connect(cls, host, port, user, expected, errors):
        reader, writer = await asyncio.wait_for(asyncio.open_connection(host, port), 3)
        try:
            kind, body = await asyncio.wait_for(frame(reader), 3)
            assert kind == 1 and len(body) == 4
            seed, = struct.unpack('<I', body)
            login = struct.pack('<IIII', user, oracle(PASSWORD, seed), 2, 0x20) + bytes(13) + bytes([255, 0xbe])
            writer.write(packet(0x15, login)); await writer.drain()
            assert await asyncio.wait_for(frame(reader), 3) == (3, b''), 'synthetic login refused'
            writer.write(packet(0x12)); await writer.drain()
            return cls(user, reader, writer, expected, errors)
        except BaseException:
            writer.close()
            raise

    async def read(self):
        try:
            while True:
                kind, body = await frame(self.reader)
                if kind == 5:
                    status, recipient, sequence = struct.unpack('<III', body)
                    assert sequence in self.acks, 'unexpected/duplicate ACK'
                    future = self.acks.pop(sequence)
                    assert status == 3 and recipient == self.expected[sequence]['recipient'], 'message was not admitted'
                    future.set_result(time.monotonic())
                elif kind == 10:
                    sender, sequence, accepted_at, flags = struct.unpack('<IIII', body[:16])
                    item = self.expected[sequence]
                    assert item['sender'] == sender and item['recipient'] == self.user
                    assert body[16:] == item['text'] + b'\0', 'payload mismatch'
                    assert bool(flags & 1) == item['offline'], 'queued flag mismatch'
                    assert not item.get('received'), 'duplicate message'
                    assert abs(accepted_at-time.time()) < 1500
                    item['received'] = time.monotonic()
                    self.received.append(sequence)
                elif kind == 7:
                    assert not body
                else:
                    raise AssertionError('unexpected frame type ' + str(kind))
        except asyncio.CancelledError:
            raise
        except Exception as error:
            if not self.closing:
                self.errors.append(type(error).__name__ + ': ' + str(error))
        finally:
            for future in self.acks.values():
                if not future.done(): future.cancel()

    async def send(self, sequence, deadline):
        item = self.expected[sequence]
        future = asyncio.get_running_loop().create_future()
        self.acks[sequence] = future
        item['sent'] = time.monotonic()
        item['schedule_lag'] = max(0, item['sent'] - deadline)
        self.writer.write(packet(0xb, struct.pack('<III', item['recipient'], sequence, 8) + item['text'] + b'\0'))
        await self.writer.drain()
        item['ack'] = await asyncio.wait_for(future, 1)

    async def close(self):
        if self.closing: return
        self.closing = True
        self.writer.close()
        try: await asyncio.wait_for(self.writer.wait_closed(), 2)
        except (OSError, asyncio.TimeoutError): pass
        self.task.cancel()
        await asyncio.gather(self.task, return_exceptions=True)

async def run(args):
    assert ipaddress.ip_address(args.host).is_loopback
    assert 2 <= args.sessions <= 100 and 1 <= args.offline <= 20
    assert 5 <= args.seconds <= 1200 and 1 <= args.rate <= 10
    assert 2*args.offline/args.rate > 1, 'FIFO checks require a completed ACK before the next send to that offline recipient'
    expected, errors, clients, offline_clients, sends = {}, [], [], [], []
    samples = []
    sampling = None
    loop = asyncio.get_running_loop()
    async def snapshot():
        return await loop.run_in_executor(None, metric_snapshot, args.host, args.metrics_port)
    async def wait_metric(name, value):
        deadline = time.monotonic()+8
        while True:
            if (await snapshot())[name] == value: return
            assert time.monotonic() < deadline, 'metric did not converge: '+name
            await asyncio.sleep(.1)
    async def observe():
        while True:
            sample = await snapshot()
            assert sample['rgg_sessions{phase="ready"}'] == args.sessions, 'session count changed during load'
            samples.append({'at':time.monotonic(), 'pending':sample['rgg_delivery_records{state="pending"}'], 'queue_bytes':sample['rgg_queued_payload_bytes']})
            await asyncio.sleep(1)
    def task_done(task):
        if task.cancelled(): errors.append('send cancelled')
        elif task.exception() is not None: errors.append(type(task.exception()).__name__ + ': ' + str(task.exception()))
    try:
        initial = await snapshot()
        assert initial['rgg_accounts{state="active"}'] == args.sessions + args.offline
        assert initial['rgg_queued_payload_bytes'] == 0 and initial['rgg_sessions{phase="ready"}'] == 0
        for user in range(1, args.sessions+1):
            clients.append(await Wire.connect(args.host, args.gg_port, user, expected, errors))
        await wait_metric('rgg_sessions{phase="ready"}', args.sessions)
        await asyncio.sleep(1)
        start = time.monotonic()
        print('LOAD_EVENT='+json.dumps({'phase':'measure_start','at':start,'sessions':args.sessions}),flush=True)
        sampling = asyncio.create_task(observe())
        total = args.rate * args.seconds
        for index in range(total):
            deadline = start + index/args.rate
            await asyncio.sleep(max(0, deadline-time.monotonic()))
            assert not errors, errors[:3]
            if sampling.done(): await sampling
            source = clients[index % args.sessions]
            offline = bool(index % 2)
            recipient = args.sessions + 1 + (index//2) % args.offline if offline else source.user % args.sessions + 1
            sequence = index+1
            text = ('rgg-load-%010d:' % sequence).encode().ljust(128,b'x')
            expected[sequence] = dict(sender=source.user,recipient=recipient,text=text,offline=offline)
            task = asyncio.create_task(source.send(sequence, deadline)); task.add_done_callback(task_done); sends.append(task)
            if index and index % (args.rate*30) == 0:
                print('LOAD_EVENT='+json.dumps({'phase':'progress','scheduled':index}),flush=True)
        await asyncio.gather(*sends)
        await asyncio.sleep(max(0, start+args.seconds-time.monotonic()))
        end = time.monotonic()
        print('LOAD_EVENT='+json.dumps({'phase':'measure_end','at':end}),flush=True)
        if sampling.done(): await sampling
        sampling.cancel(); await asyncio.gather(sampling, return_exceptions=True); sampling = None
        assert (await snapshot())['rgg_sessions{phase="ready"}'] == args.sessions
        deadline = time.monotonic()+5
        while any(not item.get('received') for item in expected.values() if not item['offline']):
            assert time.monotonic()<deadline and not errors, errors[:3]
            await asyncio.sleep(.01)
        assert not errors,errors[:3]
        offline_count = sum(item['offline'] for item in expected.values())
        await wait_metric('rgg_delivery_records{state="pending"}', offline_count)
        await asyncio.gather(*(client.close() for client in clients))
        await wait_metric('rgg_sessions{phase="ready"}', 0)
        print('LOAD_EVENT='+json.dumps({'phase':'offline_replay','messages':offline_count}),flush=True)
        for user in range(args.sessions+1,args.sessions+args.offline+1):
            offline_clients.append(await Wire.connect(args.host,args.gg_port,user,expected,errors))
        deadline = time.monotonic()+30
        while any(not item.get('received') for item in expected.values()):
            assert time.monotonic()<deadline and not errors,errors[:3]
            await asyncio.sleep(.02)
        for client in offline_clients:
            wanted = [seq for seq,item in expected.items() if item['recipient']==client.user]
            assert client.received == wanted, 'offline FIFO mismatch'
        await wait_metric('rgg_queued_payload_bytes',0)
        await wait_metric('rgg_delivery_records{state="written_unconfirmed"}',total)
        assert not errors, errors[:3]
        acks = [item['ack']-item['sent'] for item in expected.values()]
        deliveries = [item['received']-item['sent'] for item in expected.values() if not item['offline']]
        lag = [item['schedule_lag'] for item in expected.values()]
        report = dict(format=1,passed=True,sessions=args.sessions,offline_accounts=args.offline,rate=args.rate,
                      duration_seconds=round(end-start,3),scheduled=total,accepted=len(acks),
                      online_verified=total-offline_count,offline_verified=offline_count,all_payloads_exact=True,offline_fifo=True,
                      ack_p50_ms=quantile(acks,.5),ack_p95_ms=quantile(acks,.95),ack_max_ms=quantile(acks,1),
                      online_p95_ms=quantile(deliveries,.95),online_max_ms=quantile(deliveries,1),
                      schedule_lag_max_ms=quantile(lag,1),achieved_rate=round(total/(end-start),4),
                      measure_start=start,measure_end=end,metrics_samples=samples,errors=[],native_client_qualified=False)
        print('LOAD_REPORT='+json.dumps(report),flush=True)
        return report
    finally:
        if sampling:
            sampling.cancel();await asyncio.gather(sampling,return_exceptions=True)
        for task in sends:
            if not task.done():task.cancel()
        await asyncio.gather(*sends,return_exceptions=True)
        await asyncio.gather(*(client.close() for client in clients+offline_clients),return_exceptions=True)

def prepare(admin, data, count):
    assert 3 <= count <= 120 and not data.exists(), 'new synthetic data directory required'
    def command(name,*args,payload=None):
        process=subprocess.run([str(admin),name,'--data-dir',str(data),*args],input=json.dumps(payload) if payload else None,
                               text=True,capture_output=True,check=True,timeout=15)
        return json.loads(process.stdout)
    command('init')
    code=command('invite','--uses',str(count))['codes'][0]
    for user in range(1,count+1):
        command('account-create',payload=dict(requested_uin=user,invite=code,password=PASSWORD.decode(),email='r0-synthetic@example.invalid'))
    assert command('stats')['registered']==count

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest='command',required=True)
    p=sub.add_parser('prepare');p.add_argument('--admin',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--count',type=int,required=True)
    p=sub.add_parser('run');p.add_argument('--host',default='127.0.0.1');p.add_argument('--gg-port',type=int,default=8074);p.add_argument('--metrics-port',type=int,default=9090)
    p.add_argument('--sessions',type=int,default=100);p.add_argument('--offline',type=int,default=20);p.add_argument('--seconds',type=int,default=180);p.add_argument('--rate',type=int,default=10)
    args=parser.parse_args()
    if args.command=='prepare':prepare(args.admin,args.data,args.count)
    else:asyncio.run(run(args))
if __name__=='__main__':main()
