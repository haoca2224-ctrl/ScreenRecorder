"""Create a small GitHub-ready source archive without generated or downloaded files."""

from __future__ import annotations

import pathlib
import zipfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION = next(
    line.split('"')[1]
    for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines()
    if line.startswith("version = ")
)
OUTPUT = ROOT / "dist" / f"ScreenRecorder-source-v{VERSION}.zip"
ROOT_FILES = {
    ".gitignore",
    "Cargo.lock",
    "Cargo.toml",
    "build.rs",
    "DESIGN_REFERENCES.md",
    "GITHUB_UPLOAD.md",
    "LICENSE",
    "README.md",
    "THIRD_PARTY_NOTICES.md",
    "VALIDATION.md",
}
SOURCE_DIRS = ("assets", "examples", "src", "tools")


def included_files() -> list[pathlib.Path]:
    files = [ROOT / name for name in sorted(ROOT_FILES)]
    for folder in SOURCE_DIRS:
        files.extend(
            path
            for path in sorted((ROOT / folder).rglob("*"))
            if path.is_file() and "__pycache__" not in path.parts and path.suffix != ".pyc"
        )
    return files


OUTPUT.parent.mkdir(parents=True, exist_ok=True)
temporary = OUTPUT.with_suffix(".zip.partial")
with zipfile.ZipFile(
    temporary,
    "w",
    compression=zipfile.ZIP_DEFLATED,
    compresslevel=9,
    strict_timestamps=False,
) as archive:
    for path in included_files():
        archive.write(path, f"ScreenRecorder-source-v{VERSION}/{path.relative_to(ROOT).as_posix()}")
temporary.replace(OUTPUT)
print(f"Source archive: {OUTPUT} ({OUTPUT.stat().st_size / 1024:.1f} KiB)")
