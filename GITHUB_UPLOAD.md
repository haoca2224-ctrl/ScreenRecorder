# Cập nhật repo ScreenRecorder hiện có

Đã kiểm tra ngày 2026-10-28:
- Thư mục: `D:\Khac\ScreenRecorder`.
- Remote `origin`: https://github.com/haoca2224-ctrl/ScreenRecorder.git
- Nhánh mặc định trên GitHub: `main`.
- Nhánh làm việc hiện tại: `codex/taoclip-editor` (tên cũ; mã hiện dùng Concat).
- Chưa commit, push hoặc tạo Release tự động.

## 1. Đưa mã nguồn lên repo hiện tại

Mở PowerShell trong thư mục dự án. Chạy từng nhóm lệnh, kiểm tra kết quả rồi mới
sang bước tiếp theo. Không cần tạo repo mới, không xóa `.git`, không `git init` lại.

```powershell
Set-Location 'D:\Khac\ScreenRecorder'
git status --short
git remote -v
git add -A
git diff --cached --stat
```

Danh sách staged chỉ nên chứa mã nguồn, assets, scripts và tài liệu. Không có
`target`, `vendor`, `dist`, `editor`, video cá nhân hoặc project `.taoclip`.
`.gitignore` đã được cấu hình; không dùng `git add -f` cho các thư mục này.

```powershell
git commit -m "Replace TaoClip with bundled Concat and improve recorder library"
git push -u origin HEAD
```

`HEAD` đẩy nhánh đang làm việc lên **repo hiện có**, không thay lịch sử `main`.
Git có thể mở bước đăng nhập GitHub. Nếu Git yêu cầu tên/email người commit,
thiết lập tên/email Git của chính bạn cho repo này rồi chạy lại lệnh commit.
Nếu push bị từ chối hoặc có xung đột, dừng để xử lý; không dùng `--force`.

Sau khi push, mở:
https://github.com/haoca2224-ctrl/ScreenRecorder/compare/main...codex/taoclip-editor

Chọn **Create pull request**, kiểm tra thay đổi, rồi **Merge pull request** để
cập nhật nhánh `main`. Có thể dùng nút **Compare & pull request** trên trang repo.
Chỉ tạo Release từ `main` sau khi merge thành công.

## 2. Đưa bản chạy được lên Releases

Không commit ZIP/EXE/DLL vào phần mã nguồn. Bản portable mới nhất được giữ ở:

- `dist/ScreenRecorder-portable.zip`
- `dist/ScreenRecorder-portable.zip.sha256`
- `dist/ScreenRecorder-source-v0.9.0.zip` (mã nguồn đã dọn; có thể đính kèm thêm)

Mở https://github.com/haoca2224-ctrl/ScreenRecorder/releases/new
Chọn tag mới `v0.9.0` nếu chưa tồn tại, target `main` sau khi merge,
tiêu đề `ScreenRecorder 0.9.0 — Concat Editor`. Đính kèm ZIP portable, SHA256 và
ZIP mã nguồn; lưu bản nháp để kiểm tra rồi chọn **Publish release**.
Nếu tag/version này đã phát hành, chọn version mới thay vì ghi đè tag cũ.

ZIP portable được giữ nguyên từ bản đã kiểm thử ngày 2026-10-26. Các cập nhật
ngày 2026-10-28 chỉ dọn repo, sửa hướng dẫn/build scripts và bỏ phụ thuộc fixture
TaoClip; không đổi mã Rust đã biên dịch. Giữ nguyên giấy phép, thông báo bên thứ
ba và mã nguồn Concat đóng kèm ZIP. Nhãn MIT của recorder không áp dụng cho Concat.

## 3. Tải lại dependency và dựng ứng dụng

Sau khi dọn hoặc clone repo, cần Windows x64, Rust MSVC, Visual C++ Build Tools /
Windows SDK và Python 3.11 trở lên. Không cần Node.js hoặc Electron.

```powershell
Set-Location 'D:\Khac\ScreenRecorder'
powershell -ExecutionPolicy Bypass -File tools\package.ps1
```

Script tự tải FFmpeg khi thiếu, kiểm tra checksum, tải/giải nén Concat 0.2.8
nguyên bản, dựng Rust và tạo lại `dist/ScreenRecorder/ScreenRecorder.exe` cùng ZIP.
Không cài Concat vào hệ thống. Lần đầu cần Internet và thời gian biên dịch.

Để chạy phát triển và kiểm tra:

```powershell
python -X utf8 tools/fetch_ffmpeg.py
python -X utf8 tools/prepare_concat.py
cargo test --locked
cargo clippy --locked -- -D warnings
cargo build --locked
python -X utf8 tools/check_concat.py
cargo run
```

Tạo lại ZIP mã nguồn nhỏ, không kèm dependency:

```powershell
python -X utf8 tools/package_source.py
```

## Những gì giữ / loại khỏi Git

Giữ `src`, `assets`, `examples`, `tools`, `Cargo.toml`, **Cargo.lock**, `build.rs`,
`.gitignore`, README, giấy phép và tài liệu. Giữ `.git` để bảo toàn lịch sử và remote.

Không commit cache/build (`target`), dependency tải lại (`vendor`), bản phân phối
(`dist`), dữ liệu test (`test-output`, `editor/.testout`), video (`recordings`),
project cá nhân, bản dự phòng, `.env` hoặc log. Các file này có thể còn ở máy nhưng
đã bị loại khỏi Git. Không dùng `git clean -fdx`: lệnh đó có thể xóa cả video/project
đang được bảo vệ bằng `.gitignore`.

Các project tại `%LOCALAPPDATA%/ScreenRecorder` và recordings hiện có không bị dọn.
Không dùng ZIP mã nguồn để ghi đè/xóa cả repo cũ qua web; Git sẽ cập nhật đúng từng file.

Tài liệu GitHub chính thức:
- https://docs.github.com/en/get-started/using-git/pushing-commits-to-a-remote-repository
- https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository
