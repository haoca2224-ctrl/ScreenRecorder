"""Fetch a pinned official MSI, verify it and extract (never install) Concat.

No Electron/Node dependency is needed. The upstream application is unmodified.
"""
import hashlib
import json
import pathlib
import shutil
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = '0.2.8'
COMMIT = '79116bd0f9a103c633d888603ad27890e929dbdc'
MSI_HASH = 'dc24b7588ec7fe430223ccb06cb7fa54027382e4b8867557bc5515710fa3af98'
DOWNLOAD = ROOT / 'vendor/concat-download'
DOWNLOAD.mkdir(parents=True, exist_ok=True)
msi = DOWNLOAD / f'Concat-{VERSION}-windows-x86_64.msi'
url = f'https://github.com/jub0t/Concat/releases/download/v{VERSION}/{msi.name}'
if not msi.exists():
    partial = msi.with_suffix('.partial')
    urllib.request.urlretrieve(url, partial)
    partial.replace(msi)
if hashlib.sha256(msi.read_bytes()).hexdigest() != MSI_HASH:
    raise SystemExit('Concat MSI SHA256 mismatch; refusing to package it')
extracted = ROOT / 'vendor/concat-extracted'
subprocess.run(['msiexec.exe', '/a', str(msi), '/qn', f'TARGETDIR={extracted}',
                '/l*v', str(ROOT / 'concat-extract.log')], check=True,
               creationflags=subprocess.CREATE_NO_WINDOW)
runtime = extracted / 'PFiles64/Concat'
version = subprocess.check_output([str(runtime / 'concat.exe'), '--version'], timeout=20).decode().strip()
if version != f'concat {VERSION}':
    raise SystemExit(f'Unexpected editor version: {version}')
source = ROOT / f'vendor/concat-source/Concat-{VERSION}-source.zip'
source.parent.mkdir(parents=True, exist_ok=True)
if not source.exists():
    partial = source.with_suffix('.partial')
    urllib.request.urlretrieve(f'https://codeload.github.com/jub0t/Concat/zip/{COMMIT}', partial)
    partial.replace(source)
destination = ROOT / 'dist/ScreenRecorder/concat'
destination.mkdir(parents=True, exist_ok=True)
inventory = {}
for file in runtime.iterdir():
    if file.is_file():
        shutil.copy2(file, destination / file.name)
        inventory[file.name] = hashlib.sha256(file.read_bytes()).hexdigest()
shutil.copy2(source, destination / source.name)
(destination / 'UPSTREAM.json').write_text(json.dumps({
    'version': VERSION, 'commit': COMMIT, 'source': f'https://github.com/jub0t/Concat/tree/{COMMIT}',
    'installer': url, 'installerSha256': MSI_HASH, 'modified': False,
    'sourceSha256': hashlib.sha256(source.read_bytes()).hexdigest(), 'files': inventory,
}, indent=2), encoding='utf-8')
shutil.copy2(ROOT / 'CONCAT.md', destination / 'SCREENRECORDER-INTEGRATION.md')
print(f'Prepared unmodified {version}, {len(inventory)} runtime files, with upstream source archive.')
