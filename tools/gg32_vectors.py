# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2024-2026 Oreeeee
# SPDX-FileCopyrightText: 2026 webs1821
# Test oracle ported from poggadaj GG32LoginHash; provenance: poggadaj-gg32.
# Original source and immutable revision: THIRD_PARTY.toml.
"""Produce/check synthetic regression vectors. This is not an independent GG interoperability test."""
import argparse
import json
from pathlib import Path


def oracle(password, seed):
    x, y = 0, seed
    for byte in password:
        x = (x & 0xffffff00) | byte
        y ^= x
        y = (y + x) & 0xffffffff
        x = (x << 8) & 0xffffffff
        y ^= x
        x = (x << 8) & 0xffffffff
        y = (y - x) & 0xffffffff
        x = (x << 8) & 0xffffffff
        y ^= x
        rotation = y & 31
        y = ((y << rotation) | (y >> (32 - rotation))) & 0xffffffff
    return y


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    cases = [(b"", 0), (b"", 0xffffffff), (b"LegacyTest9!", 0x12345678),
             (b"\x80\xff\xaf\xf3\xb3\xe6", 0xffffffff), (b"\0", 0), (bytes(range(256)), 0x80000000)]
    rows = [{"input": list(text), "seed": seed, "output": oracle(text, seed)} for text, seed in cases]
    path = Path(__file__).resolve().parents[1] / "tests/fixtures/gg32.json"
    if args.write:
        with path.open("w", encoding="utf-8", newline="\n") as handle:
            handle.write(json.dumps(rows, indent=2) + "\n")
    else:
        assert json.loads(path.read_text(encoding="utf-8")) == rows, "GG32 vector mismatch"
        print("OK: 6 synthetic GG32 vectors match the pinned algorithm")


if __name__ == "__main__":
    main()
