# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Opt-in local UI preview using a temporary synthetic database; Enter stops and cleans it."""
import argparse, sys, tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"tests"))
from gg6_sessions import Lab

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--server",type=Path,required=True);parser.add_argument("--admin",type=Path,required=True);parser.add_argument("--downloads",action="store_true");args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rgg-ui-preview-") as root:
        lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
        try:
            if args.downloads:
                from http_downloads import fixture,CONTENT
                folder,manifest=fixture(Path(root),title='GG6 — przykładowy klient')
                (folder/'setup.exe').write_bytes(CONTENT)
                second=manifest[manifest.index('[[artifacts]]'):].replace('id="client"','id="installer"').replace('kind="client"','kind="installer"').replace('file="client.exe"','file="setup.exe"')
                import re
                second=re.sub(r'title=.*','title="GG6 — przykładowy instalator"',second)
                second=re.sub(r'instructions=.*','instructions="Próbka do podglądu strony, nie jest prawdziwym programem. W tym miejscu będzie instrukcja instalacji."',second)
                (folder/'manifest.toml').write_text(manifest+second,encoding='utf-8')
            lab.start(registration=True,statistics=True,downloads='artifacts/manifest.toml' if args.downloads else None)
            print("Preview: http://"+lab.server.address+("/downloads" if args.downloads else "/"),flush=True)
            input()
        finally:lab.stop()
if __name__=="__main__":main()
