"""Exercise real App recording handlers against our test-pattern window only.
Scale is egui application zoom, not a change to Windows display settings.
"""
import json
import os
import pathlib
import subprocess
import sys
import time

root = pathlib.Path(__file__).resolve().parents[1]
exe = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / 'target/debug/screen-recorder.exe'
scenario = sys.argv[2] if len(sys.argv) > 2 else 'quick'
scale = sys.argv[3] if len(sys.argv) > 3 else '1'
lang = sys.argv[4] if len(sys.argv) > 4 else 'vi'
folder = root / 'test-output/phase2-ui' / f'{scenario}-{scale}-{lang}'
folder.mkdir(parents=True, exist_ok=True)
(folder / 'settings.json').write_text(json.dumps({'language': 'En' if lang == 'en' else 'Vi', 'system_audio': False, 'microphone': False, 'output': str(folder / 'recordings')}), encoding='utf-8')
for marker in ('report.json', 'close-source.flag'):
    (folder / marker).unlink(missing_ok=True)
env = {**os.environ, 'SCREENRECORDER_SETTINGS_PATH': str(folder / 'settings.json'),
       'SCREENRECORDER_LIBRARY_DIR': str(folder / 'catalog'), 'SCREENRECORDER_UI_SCALE': scale,
       'SCREENRECORDER_UI_CAPTURE': str(folder / 'screen.png'), 'SCREENRECORDER_QA_OUTPUT': str(folder),
       'SCREENRECORDER_QA_SCENARIO': scenario, 'SCREENRECORDER_EDITOR_PROFILE': str(folder / 'editor-profile')}
pattern = subprocess.Popen([str(exe), '--test-pattern'], cwd=root)
env['SCREENRECORDER_QA_PATTERN_PID'] = str(pattern.pid)
app = None
try:
    time.sleep(1)
    app = subprocess.Popen([str(exe), '--ui-smoke', '--inspect-ui'], cwd=root, env=env)
    deadline = time.monotonic() + 90
    while app.poll() is None and time.monotonic() < deadline:
        if (folder / 'close-source.flag').exists() and pattern.poll() is None:
            pattern.terminate()
        time.sleep(.2)
    if app.poll() is None:
        raise RuntimeError('UI acceptance process timed out')
    report = json.loads((folder / 'report.json').read_text(encoding='utf-8'))
    if report.get('editor_pid'):
        subprocess.run(['taskkill', '/PID', str(report['editor_pid']), '/T', '/F'], capture_output=True)
    if scenario in ('quick', 'edit'):
        assert report['error'] is None, report
        assert all(state in report['states'] for state in ['Countdown', 'Recording', 'Paused', 'Finishing', 'Ready']), report
        assert report['editor_opened'] == (scenario == 'edit'), report
        video = pathlib.Path(report['video'])
        data = json.loads(subprocess.check_output([str(root / 'vendor/ffprobe.exe'), '-v', 'error', '-show_streams', '-show_format', '-of', 'json', str(video)]))
        assert 3 <= float(data['format']['duration']) < 8, data
        assert not any(s['codec_type'] == 'audio' for s in data['streams']), data
        subprocess.run([str(root / 'vendor/ffmpeg.exe'), '-v', 'error', '-i', str(video), '-f', 'null', '-'], check=True, capture_output=True, timeout=30)
        report['decoded'] = True
    if scenario in ('disconnected', 'source-loss'):
        assert report['error'], report
        assert not report['editor_opened'], report
    (folder / 'acceptance.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    sys.stdout.reconfigure(encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))
finally:
    if app and app.poll() is None:
        app.terminate()
        app.wait(timeout=10)
    if pattern.poll() is None:
        pattern.terminate()
    pattern.wait(timeout=10)
