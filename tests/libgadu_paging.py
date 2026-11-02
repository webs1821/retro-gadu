# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""41 opt-in profiles published and paged through the external libgadu 1.7.0 API."""
import argparse
import hashlib
import json
import locale
import os
import platform
import socket
import tempfile
import time
from pathlib import Path
from gg6_sessions import Lab, PASSWORD, until
from libgadu_client import Probe, WireRelay, sql


def exercise(args, root, evidence):
    lab = Lab(args.server, args.admin, root)
    relay = WireRelay(max_connections=48)
    clients = []

    def connect(user):
        client = Probe(args.probe, relay.port, user, root); clients.append(client)
        client.take('login', uin=user, protocol_version=32)
        client.command('empty'); client.ping()
        return client

    def pages(client, base):
        cursor, found, results = 0, [], []
        for index in range(3):
            sequence = base + index
            client.command(f'page {sequence} {cursor}')
            result = client.take('directory', seq=sequence, type=5)
            assert result['count'] == [20, 20, 1][index]
            users = [int(bytes.fromhex(row['FmNumber'])) for row in result['rows']]
            assert users == list(range(3 + index * 20, min(23 + index * 20, 44)))
            for row in result['rows']:
                assert row['birthyear'] == b'1987'.hex() and row['city'] == 'Łódź'.encode('cp1250').hex()
                assert row['FmStatus'] == b'1'.hex()
            cursor = result['next']
            assert bool(cursor) == (index < 2)
            found += users; results.append(result)
        assert found == list(range(3, 44)) and len(set(found)) == 41
        return results

    try:
        code = lab.run('invite', '--uses', '40')['codes'][0]
        for user in range(4, 44):
            lab.run('account-create', payload={'requested_uin': user, 'invite': code,
                    'password': PASSWORD.decode(), 'email': 'paging-synthetic@example.invalid'})
        lab.start(); relay.target = lab.address
        for user in range(3, 44):
            owner = connect(user)
            owner.command('profile 101 ' + ('Żółwik' + str(user)).encode('cp1250').hex())
            owner.take('directory', seq=101, type=1, count=0, next=0)
            owner.close(); assert owner.process.returncode == 0
            time.sleep(.1)  # Respect admission and let the bounded relay close this pair.
        assert sql(lab, 'SELECT count(*) FROM public_profiles') == [(41,)]
        until(lambda: sql(lab, 'SELECT count(*) FROM account_sessions') == [(0,)])
        viewer = connect(2); first = pages(viewer, 500)
        viewer.ping(); lab.stop(); viewer.take('closed'); assert viewer.process.wait(timeout=3) == 0
        lab.start(); relay.target = lab.address
        viewer = connect(2); second = pages(viewer, 600)
        assert [page['next'] for page in first] == [page['next'] for page in second]
        viewer.ping(); lab.stop(); viewer.take('closed'); assert viewer.process.wait(timeout=3) == 0
        assert sql(lab, 'PRAGMA integrity_check') == [('ok',)] and sql(lab, 'PRAGMA foreign_key_check') == []
        assert len(relay.records) == 43 and all(item['image_size_kib'] == 255 for item in relay.records)
        evidence.update(passed=True,profiles=41,page_sizes=[20,20,1],before_restart=first,after_restart=second,
                        no_duplicates=True,no_empty_rows=True,all_profiles_via_external_api=True,
                        library_sources_modified=False,native_gg_qualified=False)
    finally:
        evidence.update(login_captures=relay.records,client_events=[client.trace for client in clients])
        for client in clients: client.close()
        lab.stop(); relay.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ['server','admin','probe','report']: parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--outer-network-namespace',required=True)
    args=parser.parse_args()
    namespace=os.readlink('/proc/self/ns/net')
    assert namespace!=args.outer_network_namespace and args.outer_network_namespace.startswith('net:[')
    assert {name for _,name in socket.if_nameindex()}=={'lo'} and not args.report.exists()
    evidence={'passed':False,'network_namespace':namespace,'outer_network_namespace':args.outer_network_namespace,
              'loopback_only':True,'platform':platform.platform(),'locale_encoding':locale.getpreferredencoding(False),
              'binary_sha256':{name:hashlib.sha256(getattr(args,name).read_bytes()).hexdigest() for name in ['server','admin','probe']},
              'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    try:
        with tempfile.TemporaryDirectory(prefix='rgg-libgadu-paging-') as temporary:
            exercise(args,Path(temporary),evidence)
    except BaseException as error:
        evidence['error']=repr(error);raise
    finally:
        args.report.write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
    print('PASS: libgadu publishes 41 profiles, reads pages 20/20/1 and repeats after restart',flush=True)


if __name__=='__main__': main()
