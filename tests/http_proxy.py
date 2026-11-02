# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Disposable verified TLS proxy, explicit local trust and synthetic GG accounts only."""
import argparse
import http.client
import http.server
import json
import re
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
from pathlib import Path
from urllib.parse import urlencode

from gg6_sessions import Lab, PASSWORD, until
from http_discovery import request
from http_metrics import endpoint, metrics
from http_downloads import fixture, CONTENT
from http_patcher import ASSETS, ROOT

PROXY_IP = '127.0.0.2'
CLIENT_A = '127.0.0.3'
CLIENT_B = '127.0.0.4'
NEW_PASSWORD = b'ProxyTest9!'


class LocalHTTPS(http.client.HTTPSConnection):
    def connect(self):
        raw = socket.create_connection(('127.0.0.1', self.port), self.timeout, self.source_address)
        try:
            self.sock = self._context.wrap_socket(raw, server_hostname=self.host)
        except BaseException:
            raw.close()
            raise


class TLSProxy:
    """Single joined worker, bounded requests, explicit backend source; never public."""
    def __init__(self, root):
        key, cert = root/'test-key.pem', root/'test-cert.pem'
        subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes',
                        '-keyout', str(key), '-out', str(cert), '-days', '1',
                        '-subj', '/CN=rgg.test', '-addext', 'subjectAltName=DNS:rgg.test'],
                       check=True, capture_output=True, timeout=20)
        server_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        server_context.minimum_version = ssl.TLSVersion.TLSv1_2
        server_context.load_cert_chain(cert, key)
        self.context = ssl.create_default_context(cafile=str(cert))
        self.context.minimum_version = ssl.TLSVersion.TLSv1_2
        self.versions = set()
        self.backend = None
        self.worker = None
        proxy = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'
            def log_message(self, *args):
                pass
            def setup(self):
                self.request.settimeout(3)
                self.request.do_handshake()
                proxy.versions.add(self.request.version())
                super().setup()
            def do_GET(self):
                self.forward()
            def do_POST(self):
                self.forward()
            def forward(self):
                self.close_connection = True
                lengths = self.headers.get_all('Content-Length') or []
                if self.headers.get('Transfer-Encoding') or len(lengths) > 1:
                    self.send_error(400); return
                try:
                    size = int(lengths[0]) if lengths else 0
                    assert 0 <= size <= 4096
                except (ValueError, AssertionError):
                    self.send_error(413); return
                body = self.rfile.read(size)
                assert len(body) == size
                connection = http.client.HTTPConnection(proxy.backend, timeout=4, source_address=(PROXY_IP, 0))
                try:
                    connection.putrequest(self.command, self.path, skip_host=True, skip_accept_encoding=True)
                    # Incoming identity and transport headers are replaced, never appended.
                    for name, value in self.headers.items():
                        if name.lower() not in {'x-real-ip', 'x-forwarded-proto', 'forwarded',
                                                'x-forwarded-for', 'x-forwarded-host', 'connection',
                                                'content-length', 'transfer-encoding'}:
                            connection.putheader(name, value)
                    connection.putheader('X-Real-IP', self.client_address[0])
                    connection.putheader('X-Forwarded-Proto', 'https')
                    connection.putheader('Connection', 'close')
                    connection.putheader('Content-Length', str(len(body)))
                    connection.endheaders(body)
                    response = connection.getresponse()
                    payload = response.read(1024*1024+1)
                    assert len(payload) <= 1024*1024
                    self.send_response(response.status)
                    for name, value in response.getheaders():
                        if name.lower() not in {'connection', 'transfer-encoding', 'content-length'}:
                            self.send_header(name, value)
                    self.send_header('Content-Length', str(len(payload)))
                    self.send_header('Connection', 'close')
                    self.end_headers()
                    self.wfile.write(payload)
                finally:
                    connection.close()

        class Server(http.server.HTTPServer):
            def handle_error(self, request, address):
                # Preserve a failure flag without logging any form body or credentials.
                if isinstance(sys.exc_info()[1], ssl.SSLError):
                    self.tls_rejections += 1
                else:
                    self.failed = True

        self.server = Server(('127.0.0.1', 0), Handler)
        self.server.failed = False
        self.server.tls_rejections = 0
        self.server.socket = server_context.wrap_socket(self.server.socket, server_side=True, do_handshake_on_connect=False)
        self.port = self.server.server_address[1]
        self.origin = f'https://rgg.test:{self.port}'
        self.authority = f'rgg.test:{self.port}'

    def start(self, lab):
        self.backend = lab.server.address
        self.worker = threading.Thread(target=self.server.serve_forever, kwargs={'poll_interval': .05})
        self.worker.start()

    def config(self):
        return f"\n[web_proxy]\norigin='{self.origin}'\ntrusted_proxy='{PROXY_IP}'\n"

    def call(self, path, method='GET', body=None, headers=None, source=CLIENT_A):
        connection = LocalHTTPS('rgg.test', self.port, timeout=5, context=self.context, source_address=(source, 0))
        try:
            connection.request(method, path, body=body, headers=headers or {})
            reply = connection.getresponse()
            return reply.status, dict(reply.getheaders()), reply.read()
        finally:
            connection.close()

    def close(self):
        if self.worker:
            self.server.shutdown()
            self.worker.join(timeout=4)
            assert not self.worker.is_alive()
        self.server.server_close()
        assert not self.server.failed, 'local proxy handler failed'


