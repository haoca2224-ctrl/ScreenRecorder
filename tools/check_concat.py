"""Real-engine acceptance: import, edit, save/reopen, export, legacy preservation.

Creates only synthetic fixtures under test-output. No desktop/audio capture.
Usage: python -X utf8 tools/check_concat.py [path/to/ScreenRecorder.exe]
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
EXE = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / 'target/debug/screen-recorder.exe'
RUN = ROOT / 'test-output' / ('concat-acceptance-' + str(time.time_ns()))
RUN.mkdir(parents=True)
ENV = {**os.environ, 'SCREENRECORDER_LIBRARY_DIR': str(RUN / 'catalog')}
CONCAT = EXE.parent / 'concat/concat.exe'
if not CONCAT.exists():
    CONCAT = ROOT / 'vendor/concat-extracted/PFiles64/Concat/concat.exe'


class Mcp:
    def __init__(self):
        env = {**ENV, 'CONCAT_MCP_ROOTS': str(RUN), 'APPDATA': str(RUN / 'concat-profile'),
               'LOCALAPPDATA': str(RUN / 'concat-profile')}
        self.log = open(RUN / 'test-mcp.log', 'ab')
        self.p = subprocess.Popen([str(CONCAT), '--mcp', 'headless'], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=self.log, env=env)
        self.n = 0
        self.rpc('initialize', {'protocolVersion': '2025-11-25', 'capabilities': {},
                               'clientInfo': {'name': 'screenrecorder-test', 'version': '1'}})

    def rpc(self, method, params):
        self.n += 1
        self.p.stdin.write((json.dumps({'jsonrpc': '2.0', 'id': self.n, 'method': method,
                                       'params': params}) + '\n').encode())
        self.p.stdin.flush()
        while True:
            line = self.p.stdout.readline()
            assert line, 'Concat unexpectedly exited'
            result = json.loads(line)
            if result.get('id') == self.n:
                assert 'error' not in result, result
                return result['result']

    def call(self, name, args):
        result = self.rpc('tools/call', {'name': name, 'arguments': args})
        assert not result.get('isError'), result
        return result.get('structuredContent', result)

    def close(self):
        self.p.stdin.close()
        try:
            self.p.wait(timeout=15)
        except subprocess.TimeoutExpired:
            self.p.kill()
            self.p.wait()
        self.log.close()


def run(args, success=True, timeout=120):
    p = subprocess.run([str(EXE), *map(str, args)], capture_output=True, env=ENV, timeout=timeout)
    assert (p.returncode == 0) == success, (args, p.returncode, p.stderr.decode(errors='replace'))
    return p


def probe(path):
    return json.loads(subprocess.check_output([str(ROOT / 'vendor/ffprobe.exe'), '-v', 'error',
                      '-count_frames', '-show_streams', '-show_format', '-of', 'json', str(path)]))


results = []
legacy = RUN / 'Dự án cũ.taoclip'
legacy.write_bytes(b'{"timelines":[],"legacyEdit":"MUST KEEP"}')
old_hash = hashlib.sha256(legacy.read_bytes()).hexdigest()
for rate, sound in [('30', False), ('60', True), ('30000/1001', True)]:
    name = f'Bản quay thử {rate.replace("/", "-")} có khoảng trắng.mp4'
    source = RUN / name
    cmd = [str(ROOT / 'vendor/ffmpeg.exe'), '-v', 'error', '-f', 'lavfi', '-i',
           f'testsrc2=size=640x360:rate={rate}']
    if sound:
        cmd += ['-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000']
    cmd += ['-t', '3', '-c:v', 'libx264', '-preset', 'ultrafast', '-pix_fmt', 'yuv420p']
    if sound:
        cmd += ['-c:a', 'aac']
    subprocess.run(cmd + [str(source)], check=True, capture_output=True, timeout=30)
    original = hashlib.sha256(source.read_bytes()).hexdigest()
    output = RUN / f'export-{rate.replace("/", "-")}.mp4'
    run(['--concat-check', source, output])
    report = json.loads(output.with_suffix('.report.json').read_text(encoding='utf-8'))
    manifest = pathlib.Path(report['project'])
    summary = report['timeline']['timeline']
    assert len(summary['clips']) == 1 and summary['clips'][0]['start'] == 0
    data = probe(output)
    video = next(s for s in data['streams'] if s['codec_type'] == 'video')
    assert (video['width'], video['height']) == (640, 360)
    assert video['r_frame_rate'] == (rate if '/' in rate else rate + '/1'), video
    assert any(s['codec_type'] == 'audio' for s in data['streams']) == sound
    assert abs(float(data['format']['duration']) - 3) < .15
    subprocess.run([str(ROOT / 'vendor/ffmpeg.exe'), '-v', 'error', '-i', str(output),
                    '-f', 'null', '-'], check=True, capture_output=True, timeout=30)
    # An actual edit must survive a new process, not just an unchanged save.
    m = Mcp()
    try:
        opened = m.call('project_open', {'path': str(manifest.parent)})
        clip = opened['timeline']['clips'][0]['id']
        m.call('clips_split', {'path': str(manifest.parent), 'clipIds': [clip], 'time': 1.0})
        m.call('project_save', {'path': str(manifest.parent)})
        m.call('project_close', {'path': str(manifest.parent), 'save': True})
    finally:
        m.close()
    second = output.with_stem(output.stem + '-edited')
    run(['--concat-check', source, second])
    edited = json.loads(second.with_suffix('.report.json').read_text(encoding='utf-8'))
    assert len(edited['timeline']['timeline']['clips']) == 2, edited
    assert edited['project'] == report['project']
    assert hashlib.sha256(source.read_bytes()).hexdigest() == original
    # Never overwrite an existing output or the source.
    run(['--concat-check', source, source], success=False)
    run(['--concat-check', source, output], success=False)
    results.append({'rate': rate, 'audio': sound, 'manifest': str(manifest),
                    'savedEditReopened': True, 'decodeOk': True, 'probe': data})

run(['--open-editor', legacy], success=False)
assert hashlib.sha256(legacy.read_bytes()).hexdigest() == old_hash
run(['--open-editor', RUN / 'missing.mp4'], success=False)
bad = RUN / 'broken.mp4'
bad.write_bytes(b'not a movie')
run(['--concat-check', bad, RUN / 'bad-output.mp4'], success=False)
run(['--library-scan', RUN, RUN / 'library-report.json'])
library = json.loads((RUN / 'library-report.json').read_text(encoding='utf-8'))
projects = [i for i in library['items'] if i['kind'] == 'Project' and i['path'].endswith('concat.json')]
assert len(projects) == 3, projects
assert all(i['state'] == 'Ready' and i['metadata']['width'] == 640 for i in projects), projects
assert any(i['path'] == str(legacy) for i in library['items'])
report = {'executable': str(EXE), 'cases': results, 'legacyPreserved': True,
          'invalidInputsRejected': True, 'libraryProjects': len(projects), 'runDirectory': str(RUN)}
(RUN / 'acceptance.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps({'passed': True, 'report': str(RUN / 'acceptance.json')}, ensure_ascii=False))
