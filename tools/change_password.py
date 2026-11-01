# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Interactive offline password change; secrets go only to the child's stdin."""
import argparse,getpass,json,subprocess,sys,warnings
from pathlib import Path
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--admin',type=Path,required=True);p.add_argument('--data-dir',type=Path,required=True);p.add_argument('--uin',type=int,required=True)
    a=p.parse_args()
    if not sys.stdin.isatty():raise SystemExit('Uruchom pomocnika w interaktywnym terminalu z ukrytym wpisywaniem hasła.')
    warnings.simplefilter('error',getpass.GetPassWarning)
    try:
        old=getpass.getpass('Obecne hasło GG: ');new=getpass.getpass('Nowe hasło GG: ')
        if new!=getpass.getpass('Powtórz nowe hasło GG: '):raise SystemExit('Nowe hasła różnią się; nie wykonano zmiany.')
        result=subprocess.run([str(a.admin.resolve(strict=True)),'password-change','--data-dir',str(a.data_dir),'--uin',str(a.uin)],input=json.dumps({'current_password':old,'new_password':new}).encode(),capture_output=True,timeout=30)
    except (getpass.GetPassWarning,EOFError,KeyboardInterrupt):raise SystemExit('Przerwano wpisywanie; nie wykonano zmiany.')
    if result.returncode:raise SystemExit(result.stderr.decode().strip())
    print('Zmieniono hasło konta GG:',json.loads(result.stdout)['uin'])
if __name__=='__main__':main()