def form(proxy, path='/register', source=CLIENT_A, headers=None):
    status, response, body = proxy.call(path, source=source, headers=headers)
    assert status == 200, (status, body)
    cookie = response['set-cookie']
    assert '; Secure' in cookie and '; HttpOnly' in cookie and 'SameSite=Strict' in cookie
    assert 'Domain=' not in cookie and f'Path={path};' in cookie
    token = re.search(rb'name="csrf" value="([0-9a-f]{64})"', body).group(1).decode()
    return token, cookie.split(';', 1)[0]


def submit(proxy, values, cookie, path='/register', source=CLIENT_A, headers=None):
    base = {'Origin': proxy.origin, 'Content-Type': 'application/x-www-form-urlencoded', 'Cookie': cookie}
    base.update(headers or {})
    return proxy.call(path, 'POST', urlencode(values), base, source)


def fields(csrf, invite):
    return dict(csrf=csrf, requested_uin='', invitation=invite,
                password=PASSWORD.decode(), email='synthetic@example.invalid')


def forms_and_sessions(lab, proxy, invite):
    # A valid hostname and a trusted test certificate are both necessary.
    for context, hostname in [(proxy.context, 'attacker.test'), (ssl.create_default_context(), 'rgg.test')]:
        with socket.create_connection(('127.0.0.1', proxy.port), timeout=3) as raw:
            try:
                with context.wrap_socket(raw, server_hostname=hostname):
                    raise AssertionError('invalid test certificate trust unexpectedly succeeded')
            except ssl.SSLCertVerificationError:
                pass
    until(lambda: proxy.server.tls_rejections == 2)
    spoof = {'X-Real-IP': CLIENT_B, 'X-Forwarded-For': CLIENT_B, 'X-Forwarded-Proto': 'http', 'Forwarded': 'for='+CLIENT_B}
    token, cookie = form(proxy, headers=spoof)
    data = fields(token, invite)
    assert submit(proxy, data, cookie, source=CLIENT_B)[0] == 403
    status, headers, body = submit(proxy, data, cookie, headers=spoof)
    assert status == 201, (status, body)
    assert 'Max-Age=0' in headers['set-cookie'] and '; Secure' in headers['set-cookie']
    uin = int(re.search(rb'data-uin="(\d+)"', body).group(1))
    assert uin == 112233
    assert submit(proxy, data, cookie)[0] == 403
    created = lab.client(uin); created.ping()
    old = lab.client(1); old.ping()
    token, cookie = form(proxy, '/password')
    data = dict(csrf=token, uin='1', current_password=PASSWORD.decode(),
                new_password=NEW_PASSWORD.decode(), confirm_password=NEW_PASSWORD.decode())
    assert submit(proxy, data, cookie, '/password', source=CLIENT_B)[0] == 403
    status, headers, _ = submit(proxy, data, cookie, '/password')
    assert status == 200 and '; Secure' in headers['set-cookie'] and 'Max-Age=0' in headers['set-cookie']
    old.closed()
    assert submit(proxy, data, cookie, '/password')[0] == 403
    invalid = lab.client(); assert invalid.login(1) == (9, b''); invalid.closed()
    updated = lab.client(); assert updated.login(1, password=NEW_PASSWORD) == (3, b''); updated.ping()
    assert json.loads(proxy.call('/api/stats')[2])['registered'] == 4
    status, headers, body = proxy.call('/downloads/client')
    assert status == 200 and body == CONTENT and 'attachment' in headers['content-disposition']
    assert proxy.call('/downloads')[0] == 200 and proxy.call('/api/downloads')[0] == 200
    status, headers, _ = proxy.call('/patcher')
    assert status == 200 and "script-src 'self'" in headers['content-security-policy']
    for path, filename in ASSETS.items():
        assert proxy.call(path)[2] == (ROOT/'crates/rgg-http/src'/filename).read_bytes()
    assert proxy.call('/metrics')[0] == 404
    assert metrics(lab)['rgg_database_schema_version'] == 13
    assert request(endpoint(lab), '/metrics', headers={'Host': proxy.authority})[0] == 403
    assert request(lab, '/health/ready')[0] == 200
    assert request(lab, '/appsvc/appmsg3.asp', headers={'Host': 'legacy.invalid'})[2].startswith(b'0 0 127.0.0.1:')
    assert lab.sql('PRAGMA integrity_check') == [('ok',)]
    assert NEW_PASSWORD not in lab.server.log.read_bytes()


