# Design references

No source code from the reference projects below is copied or bundled. The initial motion-design review was on 20 October 2026 for the legacy Rust editor. The current editor is the separately licensed TaoClip/Electron integration described in THIRD_PARTY_NOTICES.md; the recorder remains Rust/egui.

- React Bits — https://github.com/DavidHDev/react-bits — commit `e12c22192ac7b4b452c5e2feb9b586d74765d696`. Used as a naming and interaction reference for compact animation presets such as Fade, Slide Up, Pop and Typewriter. The repository currently states an MIT + Commons Clause condition.
- Remotion — https://github.com/remotion-dev/remotion — commit `51d0273b743e49a32b9ea3a6256a80e5478d4b2d`. Used as a reference for time-based composition, explicit in/out points and render presets. Remotion is not a runtime dependency; consult its repository for the current license.

The application icon and toolbar symbol sheet in `assets/` are original project assets.


## Thư viện Phase 1 — 26/10/2026

Tham khảo Cap tại commit `5fcf8e9cb42625443d31be1023cc816284a7caf5`:

- [Library/Recents model](https://github.com/CapSoftware/Cap/blob/5fcf8e9cb42625443d31be1023cc816284a7caf5/apps/desktop-gpui/src/library.rs): tách mục media, tên hiển thị, thumbnail và trạng thái.
- [Recordings interactions](https://github.com/CapSoftware/Cap/blob/5fcf8e9cb42625443d31be1023cc816284a7caf5/apps/desktop/src/routes/%28window-chrome%29/settings/recordings.tsx): tìm kiếm/lọc, mở thư mục/editor, cập nhật dữ liệu khi thay đổi.
- [Recording locations](https://github.com/CapSoftware/Cap/blob/5fcf8e9cb42625443d31be1023cc816284a7caf5/apps/desktop/src-tauri/src/recordings_locations.rs): giữ danh sách thư mục cũ khi thay nơi lưu.
- [LICENSE](https://github.com/CapSoftware/Cap/blob/5fcf8e9cb42625443d31be1023cc816284a7caf5/LICENSE): phần này của Cap thuộc AGPLv3; ngoại lệ MIT áp dụng cho các họ crate cap-camera*/scap-* được liệt kê. ScreenRecorder không sao chép/chuyển thể mã hoặc asset Cap. Thiết kế và mã thư viện Rust/egui cùng cầu nối Electron được viết riêng.

ScreenRecorder dùng danh sách ảo cho cả lưới/danh sách; catalogue nằm ngoài media, tên hiển thị không làm đổi file. Phần nền dùng polling 3 giây, hai worker metadata và outbox sự kiện nguyên tử để phù hợp ứng dụng Rust + Electron hiện có.

Recycle Bin sử dụng [IFileOperation flags của Windows](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-setoperationflags), với RECYCLEONDELETE, ADDUNDORECORD và EARLYFAILURE.


## Giao diện quay Phase 2 — 26/10/2026

Đã đọc desktop Cap tại commit `f6046f1ec82314b0aa887b76bc1ebf8bc77e3579`:

- [TargetMenuGrid / TargetCard](https://github.com/CapSoftware/Cap/tree/f6046f1ec82314b0aa887b76bc1ebf8bc77e3579/apps/desktop/src/routes/%28window-chrome%29/new-main): thumbnail nguồn, tên và dấu hiệu lựa chọn; tách cấu hình nguồn với âm thanh và chế độ làm việc.
- Cùng thư mục: `MicrophoneSelect.tsx`, `ModeInfoPanel.tsx` — mức tín hiệu và giải thích hành vi của mỗi luồng.
- [Giấy phép đã kiểm tra](https://github.com/CapSoftware/Cap/blob/f6046f1ec82314b0aa887b76bc1ebf8bc77e3579/LICENSE): phần giao diện thuộc AGPLv3; ngoại lệ MIT áp dụng cho các họ crate được nêu. Không sao chép mã/asset Cap.

ScreenRecorder dùng Rust/egui + Win32 hiện có, giữ TaoClip/Electron. Hai luồng là lưu cục bộ nhanh và lưu rồi mở editor; không thêm tải lên/cloud. Font Segoe UI, nền `#0e1219`, panel `#17202e`, chữ `#eaeff7`, chữ phụ `#9daec3`, màu chọn `#28566e`, accent `#6fc6ed`, nút quay `#da3e4e` được dùng chung với thư viện/editor.
