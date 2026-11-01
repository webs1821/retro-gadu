# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Bounded readers for the PE32 resources and solid-LZMA NSIS format used by GG6.
Format facts: NSIS firstheader (THIRD_PARTY: nsis-format-reference).
No supplied executable or installer instruction is executed.
"""
from dataclasses import dataclass
import lzma
import struct
import zlib

MAX_FILE = 32 * 1024 * 1024
MAX_STREAM = 16 * 1024 * 1024

class Invalid(ValueError):
    pass

def region(data, start, length):
    if start < 0 or length < 0 or start + length > len(data):
        raise Invalid('binary range exceeds input')
    return data[start:start + length]

def integer(data, offset, width=4):
    return int.from_bytes(region(data, offset, width), 'little')

@dataclass(frozen=True)
class Resource:
    kind: object
    name: object
    language: object
    offset: int
    size: int

class PE32:
    def __init__(self, data):
        if not 256 <= len(data) <= MAX_FILE or data[:2] != b'MZ':
            raise Invalid('not a bounded PE file')
        self.data = data
        pe = integer(data, 60)
        if region(data, pe, 4) != b'PE\0\0' or integer(data, pe + 4, 2) != 0x14c:
            raise Invalid('expected x86 PE32')
        count = integer(data, pe + 6, 2)
        optional_size = integer(data, pe + 20, 2)
        optional = pe + 24
        if not 1 <= count <= 32 or optional_size < 224 or integer(data, optional, 2) != 0x10b:
            raise Invalid('unsupported PE headers')
        region(data, optional, optional_size)
        if integer(data, optional + 92) < 3:
            raise Invalid('resource directory absent')
        self.image_base = integer(data, optional + 28)
        self.sections = []
        for i in range(count):
            entry = optional + optional_size + 40 * i
            region(data, entry, 40)
            rva, size, offset = (integer(data, entry + p) for p in (12, 16, 20))
            region(data, offset, size)
            self.sections.append((rva, size, offset))
        rva, size = (integer(data, optional + p) for p in (112, 116))
        self.resource_size = size
        self.resource_root = self.file_offset(rva, size)

    def file_offset(self, address, size=1):
        matches = [offset + address - rva for rva, length, offset in self.sections
                   if address >= rva and address + size <= rva + length]
        if len(matches) != 1:
            raise Invalid('PE RVA has no unique file range')
        return matches[0]

    def resources(self):
        output, visited = [], set()
        def relative(offset, size):
            if offset < 0 or offset + size > self.resource_size:
                raise Invalid('resource directory range exceeded')
            return self.resource_root + offset
        def name(value):
            if not value & 0x80000000:
                return value
            offset = value & 0x7fffffff
            length = integer(self.data, relative(offset, 2), 2)
            return region(self.data, relative(offset + 2, length * 2), length * 2).decode('utf-16le', 'surrogatepass')
        def walk(offset, path):
            if offset in visited or len(visited) >= 4096 or len(path) > 2:
                raise Invalid('resource directory is cyclic or too deep')
            visited.add(offset)
            table = relative(offset, 16)
            count = integer(self.data, table + 12, 2) + integer(self.data, table + 14, 2)
            if count > 4096 or len(output) + count > 4096:
                raise Invalid('too many resource records')
            for i in range(count):
                entry = relative(offset + 16 + i * 8, 8)
                label, child = integer(self.data, entry), integer(self.data, entry + 4)
                route = path + (name(label),)
                if child & 0x80000000:
                    walk(child & 0x7fffffff, route)
                else:
                    if len(route) != 3:
                        raise Invalid('unexpected resource tree shape')
                    leaf = relative(child, 16)
                    rva, size = integer(self.data, leaf), integer(self.data, leaf + 4)
                    output.append(Resource(*route, self.file_offset(rva, size), size))
        walk(0, ())
        return output

    def strings(self, resource):
        if resource.kind != 6 or not isinstance(resource.name, int):
            raise Invalid('not an RT_STRING block')
        data = region(self.data, resource.offset, resource.size)
        texts, cursor = [], 0
        for _ in range(16):
            size = integer(data, cursor, 2) * 2
            texts.append(region(data, cursor + 2, size).decode('utf-16le', 'surrogatepass'))
            cursor += size + 2
        return texts, data[cursor:]

@dataclass(frozen=True)
class SolidInstaller:
    source: bytes
    offset: int
    header_size: int
    properties: bytes
    stream: bytes

    @classmethod
    def parse(cls, data):
        signature = b'\xef\xbe\xad\xdeNullsoftInst'
        if not 512 <= len(data) <= MAX_FILE or data[:2] != b'MZ' or data.count(signature) != 1:
            raise Invalid('unsupported installer signature')
        offset = data.index(signature) - 4
        if offset < 512 or offset % 512:
            raise Invalid('unaligned installer header')
        header = region(data, offset, 28)
        flags, _, _, _, _, header_size, extent = struct.unpack('<7I', header)
        if flags != 0 or extent != len(data) - offset or not 4 <= header_size < MAX_STREAM:
            raise Invalid('unsupported installer header')
        if zlib.crc32(data[512:-4]) != integer(data, len(data) - 4):
            raise Invalid('installer CRC mismatch')
        properties = region(data, offset + 28, 5)
        dictionary = int.from_bytes(properties[1:], 'little')
        if properties[0] != 0x5d or not 4096 <= dictionary <= 8 * 1024 * 1024:
            raise Invalid('unsupported LZMA properties')
        decoder = lzma.LZMADecompressor(format=lzma.FORMAT_RAW, filters=cls.filters(properties))
        stream = decoder.decompress(data[offset + 33:-4], max_length=MAX_STREAM + 1)
        if not decoder.eof or decoder.unused_data or len(stream) > MAX_STREAM:
            raise Invalid('incomplete or excessive installer stream')
        if integer(stream, 0) != header_size:
            raise Invalid('installer script header mismatch')
        region(stream, 4, header_size)
        return cls(data, offset, header_size, properties, stream)

    @staticmethod
    def filters(properties):
        return [{'id': lzma.FILTER_LZMA1, 'dict_size': int.from_bytes(properties[1:], 'little'),
                 'lc': 3, 'lp': 0, 'pb': 2}]

    def replace_member(self, position, before, after):
        if position < 4 + self.header_size or len(before) != len(after) or region(self.stream, position, len(before)) != before:
            raise Invalid('replacement does not match an equal-sized payload member')
        updated = self.stream[:position] + after + self.stream[position + len(before):]
        encoded = lzma.compress(updated, format=lzma.FORMAT_RAW, filters=self.filters(self.properties))
        prefix = bytearray(self.source[:self.offset + 28])
        struct.pack_into('<I', prefix, self.offset + 24, 37 + len(encoded))
        body = bytes(prefix) + self.properties + encoded
        result = body + struct.pack('<I', zlib.crc32(body[512:]))
        check = self.parse(result)
        if check.stream != updated or result[:self.offset] != self.source[:self.offset]:
            raise Invalid('rebuilt installer verification failed')
        return result
