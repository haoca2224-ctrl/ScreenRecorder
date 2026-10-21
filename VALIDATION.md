# Kiểm chứng bản 0.6.0 — Editable clips, waveform, speed & text transform

Ngày 20/10/2026, trên Windows 10 Pro 19045 / màn hình 1280×720.

- Split tạo các clip có `source_start`, `source_end` và speed độc lập. Timeline cho chọn, xóa, đổi thứ tự bằng nút trái/phải và tăng/giảm tốc độ từ 0,25× đến 4×.
- Export dựng lại thứ tự bằng FFmpeg `split/asplit + trim/atrim + concat`; `setpts` đổi tốc độ hình và chuỗi `atempo` đổi tốc độ audio tương ứng.
- Waveform được lấy nền ở mono 400 Hz, gom còn tối đa khoảng 6.000 peak và vẽ theo đúng khoảng nguồn của từng clip, kể cả sau khi đổi thứ tự.
- Text transform tách vùng kéo khỏi tay nắm scale; kích thước preview quy đổi theo preset đầu ra. Chữ snap vào tâm/safe-zone 5% và hiển thị guide tại đúng điểm snap.
- Undo/Redo riêng lưu crop, text layers, clip order/speed và video preset; hỗ trợ nút giao diện, Ctrl+Z, Ctrl+Y và Ctrl+Shift+Z.
- Smoke export với hai clip đảo thứ tự ở 2× và 0,5×: MP4 1920×1080, video 3,133 giây, AAC 3,116 giây; giải mã toàn bộ không lỗi. Nhánh video không có audio cũng xuất và giải mã thành công.
- `cargo test --locked`: 12 kiểm tra đạt; `cargo clippy --locked --all-targets -- -D warnings`: đạt.

---

# Kiểm chứng bản 0.5.0 — D3D11/QSV zero-copy & reusable buffers

Ngày 20/10/2026, trên Windows 10 Pro 19045 / màn hình 1280×720 / Intel UHD 630.

- Quay Toàn màn hình/Vùng chọn ở kích thước gốc dùng `ddagrab` để giữ frame trong D3D11 texture, `hwmap` sang QSV video memory và mã hóa `h264_qsv`; stdin FFmpeg chỉ nhận lệnh kết thúc, không nhận BGRA.
- Probe ba frame chạy trước phiên. Nếu D3D11/QSV không khởi tạo được, ứng dụng tự trở về pipeline WGC hiện có.
- Quay cửa sổ, CPU và tác vụ cần downscale tiếp tục dùng WGC để giữ đúng ngữ nghĩa nguồn. Frame WGC lấy từ pool giới hạn ba buffer; buffer ghép chú thích và buffer hiệu ứng được tái sử dụng.
- Bài ghi GPU 1280×720, 30 fps, 5,000 giây xuất H.264/YUV420, giải mã toàn bộ không lỗi.
- Bài Studio hai đoạn pause/resume xuất 920×540, 7,833 giây. Frame kiểm tra chứa chữ tiếng Việt, highlight, khung và mũi tên; toolbar không xuất hiện trong video.
- Pipeline CPU dự phòng `libx264` ghi 3 giây và hoàn thiện MP4 thành công.
- Đo một lần trên bản release trong bài ghi 8 giây: tiến trình ứng dụng đạt đỉnh khoảng **8,0 MiB** ở đường GPU so với **34,8 MiB** ở đường CPU/WGC; tiến trình FFmpeg lần lượt khoảng **77,7 MiB** và **100,1 MiB**. Đây là số đo tham khảo ngắn trên máy thử, không phải cam kết cho mọi nguồn hay driver.
- `cargo test --locked`: 10 kiểm tra đạt, gồm hai kiểm tra mới về tái sử dụng buffer và giới hạn pool; `cargo clippy --locked --all-targets -- -D warnings`: đạt.

## Phạm vi đường GPU

