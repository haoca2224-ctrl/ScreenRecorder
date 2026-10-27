"""Record only this script's test-pattern window; validate the Rust output.

Does not capture the desktop, microphone, or system audio. Run --concat-check
with the video path written to test-output/handoff-recording.json.
"""
import ctypes
from ctypes import wintypes
import json
import pathlib
import subprocess
import sys
import time

root = pathlib.Path(__file__).resolve().parents[1]
exe = root / 'dist/ScreenRecorder/ScreenRecorder.exe'
user32 = ctypes.windll.user32
callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.IsWindowVisible.argtypes = [wintypes.HWND]
user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
pattern = subprocess.Popen([str(exe), '--test-pattern'], cwd=root)
try:
    handles = []

    @callback_type
    def visit(hwnd, _):
        pid = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        title = ctypes.create_unicode_buffer(256)
        user32.GetWindowTextW(hwnd, title, len(title))
        if pid.value == pattern.pid and title.value == 'Recorder Test Pattern' and user32.IsWindowVisible(hwnd):
            handles.append(hwnd)
        return True

    for _ in range(100):
        user32.EnumWindows(visit, 0)
        if handles:
            break
        time.sleep(0.1)
    if not handles:
        raise RuntimeError('Test-pattern window did not open')
    time.sleep(0.5)
    result = subprocess.run([str(exe), '--smoke-test', '2', '--pause', '--mute',
                             '--hwnd', str(handles[0])], cwd=root, capture_output=True,
                            text=True, encoding='utf-8', timeout=100)
    (root / 'test-output').mkdir(exist_ok=True)
    (root / 'test-output/handoff-recording.log').write_text(result.stdout + result.stderr, encoding='utf-8')
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    video = next(line[7:] for line in result.stdout.splitlines() if line.startswith('OUTPUT='))
    metadata = json.loads(subprocess.check_output([
        str(root / 'vendor/ffprobe.exe'), '-v', 'error', '-show_streams', '-show_format', '-of', 'json', video
    ]))
    streams = metadata['streams']
    assert any(stream['codec_name'] == 'h264' for stream in streams)
    assert all(stream['codec_type'] != 'audio' for stream in streams)
    assert 3.3 < float(metadata['format']['duration']) < 5.5
    subprocess.run([str(root / 'vendor/ffmpeg.exe'), '-v', 'error', '-i', video, '-f', 'null', '-'],
                   check=True, capture_output=True, timeout=30)
    report = {'video': video, 'duration': float(metadata['format']['duration']),
              'pause_resume': True, 'decode_ok': True, 'source': 'test-pattern window only'}
    (root / 'test-output/handoff-recording.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    sys.stdout.reconfigure(encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))
finally:
    pattern.terminate()
    pattern.wait(timeout=10)
