# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Offline administrator recovery; mandatory new backup and hidden password input."""
import argparse
import getpass
import json
import subprocess
import sys
import warnings
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--admin', type=Path, required=True)
    parser.add_argument('--data-dir', type=Path, required=True)
    parser.add_argument('--uin', type=int, required=True)
    parser.add_argument('--backup', type=Path, required=True)
    args = parser.parse_args()
    if not sys.stdin.isatty():
        raise SystemExit('Uruchom pomocnika w interaktywnym terminalu z ukrytym wpisywaniem hasła.')
    warnings.simplefilter('error', getpass.GetPassWarning)
    try:
        print('Serwer musi być zatrzymany. Przed resetem powstanie pełna kopia bazy i kluczy.')
        if input(f'Potwierdź numer konta {args.uin}, wpisując go ponownie: ').strip() != str(args.uin):
            raise SystemExit('Numer nie jest zgodny; nie wykonano resetu.')
        new = getpass.getpass('Nowe hasło GG (8–15 znaków ASCII bez spacji): ')
        if new != getpass.getpass('Powtórz nowe hasło GG: '):
            raise SystemExit('Nowe hasła różnią się; nie wykonano resetu.')
        result = subprocess.run(
            [str(args.admin.resolve(strict=True)), 'password-reset', '--data-dir', str(args.data_dir),
             '--uin', str(args.uin), '--backup', str(args.backup)],
            input=json.dumps({'confirm_uin': args.uin, 'new_password': new}).encode(),
            capture_output=True, timeout=300)
    except (getpass.GetPassWarning, EOFError, KeyboardInterrupt):
        raise SystemExit('Przerwano operację. Jeśli uruchomiono już narzędzie, sprawdź audyt i kopię przed ponowieniem.')
    except subprocess.TimeoutExpired:
        raise SystemExit('Przekroczono czas operacji. Sprawdź audyt i kopię przed ponowieniem.')
    if result.returncode:
        raise SystemExit(result.stderr.decode().strip())
    print('Zresetowano hasło konta GG:', json.loads(result.stdout)['uin'])


if __name__ == '__main__':
    main()
