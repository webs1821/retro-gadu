# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Independent, hash-pinned GG6 patch recipe; compatibility facts credited in THIRD_PARTY.
PE string-table entries are rebuilt as length-prefixed strings, not padded C strings.
"""
from dataclasses import dataclass
import hashlib
import re
import struct
from client_format import Invalid, PE32, region

INSTALLER_SHA256 = 'bcc8157aa6bface009d8018c308bf3cef8725546b4f826bdbaf6bbeaa953b06f'
CLIENT_SHA256 = '74b78af318c52f86dff8a7ee27a7a0b5ea545167aebb594a8a559ec97f2e3a57'
CLIENT_SIZE = 790528
HOSTS = tuple(name + '.gadu-gadu.pl' for name in
              ('appsrv', 'appmsg', 'register', 'adserver', 'update', 'www', 'pubdir', 'retr', 'smsat'))

@dataclass(frozen=True)
class Edit:
    start: int
    before: bytes
    after: bytes
    reason: str

def sha(data):
    return hashlib.sha256(data).hexdigest()

def hostname(value):
    if not isinstance(value, str) or not 1 <= len(value) <= 16 or not re.fullmatch(r'[A-Za-z0-9]+(?:[.-][A-Za-z0-9]+)*', value):
        raise Invalid('hostname must contain 1-16 ASCII letters/digits with internal dots or hyphens')
    return value

def endpoint_text(text, destination):
    return re.sub('|'.join(re.escape(host) for host in HOSTS), lambda _: destination, text)

def domain_edits(data, destination):
    pe = PE32(data)
    resources = pe.resources()
    tables = [r for r in resources if r.kind == 6]
    edits = []
    for resource in tables:
        old, tail = pe.strings(resource)
        new = [endpoint_text(text, destination) for text in old]
        if new != old:
            parts = []
            for text in new:
                encoded = text.encode('utf-16le', 'surrogatepass')
                parts.append(struct.pack('<H', len(encoded) // 2) + encoded)
            rebuilt = b''.join(parts) + tail
            if len(rebuilt) > resource.size:
                raise Invalid('resource replacement would grow')
            edits.append(Edit(resource.offset, region(data, resource.offset, resource.size),
                              rebuilt.ljust(resource.size, b'\0'), 'RT_STRING'))
    for encoding, width in [('ascii', 1), ('utf-16le', 2)]:
        ranges = []
        for old in HOSTS:
            for match in re.finditer(re.escape(old.encode(encoding)), data):
                start = match.start()
                if 0x841bc <= start < 0x841dc:  # This retired URL becomes sound-name storage.
                    continue
                if any(r.offset <= start < r.offset + r.size for r in tables):
                    continue
                end = start
                while region(data, end, width) != bytes(width):
                    end += width
                    if end - start > 8192:
                        raise Invalid('endpoint string has no bounded terminator')
                ranges.append((start, end))
        # Multiple endpoint occurrences in one C string must be rewritten together.
        merged = []
        for start, end in sorted(set(ranges)):
            if merged and start < merged[-1][1]:
                merged[-1] = (merged[-1][0], max(end, merged[-1][1]))
            else:
                merged.append((start, end))
        for start, end in merged:
            before = data[start:end]
            after = before
            pattern = b'|'.join(re.escape(host.encode(encoding)) for host in HOSTS)
            after = re.sub(pattern, lambda _: destination.encode(encoding), before)
            if len(after) > len(before):
                raise Invalid('endpoint replacement would grow')
            edits.append(Edit(start, before, after.ljust(len(before), b'\0'), encoding + ' endpoint'))
    return edits

def apply_edits(data, edits):
    output = bytearray(data)
    end = 0
    for edit in sorted(edits, key=lambda e: e.start):
        if edit.start < end or len(edit.before) != len(edit.after) or region(data, edit.start, len(edit.before)) != edit.before:
            raise Invalid('overlapping or mismatched patch preconditions')
        output[edit.start:edit.start + len(edit.before)] = edit.after
        end = edit.start + len(edit.before)
    return bytes(output)

def contact_upload_stub():
    """Own x86 code: append one empty MORE after a positive multiple of2047.

    Preserve the original thiscall ABI,50000-byte cap, first empty PUT and
    epilogue. This code is not copied from another client/server.
    """
    base = 0x479400
    code = bytearray()
    fixups = []
    labels = {}

    def emit(value):
        code.extend(bytes.fromhex(value))

    def branch(opcode, target):
        emit(opcode)
        fixups.append((len(code), target))
        code.extend(bytes(4))

    emit('3bf0')                       # cmp esi,eax (eax is zero at the hook)
    branch('0f8f', 0x424a42)           # remaining bytes: original send path
    emit('3945fc')                     # cmp [ebp-4],eax (first fragment)
    branch('0f85', 0x424a42)           # retain the original empty PUT
    emit('85db')                       # test ebx,ebx (total bytes already sent)
    branch('0f84', 'exit')             # zero bytes need no extra terminator
    emit('89d8 31d2 b9ff070000 f7f1 85d2')  # divide total by2047; test remainder
    branch('0f85', 'exit')             # a short last fragment already finished
    emit('ff879c000000')               # count the additional expected reply
    emit('c685fcf7ffff01')             # existing stack buffer: MORE with no data
    emit('6a01 8d85fcf7ffff 50 6a16 8bcf')  # length1, buffer, kind0x16, this=edi
    branch('e8', 0x42459a)             # existing packet sender; callee pops args
    labels['exit'] = base + len(code)
    emit('31c0')                       # retain the original zero return value
    branch('e9', 0x424a85)             # original saved-register/stack epilogue
    for position, target in fixups:
        destination = labels[target] if isinstance(target, str) else target
        struct.pack_into('<i', code, position, destination - (base + position + 4))
    if len(code) > 128:
        raise Invalid('contact upload stub exceeds its reserved padding')
    return bytes(code)


def contact_upload_edits():
    return [
        Edit(0x220, struct.pack('<I', 0x78398), struct.pack('<I', 0x78480),
             'contact uploader mapped padding'),
        Edit(0x24a39, bytes.fromhex('3bf07f053945fc7443'),
             b'\xe9' + struct.pack('<i', 0x479400 - 0x424a39 - 5) + b'\x90' * 4,
             'contact uploader completion branch'),
        Edit(0x79400, bytes(128), contact_upload_stub().ljust(128, b'\0'),
             'contact uploader empty final fragment'),
    ]


def patch_client(data, destination):
    hostname(destination)
    if len(data) != CLIENT_SIZE or sha(data) != CLIENT_SHA256:
        raise Invalid('only the pinned original GG 6.1.0.158 executable is supported')
    pe = PE32(data)
    if pe.image_base != 0x400000 or pe.file_offset(0x841bc, 32) != 0x841bc:
        raise Invalid('unexpected GG sound-storage mapping')
    edits = domain_edits(data, destination)
    edits += [
        Edit(0x45f80, bytes.fromhex('ff257ca84700ff2578a84700ff2574a84700'),
             bytes.fromhex('31c0c3909090') * 3, 'cdecl updater import thunks'),
        Edit(0x841bc, b'http://update.gadu-gadu.pl/auto\0',
             b'wiadomosc\0dostepny\0'.ljust(32, b'\0'), 'sound basenames'),
        Edit(0x33207, bytes.fromhex('6885000000'), bytes.fromhex('68bc414800'), 'message sound pointer'),
        Edit(0x33221, bytes.fromhex('6886000000e8caacfeff59'),
             bytes.fromhex('68c6414800589090909090'), 'online sound pointer and stack cleanup'),
    ]
    edits += contact_upload_edits()
    output = apply_edits(data, edits)
    # Decode the resulting table with the same structural reader; native Windows tests are separate.
    changed = PE32(output)
    if changed.file_offset(0x79400, 128) != 0x79400:
        raise Invalid('contact uploader code mapping failed')
    for r in pe.resources():
        if r.kind == 6:
            old, _ = pe.strings(r)
            new, _ = changed.strings(r)
            if new != [endpoint_text(text, destination) for text in old]:
                raise Invalid('string-table verification failed')
    return output, edits