def direct(lab, pairs, source=PROXY_IP):
    connection = http.client.HTTPConnection(lab.server.address, timeout=3, source_address=(source, 0))
    try:
        connection.putrequest('GET', '/register', skip_host=True, skip_accept_encoding=True)
        for name, value in pairs:
            connection.putheader(name, value)
        connection.putheader('Connection', 'close')
        connection.endheaders()
        response = connection.getresponse()
        return response.status, response.read()
    finally:
        connection.close()


def guards_and_budget(lab, proxy, _invite):
    valid = [('Host', proxy.authority), ('Origin', proxy.origin), ('X-Real-IP', CLIENT_A),
             ('X-Forwarded-Proto', 'https'), ('Sec-Fetch-Site', 'same-origin')]
    assert direct(lab, valid, source=CLIENT_A)[0] == 403
    assert request(lab, '/register', headers=dict(valid))[0] == 403
    for i in range(len(valid)):
        assert direct(lab, valid+[valid[i]])[0] in (400, 403)
    for name, value in [('Host', 'attacker.test'), ('Origin', 'https://attacker.test'),
                        ('X-Real-IP', '127.0.0.3, 127.0.0.4'), ('X-Real-IP', '127.0.0.3:80'),
                        ('X-Forwarded-Proto', 'http'), ('X-Forwarded-Proto', 'https,http'),
                        ('Sec-Fetch-Site', 'cross-site')]:
        assert direct(lab, [(k, value if k == name else v) for k, v in valid])[0] == 403
    for name in ['X-Real-IP', 'X-Forwarded-Proto']:
        assert direct(lab, [(k, v) for k, v in valid if k != name])[0] == 403
    for path in ['/', '/register', '/password', '/api/stats', '/assets/registration.css']:
        assert proxy.call(path, headers={'Host': 'attacker.test'})[0] == 403
        assert proxy.call(path, headers={'Origin': 'https://attacker.test'})[0] == 403
    # The mapped IPv6 spelling and IPv4 spelling refer to the same quota identity.
    ip = '127.0.0.5'
    for n in range(60):
        claimed = ip if n % 2 else '::ffff:'+ip
        pairs = [(k, claimed if k == 'X-Real-IP' else v) for k, v in valid]
        assert direct(lab, pairs)[0] == 200
    assert direct(lab, [(k, ip if k == 'X-Real-IP' else v) for k, v in valid])[0] == 429
    assert proxy.call('/register', source=CLIENT_B)[0] == 200
    # Client-controlled forwarded headers cannot split the POST budget at the real TLS proxy.
    data = fields('00'*32, '00'*16)
    for n in range(20):
        assert submit(proxy, data, 'rgg_register='+'00'*32, source='127.0.0.6',
                      headers={'X-Real-IP': f'192.0.2.{n+1}', 'X-Forwarded-For': f'192.0.2.{n+1}'})[0] == 403
    assert submit(proxy, data, 'rgg_register='+'00'*32, source='127.0.0.6')[0] == 429
    assert submit(proxy, data, 'rgg_register='+'00'*32, source='127.0.0.7')[0] == 403
    assert lab.sql('SELECT count(*) FROM accounts') == [(3,)]


