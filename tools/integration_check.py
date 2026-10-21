"""Local acceptance checks. Records only the explicitly supplied test-window HWND."""
import array
import json
import pathlib
import subprocess
import sys
import time
import winsound
sys.stdout.reconfigure(encoding="utf-8")

root = pathlib.Path(__file__).resolve().parents[1]
exe = root / "target/release/screen-recorder.exe"
ffprobe = root / "vendor/ffprobe.exe"
ffmpeg = root / "vendor/ffmpeg.exe"
hwnd = sys.argv[1]
cases = [
    ("qsv_motion_60s", 60, ["--mute"], 60, False),
    ("system_audio_tone", 9, [], 9, True),
    ("pause_resume", 4, ["--pause"], 8, True),
    ("microphone_and_system", 4, ["--mic"], 4, True),
    ("cpu_fallback", 4, ["--mute", "--cpu"], 4, False),
]
results = []
for name, seconds, options, expected, tone in cases:
    begin = time.monotonic()
    command = [str(exe), "--smoke-test", str(seconds), "--hwnd", hwnd, *options]
    proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, encoding="utf-8", creationflags=0x08000000)
    lines = []
    output = None
    print(f"CASE {name}", flush=True)
    for line in proc.stdout:
        lines.append(line)
        if line.startswith("Recording") and tone:
            winsound.PlaySound(str(root / "test-output/test-tone.wav"), winsound.SND_FILENAME | winsound.SND_ASYNC)
        if not line.startswith("Progress"):
            print(line.strip(), flush=True)
        if line.startswith("OUTPUT="):
            output = pathlib.Path(line.strip()[7:])
    proc.wait(timeout=30)
    winsound.PlaySound(None, 0)
    if proc.returncode or output is None:
        raise RuntimeError("".join(lines))
    (root / f"test-output/{name}.log").write_text("".join(lines), encoding="utf-8")
    probe = json.loads(subprocess.check_output([str(ffprobe), "-v","error","-count_frames",
        "-show_entries","stream=codec_name,codec_type,width,height,r_frame_rate,nb_read_frames,start_time,duration,sample_rate,channels:format=duration,size",
        "-of","json",str(output)]))
    duration = float(probe["format"]["duration"])
    video = next(s for s in probe["streams"] if s["codec_type"] == "video")
    assert abs(duration - expected) < 0.7, (name, duration, expected)
    assert video["codec_name"] == "h264" and video["r_frame_rate"] == "30/1"
    assert abs(int(video["nb_read_frames"]) - expected*30) < 20
    decode = subprocess.run([str(ffmpeg),"-v","error","-i",str(output),"-f","null","-"],capture_output=True)
    assert decode.returncode == 0 and not decode.stderr.strip(), decode.stderr
    probe["file"] = str(output)
    probe["case"] = name
    probe["decode_ok"] = True
    probe["wall_seconds"] = round(time.monotonic()-begin,2)
    if tone:
        raw = subprocess.check_output([str(ffmpeg),"-v","error","-i",str(output),"-vn","-ac","1","-ar","48000","-f","f32le","pipe:1"])
        samples = array.array("f",raw)
        peak = max(abs(s) for s in samples)
        assert peak > .01, f"Tone missing: {peak}"
        probe["audio_peak"] = peak
    results.append(probe)
    (root / "test-output/acceptance.json").write_text(json.dumps(results,indent=2),encoding="utf-8")
    print(f"PASS {name}: {duration:.3f}s; {video['nb_read_frames']} frames",flush=True)