Đường zero-copy hiện áp dụng cho nguồn màn hình/vùng chọn, QSV và không yêu cầu thay đổi kích thước. Driver UHD 630 trên máy thử không nhận filter `scale_d3d11=format=nv12` riêng; chuyển đổi RGB texture sang bề mặt mã hóa NV12/YUV420 được QSV thực hiện bên trong video memory. Vì vậy tùy chọn 720p/1080p chỉ dùng đường GPU khi nguồn không lớn hơn giới hạn; trường hợp cần thu nhỏ dùng pipeline dự phòng.

---

# Kiểm chứng bản 0.4.0 — Editor 720p, direct text transform & clip split

Ngày 20/10/2026, trên Windows 10 Pro 19045 / màn hình 1280×720.

- Kích thước cửa sổ mặc định giảm còn 1120×650, giới hạn tối thiểu 900×540. Editor tự thu gọn chiều cao Player/timeline, cho ẩn hiện Browser/Inspector/Timeline và cuộn độc lập hai bảng bên.
- Chữ có sáu preset giao diện, chọn/kéo trực tiếp trong Player và đổi cỡ bằng tay nắm góc; thuộc tính vẫn có X/Y/cỡ chữ chính xác.
- Animation chữ có tab In/Out riêng; file ASS xuất kết hợp style cùng animation vào và ra.
- Timeline có Tách, Cắt trước và Cắt sau tại playhead; điểm tách được lưu và vẽ thành các đoạn riêng.
- Toolbar annotation dùng icon cho đủ Pointer, Pen, Highlight, Arrow, Rectangle, Ellipse, Text, Number, Undo, Redo và Clear.
- Bổ sung asset vector `assets/editor-symbols.svg`; toàn bộ nguồn asset toolbar/editor có tên action rõ ràng.
- `cargo test --locked`: 8 kiểm tra đạt; `cargo clippy --locked --all-targets -- -D warnings`: đạt; `--toolbar-dismiss-test`: `TOOLBAR_DISMISS=OK`.
- Editor smoke export thành công MP4 H.264 1920×1080 / 2,5 giây; frame kiểm tra có text Outline + Slide Up và file giải mã được.

---

# Kiểm chứng bản 0.3.1 — Toolbar dismissal & editing workspace

Ngày 20/10/2026, trên Windows 10 Pro 19045.

- Thêm action `Dismiss`: khi trạng thái Ready và công cụ Pointer, Esc đóng toàn bộ frame/toolbar; nút X có mặt ở toolbar đầy đủ và thu gọn.
- Bài tích hợp Win32 `--toolbar-dismiss-test` gửi phím Esc vào đúng cửa sổ toolbar và nhận `TOOLBAR_DISMISS=OK`.
- Toolbar mới được render lại trong `test-output/toolbar-1.png`; nút đóng hiển thị đúng và không làm tràn chiều rộng.
- Editor chuyển sang workspace ba cột kiểu bàn dựng: trình duyệt Media/Text/Effects, Player, Inspector/Export và timeline ngang có thumbnail, trim bounds, text tracks, playhead kéo được, Set In/Out và preview controls.
- `cargo test --locked`: 7 kiểm tra đạt. `cargo clippy --locked --all-targets -- -D warnings`: đạt.

---

# Kiểm chứng bản 0.3.0 — Source picker, editor, localization & assets

Ngày 20/10/2026, trên cùng máy Windows 10 Pro 19045 / i7-8700 / UHD 630.

- `cargo test --locked`: 7 kiểm tra đạt, gồm hình học crop và sinh lớp chữ animation ASS.
- `cargo clippy --locked --all-targets -- -D warnings`: đạt.
- Danh sách nguồn cửa sổ đọc tên tiến trình, tiêu đề, PID và kích thước; giao diện có tìm kiếm và refresh.
- Editor smoke test crop 5% mỗi cạnh, trim 2,5 giây, thêm chữ Slide Up, Soft Fade và upscale Full HD.
- Xuất và giải mã toàn bộ thành công: MP4/H.264 + AAC, MKV/H.264 + AAC, WebM/VP9 + Opus; cả ba đúng 1920×1080 và khoảng 2,533 giây.
- Ảnh frame kiểm tra `test-output/editor-smoke-frame.png` cho thấy crop/pad và chữ đã được render vào video.
- Toolbar mới render được biểu tượng Segoe MDL2, trạng thái ghi, audio meters và dòng tooltip; ảnh `test-output/toolbar-1.png` được tạo từ chính bộ render Win32.
- Icon ứng dụng được tạo ở 16/24/32/48/64/96/128 px và nhúng vào tài nguyên EXE qua `winres`.

## Giới hạn kiểm chứng 0.3.0

Chưa kiểm tra tương tác kéo crop tự động bằng chuột, clip dài hoặc tốc độ xuất VP9 trên video dài. Editor hiện là dựng một clip, chưa có nhiều track video/audio, keyframe tùy ý hay caption tự động.

---

# Kiểm chứng bản 0.2.0 — Studio

Ngày 20/10/2026, cùng máy Windows 10 Pro 19045 / i7-8700 / UHD 630 / màn hình 1280×720.

- `cargo test --locked`: 5 kiểm tra đạt (chọn vùng, tám hướng resize/di chuyển với tọa độ âm, tỷ lệ video, alpha/chú thích, undo/redo).
- `cargo clippy --locked --all-targets -- -D warnings`: đạt.
- Bài thử phần cứng `tools/check_studio.py`: toolbar hiển thị **bên trong vùng ghi**, API display affinity trả `WDA_EXCLUDEFROMCAPTURE`. Vùng pixel của toolbar trong hình WGC khớp hoàn toàn với nền bên dưới: **0 pixel sai khác**.
- Cùng bài thử: chữ tiếng Việt, khung, bút đánh dấu và mũi tên được đưa vào PNG và MP4; ảnh thô không chứa các lớp này.
- MP4 hai đoạn sau pause/resume: **9,1085 giây, 272 khung, 30 fps**; giải mã toàn bộ không lỗi.
- Tắt/bật âm thanh trong phiên: đỉnh tín hiệu trước tắt **0,05339**, trong đoạn tắt **0**, sau bật **0,05373**. Micro được khởi tạo giữa phiên, sau 5 giây.
- Đã xem ảnh giao diện chính, toolbar đầy đủ, thanh chú thích và bản thu gọn từ chính bộ render của ứng dụng; ảnh nằm trong `test-output`.
- Đã sửa đường hiển thị lớp nổi cho Windows 10: lớp per-pixel `UpdateLayeredWindow` trả lỗi khi gắn display affinity trên máy này. Bản giao dùng color-key layered window và ghép nét bán trong suốt với nền sạch; API loại trừ và kết quả hình thu đã được xác nhận.

Dữ liệu bài kiểm tra: `test-output/studio-v2/acceptance.json` và `video-annotations.png`.
Bài so sánh pixel chỉ xét hình chữ nhật toolbar để không nhầm con trỏ chỉ báo của công cụ điều khiển máy tính với giao diện ứng dụng.
Các kiểm tra cũ bên dưới thuộc 0.1.0, không được tính là đã chạy lại trên 0.2.0.

## Giới hạn kiểm chứng

Chưa có thử 15–30 phút, nhiều màn hình/DPI hỗn hợp hoặc đo đồng bộ giọng nói bằng thiết bị đo.
Công cụ thao tác Windows vẫn trả `window capture timed out` / `coordinate input geometry is unavailable` cho ảnh chụp và click; chưa xác nhận tự động toàn bộ thao tác kéo chuột thực tế. Hình học và pipeline ghi được kiểm tra trực tiếp qua bài tích hợp phần cứng.

---

