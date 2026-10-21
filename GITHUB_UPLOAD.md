# Danh mục đưa lên GitHub

## Cần commit

- `src/`: toàn bộ mã nguồn Rust.
- `assets/`: icon ứng dụng và asset SVG của toolbar/editor.
- `examples/`: các bài kiểm tra tích hợp phần cứng.
- `tools/`: script tải FFmpeg, kiểm tra, thu thập license và đóng gói.
- `.gitignore`, `Cargo.toml`, `Cargo.lock`, `build.rs`.
- `README.md`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `DESIGN_REFERENCES.md`, `VALIDATION.md`.
- `GITHUB_UPLOAD.md` và `tools/package_source.py` để tái tạo gói mã nguồn.

## Không commit

- `target/`: sản phẩm biên dịch và cache Cargo; Cargo tự tạo lại.
- `vendor/`: FFmpeg/FFprobe và gói tải; chạy `python tools/fetch_ffmpeg.py` để lấy lại và kiểm tra SHA-256.
- `dist/`: EXE, license bundle và ZIP portable/source được tạo khi đóng gói.
- `test-output/`: video, ảnh, waveform và dữ liệu kiểm thử.
- `recordings/`: video của người dùng.
- `backups/`: bản sao cục bộ của mã cũ.
- `build.log`, `check.log` và các file `*.log`, `*.zip` ở thư mục gốc.
- `tools/__pycache__/`: cache Python.

## Chuẩn bị sau khi clone

```powershell
python tools\fetch_ffmpeg.py
cargo test --locked
cargo run --release
```

Tạo lại ZIP mã nguồn bằng:

```powershell
python tools\package_source.py
```