def invalid_config(server):
    profile = "[web_proxy]\norigin='https://rgg.test'\ntrusted_proxy='127.0.0.2'\n"
    cases = [("web_public_stats=true\n"+profile.replace('https://rgg.test', origin), '127.0.0.1') for origin in
             ['http://rgg.test', 'https://rgg.test/', 'https://rgg.test:443', 'https://rgg.test:0',
              'https://user@rgg.test', 'https://rgg.test?Canary', 'https://127.1', 'https://0x7f000001']]
    cases += [(profile, '127.0.0.1'), ('web_public_stats=true\n'+profile, '0.0.0.0'),
              ('web_public_stats=true\n'+profile.replace('127.0.0.2', '192.0.2.1'), '127.0.0.1'),
              ('web_public_stats=true\n'+profile+"trust_all=true\n", '127.0.0.1'),
              ('web_public_stats=true\n'+profile.replace("trusted_proxy='127.0.0.2'\n", ''), '127.0.0.1')]
    for config, bind in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-invalid-web-proxy-') as tmp:
            root = Path(tmp)
            path = root/'rgg.toml'
            path.write_text(f"data_dir='data'\nhttp_bind='{bind}:0'\n"+config, encoding='utf-8')
            result = subprocess.run([str(server), '--config', str(path)], capture_output=True, timeout=5)
            assert result.returncode != 0 and not (root/'data').exists()
            assert b'Canary' not in result.stdout+result.stderr
    print(f'PASS: {len(cases)} invalid proxy configurations rejected before database creation', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--admin', type=Path, required=True)
    args = parser.parse_args()
    server, admin = args.server.resolve(), args.admin.resolve()
    invalid_config(server)
    for name, case in [('verified TLS, Secure cookies, IP-bound forms, registration and password session fence', forms_and_sessions),
                       ('proxy trust, hostile and duplicate headers, canonical IP and source budgets', guards_and_budget)]:
        with tempfile.TemporaryDirectory(prefix='rgg-web-proxy-') as tmp:
            root = Path(tmp)
            lab = Lab(server, admin, root)
            invite = lab.run('invite')['codes'][0]
            fixture(root)
            proxy = TLSProxy(root)
            try:
                lab.start(registration=True, passwords=True, statistics=True, discovery=True, metrics=True,
                          downloads='artifacts/manifest.toml',
                          extra=proxy.config()+"\n[client_patcher]\ndefault_host='127.0.0.1'\n")
                proxy.start(lab)
                case(lab, proxy, invite)
                assert proxy.versions and proxy.versions <= {'TLSv1.2', 'TLSv1.3'}
                print('PASS:', name, 'TLS='+','.join(sorted(proxy.versions)), flush=True)
            finally:
                try:
                    proxy.close()
                finally:
                    lab.stop()


if __name__ == '__main__':
    main()
