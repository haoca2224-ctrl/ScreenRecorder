"""Isolated native-library visual/performance fixtures; never touches user settings."""
import json
import pathlib
import subprocess
import os
import time

root = pathlib.Path(__file__).resolve().parents[1]
work = root / 'test-output/phase1-ui'
media = work / 'recordings'
catalog = work / 'catalog'
media.mkdir(parents=True, exist_ok=True)
catalog.mkdir(exist_ok=True)
video = media / 'Hướng dẫn quay màn hình.mp4'
picture = media / 'Ảnh chụp màn hình.png'
subprocess.run([str(root / 'vendor/ffmpeg.exe'), '-v', 'error', '-y',
                '-f', 'lavfi', '-i', 'testsrc2=size=640x360:rate=30', '-t', '3',
                '-an', '-c:v', 'libx264', '-preset', 'ultrafast', str(video)],
               check=True, capture_output=True, timeout=30)
subprocess.run([str(root / 'vendor/ffmpeg.exe'), '-v', 'error', '-y', '-i', str(video), '-frames:v', '1', str(picture)], check=True)
(media / 'Video bị lỗi.mp4').write_bytes(b'invalid fixture')
env = {**os.environ, 'SCREENRECORDER_LIBRARY_DIR': str(catalog)}
subprocess.run([str(root / 'target/release/screen-recorder.exe'), '--library-scan', str(media), str(work / 'scan.json')], env=env, check=True, timeout=60)
db = json.loads((catalog / 'index.json').read_text(encoding='utf-8'))
template = next(i for i in db['items'] if i['path'] == str(picture))
assert template['state'] == 'Ready'
for n in range(1500):
    file = media / f'Bộ sưu tập {n:04}.png'
    shutil.copyfile(picture, file)
    stat = file.stat()
    item = {**template, 'id': f'fixture-{n:04}', 'path': str(file), 'identity': None,
            'name': f'Bộ sưu tập {n:04}', 'favorite': n % 17 == 0,
            'created': int((time.time() - n * 3600) * 1000), 'modified': int(stat.st_mtime_ns // 1_000_000),
            'size': stat.st_size, 'aliases': []}
    db['items'].append(item)
db['items'].append({**template, 'id': 'missing-fixture', 'path': str(media / 'Đã di chuyển bên ngoài.png'), 'name': 'Ảnh đã di chuyển bên ngoài', 'state': 'Missing', 'identity': None})
for name in ['index.json', 'index.backup.json']:
    (catalog / name).write_text(json.dumps(db, ensure_ascii=False), encoding='utf-8')
settings = work / 'settings.json'
settings.write_text(json.dumps({'output': str(media), 'language': 'Vi', 'system_audio': False, 'microphone': False}), encoding='utf-8')
print(json.dumps({'root': str(work), 'settings': str(settings), 'catalog': str(catalog), 'count': len(db['items'])}, ensure_ascii=True))
