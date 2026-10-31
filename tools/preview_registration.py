# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Opt-in local UI preview using a temporary synthetic database; Enter stops and cleans it."""
import argparse, sys, tempfile
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"tests"))
from gg6_sessions import Lab

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--server",type=Path,required=True);parser.add_argument("--admin",type=Path,required=True);args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rgg-ui-preview-") as root:
        lab=Lab(args.server.resolve(),args.admin.resolve(),Path(root))
        try:
            lab.start(registration=True,statistics=True)
            print("Preview: http://"+lab.server.address+"/",flush=True)
            input()
        finally:lab.stop()
if __name__=="__main__":main()