# Kiểm chứng bản 0.1.0

Ngày: 20/10/2026, máy hiện tại: Windows 10 Pro 19045 x64, Intel i7-8700,
UHD Graphics 630, RAM khoảng 32 GB; màn hình được API nhận là 1280×720.

## Đã thực hiện

| Kiểm tra | Kết quả |
|---|---|
| `cargo fmt --check` | Đạt |
| `cargo test --locked` | 2 kiểm tra hình học/toàn vẹn tỉ lệ đạt |
| `cargo clippy --locked --all-targets -- -D warnings` | Đạt, không cảnh báo |
| `cargo build --release --locked` | Đạt |
| QSV với cửa sổ mẫu chuyển động, mục tiêu 60 giây | MP4 60,100 giây, 1.803 khung, 30 fps, giải mã toàn bộ không lỗi |
| Tiếng máy với tín hiệu kiểm tra | H.264 + AAC stereo 48 kHz, 9,088 giây, có tín hiệu âm thanh |
| Tạm dừng/tiếp tục, 2 đoạn khoảng 4 giây | MP4 8,175 giây, 244 khung, âm thanh liên tục theo các đoạn, giải mã không lỗi |
| Bật micro và tiếng máy | Cả hai thiết bị khởi tạo được; file AAC có tín hiệu và giải mã không lỗi |
| Mã hóa CPU `libx264` | MP4 4,100 giây, 123 khung, 30 fps, không lỗi giải mã |
| Vùng màn hình có tọa độ lệch (100,80), kích thước 640×360 | File xuất đúng 640×360 |
| Phiên qua giao diện được quan sát trực tiếp | Vùng chọn hiển thị X=22, Y=131, 812×502; file xuất 812×502 / 30 fps, 8,721 giây, H.264 + AAC |
| Phục hồi phiên bị gián đoạn trong lúc kiểm tra | Phục hồi được MP4 18 giây / 540 khung; giải mã không lỗi |
| Kiểm tra hình ảnh giao diện từ khung video | Chữ tiếng Việt, giao diện tối; điều khiển ghi/dừng nằm trong cửa sổ trên màn hình hiện tại |
| Thư mục tạm | Phiên xuất thành công được dọn; phiên phục hồi còn được giữ |

Sai lệch vài chục mili giây giữa thời lượng video/âm thanh là phần đệm AAC.
Thời lượng 60,100 giây bao gồm độ trễ gửi lệnh dừng từ bộ kiểm thử.
Chi tiết các file thử nằm trong `test-output/acceptance.json` ở thư mục mã nguồn.

Trong kiểm tra dài, đã phát hiện và sửa phép trừ `Duration` tại ranh giới khung hình;
vòng kiểm tra 60 giây được chạy lại thành công sau sửa.
Đã sửa thời điểm khởi động đồng hồ: chuẩn bị đường truyền FFmpeg trước khi bắt đầu
đồng hồ âm thanh/video, để việc nạp bộ mã hóa không làm ngắn đầu bản ghi.

## Phạm vi chưa xác nhận

- Chưa thử phiên liên tục 15–30 phút hoặc nhiều giờ.
- Chưa đo định lượng mức dùng CPU/GPU/RAM khi vừa quay vừa chạy tác vụ nặng.
- Chưa thử nhiều màn hình với DPI khác nhau, HDR hoặc nội dung được bảo vệ.
- Chưa nghe đánh giá chất lượng giọng nói thực tế của micro hoặc kiểm tra đồng bộ tiếng/hình bằng thiết bị đo.
- Công cụ điều khiển Windows trả lỗi `window capture timed out` và `coordinate input geometry is unavailable`;
  không hoàn tất kịch bản kéo-thả chuột tự động qua công cụ này.
  Kiểm chứng vùng ghi dựa trên phép thử backend và file của phiên GUI quan sát được.

Các video thử và video trong `recordings` không nằm trong ZIP portable.
