# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Offline key rotation with full backups, retained read keys and crash resumption."""
import argparse,json,os,subprocess,tempfile
from pathlib import Path
from gg6_sessions import Lab,PASSWORD
from backups_cli import verify,restore,hashes
def run(lab,command,*args,ok=True,fault=None):
    env=os.environ.copy();env.pop('RGG_TEST_KEY_CRASH',None)
    if fault:env['RGG_TEST_KEY_CRASH']=fault
    p=subprocess.run([str(lab.admin),command,'--data-dir',str(lab.data),*map(str,args)],capture_output=True,timeout=15,env=env)
    assert PASSWORD not in p.stdout+p.stderr
    if ok:assert p.returncode==0,p.stderr;return json.loads(p.stdout)
    assert p.returncode!=0;return p
def normal(lab):
    status=run(lab,'keys-status');old_id=status['active_key_id'];old_file=(lab.data/'keys.toml').read_bytes()
    versions=lab.sql('SELECT uin,credential_version FROM accounts');old_secret=lab.sql('SELECT uin,key_id,nonce,ciphertext FROM accounts')
    # Inactive accounts must also remain decryptable and be rewrapped.
    lab.sql('UPDATE accounts SET active=0 WHERE uin=3')
    backup=lab.root/'before-rotation';rotated=run(lab,'keys-rotate','--backup',backup);assert rotated['rewrapped']==3 and rotated['remaining']==0
    new_id=rotated['active_key_id'];assert new_id!=old_id and old_id in rotated['retained_key_ids']
    assert lab.sql('SELECT DISTINCT key_id FROM accounts')==[(new_id,)] and lab.sql('SELECT count(*) FROM secret_nonces')==[(6,)]
    assert lab.sql('SELECT uin,credential_version FROM accounts')==versions
    assert (backup/'keys.toml').read_bytes()==old_file;verify(lab,backup);saved=hashes(backup)
    assert run(lab,'keys-resume')['rewrapped']==0
    second=run(lab,'keys-rotate','--backup',lab.root/'second');assert second['rewrapped']==3 and len(second['retained_key_ids'])==3
    code=lab.run('invite')['codes'][0];lab.run('account-create',payload={'requested_uin':4,'invite':code,'password':PASSWORD.decode(),'email':'synthetic@example.invalid'})
    assert lab.sql('SELECT key_id FROM accounts WHERE uin=4')==[(second['active_key_id'],)]
    lab.start();lab.client(1).ping();lab.client(2).ping();lab.client(4).ping();lab.stop()
    # Restore the pre-rotation state with its OWN keyring; original passwords still work.
    restored=lab.root/'restored';restore(lab,backup,restored);lab.data=restored
    assert lab.sql('SELECT uin,key_id,nonce,ciphertext FROM accounts')==old_secret
    runtime=lab.root/'runtime';runtime.mkdir();(runtime/'data').symlink_to(restored,target_is_directory=True);lab.root=runtime
    lab.start();lab.client(1).ping();lab.client(2).ping();lab.stop();assert hashes(backup)==saved
def refused(lab):
    source=(lab.data/'keys.toml').read_bytes();backup=lab.root/'no-backup'
    run(lab,'keys-rotate',ok=False);assert (lab.data/'keys.toml').read_bytes()==source
    lab.start();p=run(lab,'keys-rotate','--backup',backup,ok=False);assert b'another process' in p.stderr and not backup.exists();lab.stop()
    pending=lab.data/'keys.pending';pending.write_bytes(b'incomplete');pending.chmod(0o600)
    run(lab,'keys-rotate','--backup',backup,ok=False);run(lab,'keys-resume',ok=False)
    assert (lab.data/'keys.toml').read_bytes()==source and not backup.exists();pending.unlink()
    # A valid unrelated keyring is still not a retaining successor.
    other=lab.root/'other';p=subprocess.run([str(lab.admin),'init','--data-dir',str(other)],capture_output=True);assert p.returncode==0
    pending.write_bytes((other/'keys.toml').read_bytes());pending.chmod(0o600);run(lab,'keys-resume',ok=False);assert (lab.data/'keys.toml').read_bytes()==source;pending.unlink()
    # Verify EVERY account before publishing any key or backup, including a non-first key probe.
    lab.sql('UPDATE accounts SET ciphertext=zeroblob(length(ciphertext)) WHERE uin=2');run(lab,'keys-rotate','--backup',backup,ok=False)
    assert (lab.data/'keys.toml').read_bytes()==source and not backup.exists()
def crash(lab,stage):
    original=run(lab,'keys-status')['active_key_id'];backup=lab.root/'before'
    p=run(lab,'keys-rotate','--backup',backup,ok=False,fault=stage);assert p.returncode==86
    status=run(lab,'keys-status');verify(lab,backup)
    if stage=='before_key_publish':assert status['active_key_id']==original and status['pending_key_file'] and status['remaining']==0
    elif stage=='after_key_publish':assert status['active_key_id']!=original and not status['pending_key_file'] and status['remaining']==3
    else:assert status['remaining']==2 and len(status['accounts_per_key'])==2
    run(lab,'keys-rotate','--backup',lab.root/'forbidden',ok=False);assert not (lab.root/'forbidden').exists()
    # Even an interrupted rotation has a complete, usable keyring for every existing row.
    lab.start();lab.client(1).ping();lab.client(2).ping();lab.client(3).ping();lab.stop()
    done=run(lab,'keys-resume');assert done['remaining']==0 and original in done['retained_key_ids']
    assert len(done['retained_key_ids'])==2 and not (lab.data/'keys.pending').exists()
    assert run(lab,'keys-resume')['rewrapped']==0
    lab.start();lab.client(1).ping();lab.client(2).ping();lab.client(3).ping();lab.stop()
def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);p.add_argument('--faults',action='store_true');a=p.parse_args()
    cases=[('rekey all accounts, retain keys, unchanged passwords/versions, older backup restore',normal),('refuse active server, incomplete/unrelated pending keyring, corrupt account',refused)]
    if a.faults:
        for stage in ['before_key_publish','after_key_publish','after_rekey_batch']:cases.append((stage,lambda lab,stage=stage:crash(lab,stage)))
    for name,test in cases:
        with tempfile.TemporaryDirectory(prefix='rgg-keys-') as root:
            lab=Lab(a.server.resolve(),a.admin.resolve(),Path(root))
            try:test(lab);print('PASS:',name,flush=True)
            finally:lab.stop()
if __name__=='__main__':main()
