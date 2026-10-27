"""Download the Windows FFmpeg package and verify its publisher SHA-256."""
import hashlib
import pathlib
import urllib.request
import zipfile

root = pathlib.Path(__file__).resolve().parents[1]
vendor = root / "vendor"
vendor.mkdir(exist_ok=True)
url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
archive = vendor / "ffmpeg.zip"
checksum = urllib.request.urlopen(url + ".sha256", timeout=60).read().decode().split()[0]
if not archive.exists() or hashlib.file_digest(archive.open("rb"), "sha256").hexdigest() != checksum:
    urllib.request.urlretrieve(url, archive)
actual = hashlib.file_digest(archive.open("rb"), "sha256").hexdigest()
if actual != checksum:
    raise SystemExit("FFmpeg SHA-256 mismatch")
with zipfile.ZipFile(archive) as package:
    for name in package.namelist():
        leaf = pathlib.PurePosixPath(name).name
        if leaf in ("ffmpeg.exe", "ffprobe.exe", "LICENSE", "LICENSE.txt"):
            destination = "LICENSE" if leaf in ("LICENSE", "LICENSE.txt") else leaf
            (vendor / destination).write_bytes(package.read(name))
(vendor / "SOURCE.txt").write_text(f"URL: {url}\nSHA256: {actual}\n", encoding="utf-8")
print(f"Verified FFmpeg: {actual}")
