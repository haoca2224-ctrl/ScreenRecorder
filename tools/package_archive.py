"""Package application files only. Never include recordings or session data."""
import pathlib
import zipfile

root = pathlib.Path(__file__).resolve().parents[1]
folder = root / "dist" / "ScreenRecorder"
archive = root / "dist" / "ScreenRecorder-portable.zip"
allowed = ["ScreenRecorder.exe", "ffmpeg.exe", "ffprobe.exe", "README.md", "LICENSE",
           "THIRD_PARTY_NOTICES.md", "DESIGN_REFERENCES.md", "VALIDATION.md", "FFMPEG_LICENSE.txt", "FFMPEG_SOURCE.txt", "CONCAT.md"]
for required in ("ScreenRecorder.exe", "ffmpeg.exe", "ffprobe.exe",
                 "concat/concat.exe", "concat/avcodec-62.dll", "concat/UPSTREAM.json", "concat/Concat-0.2.8-source.zip"):
    if not (folder / required).is_file():
        raise SystemExit(f"Incomplete portable bundle: {required}")
with zipfile.ZipFile(archive.with_suffix(".zip.partial"), "w", zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as package:
    for name in allowed:
        path = folder / name
        if path.exists():
            package.write(path, "ScreenRecorder/" + name)
    for directory in ("licenses", "concat", "validation"):
        for path in sorted((folder / directory).rglob("*")):
            if path.is_file() and "editor" not in path.relative_to(folder).parts:
                package.write(path, "ScreenRecorder/" + path.relative_to(folder).as_posix())
archive.with_suffix(".zip.partial").replace(archive)
print(f"Portable archive: {archive} ({archive.stat().st_size / 1024**2:.1f} MiB)")
