# Third-party software

The application communicates with FFmpeg/FFprobe through separate child processes.

- FFmpeg / FFprobe: https://ffmpeg.org/
- Windows binary distributor: https://www.gyan.dev/ffmpeg/builds/
- Downloaded build: 9.0.2 essentials (see `FFMPEG_SOURCE.txt` and `ffmpeg -version`).
- This build enables GPL and version 3. Its license is supplied as `FFMPEG_LICENSE.txt`.
- Distributor's build/source information: https://github.com/GyanD/codexffmpeg
- FFmpeg source: https://git.ffmpeg.org/ffmpeg.git

Rust dependencies are recorded in Cargo.lock. Major libraries include:

- egui / eframe: MIT OR Apache-2.0 — https://github.com/emilk/egui
- windows-capture: MIT — https://github.com/NiiightmareXD/windows-capture
- windows-rs: MIT OR Apache-2.0 — https://github.com/microsoft/windows-rs
- rfd: MIT — https://github.com/PolyMeilex/rfd
- serde / serde_json: MIT OR Apache-2.0 — https://github.com/serde-rs/serde
- chrono: MIT OR Apache-2.0 — https://github.com/chronotope/chrono
- anyhow: MIT OR Apache-2.0 — https://github.com/dtolnay/anyhow

The Windows Segoe UI font is read from the local Windows installation at runtime;
it is not copied into this package. egui's bundled fallback font is used if unavailable.
Dependency license texts and package metadata are included under `licenses`.
