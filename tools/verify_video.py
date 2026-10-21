"""Inspect a recording's streams, frame count, decode errors and audio signal."""
import array
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[1]
ffmpeg = root / "vendor" / "ffmpeg.exe"
ffprobe = root / "vendor" / "ffprobe.exe"
for arg in sys.argv[1:]:
    path = pathlib.Path(arg)
    report = json.loads(subprocess.check_output([
        str(ffprobe), "-v", "error", "-count_frames", "-show_entries",
        "stream=codec_name,codec_type,width,height,r_frame_rate,nb_read_frames,start_time,duration,sample_rate,channels:format=duration,size",
        "-of", "json", str(path)]))
    decode = subprocess.run([str(ffmpeg), "-v", "error", "-i", str(path), "-f", "null", "-"], capture_output=True)
    report["file"] = str(path)
    report["decode_ok"] = decode.returncode == 0 and not decode.stderr.strip()
    report["decode_errors"] = decode.stderr.decode(errors="replace")
    if any(s["codec_type"] == "audio" for s in report["streams"]):
        raw = subprocess.check_output([str(ffmpeg), "-v", "error", "-i", str(path),
            "-vn", "-f", "f32le", "-ac", "1", "-ar", "48000", "pipe:1"])
        samples = array.array("f", raw)
        report["audio_peak"] = max((abs(s) for s in samples), default=0)
        report["audio_rms"] = (sum(s*s for s in samples)/max(1,len(samples)))**0.5
    print(json.dumps(report, ensure_ascii=False, indent=2))
