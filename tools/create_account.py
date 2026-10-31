# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Interactive wrapper: password/invitation are passed over stdin, never in process arguments."""
import argparse
import getpass
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--admin", type=Path, required=True)
    parser.add_argument("--data-dir", type=Path, required=True)
    args = parser.parse_args()
    number = input("Numer GG (Enter = automatyczny): ").strip()
    value = {"requested_uin": int(number) if number else None,
             "email": input("E-mail: ").strip(),
             "invite": getpass.getpass("Kod zaproszenia: "),
             "password": getpass.getpass("Hasło GG: ")}
    result = subprocess.run([str(args.admin.resolve(strict=True)), "account-create", "--data-dir", str(args.data_dir)],
                            input=json.dumps(value).encode(), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=20)
    if result.returncode:
        raise SystemExit(result.stderr.decode().strip())
    print("Utworzono konto GG:", json.loads(result.stdout)["uin"])


if __name__ == "__main__":
    main()
