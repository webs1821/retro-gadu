# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
"""Small functional check of the load driver; explicitly not the R0 capacity test."""
import argparse,asyncio,re,tempfile
from pathlib import Path
from types import SimpleNamespace
from lifecycle import Server
from r0_workload import prepare,run

def main():
    p=argparse.ArgumentParser();p.add_argument('--server',type=Path,required=True);p.add_argument('--admin',type=Path,required=True);args=p.parse_args()
    with tempfile.TemporaryDirectory(prefix='rgg-r0-smoke-') as tmp:
        root=Path(tmp);prepare(args.admin.resolve(),root/'data',8)
        config=root/'rgg.toml'
        config.write_text("data_dir='data'\nhttp_bind='127.0.0.1:0'\ngg_bind='127.0.0.1:0'\nmetrics_bind='127.0.0.1:0'\n",encoding='utf-8')
        server=Server(args.server.resolve(),config,root/'server.log')
        try:
            server.ready();log=server.log.read_text()
            gg=int(re.search(r'gg_endpoint=127\.0\.0\.1:(\d+)',log).group(1))
            metrics=int(re.search(r'metrics_endpoint=127\.0\.0\.1:(\d+)',log).group(1))
            report=asyncio.run(run(SimpleNamespace(host='127.0.0.1',gg_port=gg,metrics_port=metrics,sessions=6,offline=2,seconds=5,rate=2)))
            assert report['accepted']==10 and report['online_verified']==5 and report['offline_verified']==5
            print('PASS: load driver smoke, 6 sessions/10 messages; R0 not qualified',flush=True)
        finally:server.close()
if __name__=='__main__':main()
