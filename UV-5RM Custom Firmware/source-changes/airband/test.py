"""Run AIRBAND host checks; no radio connection is opened."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
ROOT=Path(__file__).resolve().parents[2]
subprocess.run([sys.executable,'-m','unittest','discover','-s',str(Path(__file__).parent),'-p','test_airband.py'],check=True)
host=next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host: '))
for feature in ['', 'receiver','nearby','listener']:
    cmd=['cargo','test','--locked','--manifest-path',str(ROOT/'firmware-source/tools/airband-tests/Cargo.toml'),'--target',host,'--target-dir','/tmp/uv5rm-airband-tests']
    if feature:cmd+=['--features',feature]
    subprocess.run(cmd+['--','--test-threads=1'],check=True,env=os.environ.copy())

# Exercise actual firmware menu mappings with and without the AIRBAND profile.
with tempfile.TemporaryDirectory(prefix='airband-menu-tests-') as folder:
    for enabled in [False, True]:
        binary=str(Path(folder)/('airband' if enabled else 'original'))
        cmd=['rustc','--edition=2021','--test',str(ROOT/'firmware-source/src/app/menu_routes.rs'),'-o',binary]
        if enabled:cmd+=['--cfg','feature="airband"']
        subprocess.run(cmd,check=True)
        subprocess.run([binary],check=True)
