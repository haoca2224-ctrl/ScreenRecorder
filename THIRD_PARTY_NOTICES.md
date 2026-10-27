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

## Concat (current bundled editor)

Unmodified Concat 0.2.8, AGPL-3.0-or-later, distributed as a separate executable.
See `concat/LICENSE`, `concat/THIRD_PARTY_NOTICES.md`, `concat/UPSTREAM.json`
and `concat/Concat-0.2.8-source.zip` (exact build commit and build scripts).
https://github.com/jub0t/Concat
The recorder's MIT license does not relicense Concat or its dependencies.

## ScreenRecorder Editor / TaoClip (historical; no longer bundled)

Earlier versions used TaoClip Desktop, copyright (c) 2026 TaoClip,
https://github.com/hosyminh/taoclip-desktop at commit
f28f074b304b9f12294e16f747a26ec8e6f30d87, under MIT.
The obsolete local source/runtime backups have been removed. Its Electron,
Twemoji and JavaScript dependencies are not distributed in the 0.9.0 portable.
Existing user projects are preserved; this is not a project-format migration.

## Cap — tham khảo thiết kế thư viện và giao diện quay (không đóng gói mã)

Đã kiểm tra LICENSE của CapSoftware/Cap ở commit `5fcf8e9cb42625443d31be1023cc816284a7caf5`. Phần thư viện được xem là AGPLv3; chỉ tham khảo cách tổ chức/tương tác. Không có mã nguồn hoặc asset Cap được đưa vào bản phân phối ScreenRecorder. Chi tiết và đường dẫn nguồn nằm trong DESIGN_REFERENCES.md.

Phase 2 đã kiểm tra thêm giao diện desktop và LICENSE ở commit `f6046f1ec82314b0aa887b76bc1ebf8bc77e3579`, cũng chỉ dùng làm tham khảo tương tác; không tái sử dụng mã/asset.
