# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Validate an offline legacy snapshot and pipe a bounded encrypted exchange package to rgg-admin."""
import argparse
import base64
import datetime
import json
import sqlite3
import subprocess
import sys
import time
from pathlib import Path
from inspect_gg_retro import inspect, source_state, digest, Rejected

MAX_PACKAGE = 64 * 1024**2

def timestamp(value):
    if value is None:
        return None
    date = datetime.datetime.fromisoformat(value.replace('Z', '+00:00'))
    if date.tzinfo is None:
        date = date.replace(tzinfo=datetime.timezone.utc)
    return int(date.timestamp())

def build_package(source, reserved, acknowledge, ttl=30*86400):
    if not acknowledge:
        raise Rejected('missing_history_acknowledgment')
    if not isinstance(reserved, list) or len(reserved)>100000 or not all(type(v) is int for v in reserved):
        raise Rejected('invalid_reserved_number_list')
    source = Path(source).absolute()
    report = inspect(source)
    state = source_state(source)
    package = dict(format=1,source_commit=report['source_reference_commit'],source_sha256=report['source_sha256'],
                   missing_history_acknowledged=True,pending_ttl_seconds=ttl,reserved_uins=reserved,
                   next_uin=report['number_allocator_next_uin'],discarded_tokens=report['counts']['tokens'])
    # Fixed projections, types and limits; no SQL or filenames from a supplied package are executed.
    sections = {
        'users':('users','uin,name,email,password,created_at,registration_invite_hash','uin',10000,{'name':1024,'email':254,'password':512,'created_at':64,'registration_invite_hash':32}),
        'invitations':('registration_invites','code_hash,max_uses,used,active,created_at','code_hash',10000,{'code_hash':32,'created_at':64}),
        'contacts':('contact_lists','uin,data','uin',10000,{'data':262144}),
        'presence':('offline_presence','uin,status,description,return_time,privacy','uin',10000,{'description':210,'privacy':5000}),
        'profiles':('public_profiles','uin,firstname,lastname,nickname,city,familyname,familycity,birthyear,gender','uin',10000,{name:180 for name in ('firstname','lastname','nickname','city','familyname','familycity')}),
        'messages':('messages','id,sender_uin,recipient_uin,seq,time,class,message,delivered_at,formatting,conference','id',100000,{'message':16384,'delivered_at':64,'formatting':4096,'conference':4096}),
        'images':('message_images','message_id,size,crc32','message_id,size,crc32',80000,{}),
    }
    estimated = len(json.dumps(package))
    db = sqlite3.connect(source.as_uri()+'?mode=ro&immutable=1',uri=True)
    db.row_factory = sqlite3.Row
    deadline = time.monotonic()+60
    db.set_progress_handler(lambda: time.monotonic()>deadline, 10000)
    try:
        db.execute('PRAGMA query_only=ON')
        db.execute('PRAGMA trusted_schema=OFF')
        for section,(table,projection,order,max_rows,bounds) in sections.items():
            if report['counts'][table]>max_rows:
                raise Rejected('source_record_limit_exceeded')
            for field,maximum in bounds.items():
                if db.execute('SELECT EXISTS(SELECT 1 FROM '+table+' WHERE length(CAST('+field+' AS BLOB))>?)',(maximum,)).fetchone()[0]:
                    raise Rejected('source_field_exceeds_import_limit')
            values=[]
            for row in db.execute('SELECT '+projection+' FROM '+table+' ORDER BY '+order):
                if len(values)>=max_rows or time.monotonic()>deadline:
                    raise Rejected('source_export_limit_exceeded')
                record=dict(row)
                for field in ('created_at','delivered_at'):
                    if field in record:record[field]=timestamp(record[field])
                for field in ('code_hash','registration_invite_hash'):
                    if record.get(field) is not None:record[field]=record[field].hex()
                for field in ('data','privacy','formatting','conference'):
                    if record.get(field) is not None:record[field]=base64.b64encode(record[field]).decode('ascii')
                if 'active' in record:
                    if record['active'] not in (0,1):raise Rejected('invalid_invitation_activity')
                    record['active']=bool(record['active'])
                estimated+=len(json.dumps(record,ensure_ascii=False).encode('utf-8'))+2
                if estimated>MAX_PACKAGE:raise Rejected('import_package_exceeds_limit')
                values.append(record)
            package[section]=values
    finally:
        db.close()
    encoded=json.dumps(package,ensure_ascii=False,separators=(',',':')).encode('utf-8')
    if len(encoded)>MAX_PACKAGE:raise Rejected('import_package_exceeds_limit')
    if state!=source_state(source) or digest(source)!=report['source_sha256'] or state!=source_state(source):
        raise Rejected('source_changed_during_export')
    return encoded

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--admin',type=Path,required=True)
    parser.add_argument('--legacy-key',type=Path,required=True)
    parser.add_argument('--reserved-uins',type=Path,required=True,help='JSON array of additionally reserved UINs; may be empty only with explicit missing-history acknowledgment')
    parser.add_argument('--acknowledge-missing-history',action='store_true')
    parser.add_argument('--pending-ttl-seconds',type=int,default=30*86400,help='Explicit renewal of pending retention, default 2592000 seconds (30 days)')
    mode=parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check',action='store_true')
    mode.add_argument('--data-dir',type=Path)
    args=parser.parse_args()
    try:
        with args.reserved_uins.open('rb') as stream:
            raw=stream.read(1024*1024+1)
        if len(raw)>1024*1024:raise Rejected('reserved_number_file_exceeds_limit')
        data=build_package(args.source,json.loads(raw),args.acknowledge_missing_history,args.pending_ttl_seconds)
        command=[str(args.admin),'import-check' if args.check else 'import-apply','--legacy-key',str(args.legacy_key)]
        if args.data_dir is not None:command+=['--data-dir',str(args.data_dir)]
        result=subprocess.run(command,input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    except (Rejected,OSError,sqlite3.Error,ValueError,TypeError,AttributeError,OverflowError):
        print(json.dumps({'imported':False,'error':'legacy_source_or_options_rejected'}))
        return 2
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    return result.returncode

if __name__=='__main__':raise SystemExit(main())
