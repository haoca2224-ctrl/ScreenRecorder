# Screen Recorder

Trình quay màn hình và chỉnh sửa clip Windows bằng Rust, egui, Windows Graphics Capture, WASAPI và FFmpeg.

## Chạy

Trong thư mục mã nguồn, mở `dist\ScreenRecorder\ScreenRecorder.exe`.
Nếu dùng ZIP portable, giải nén rồi mở `ScreenRecorder.exe` trong thư mục `ScreenRecorder`.
Giữ `ffmpeg.exe` và `ffprobe.exe` bên cạnh ứng dụng.
Không cần cài Rust hoặc FFmpeg trên máy sử dụng.

1. Trong tab **Ghi hình**, chọn **Vùng chọn → + Chọn vùng**. Kéo chuột rồi nhấn **Enter**; **Esc** hủy chọn.
2. Khung nổi xuất hiện. Kéo cạnh/góc để đổi kích thước, kéo phần viền trên để di chuyển. Có thể nhập rộng/cao ở cửa sổ chính. Khung được khóa khi bắt đầu phiên ghi.
3. Chọn **Âm thanh máy**, **Micro** và thiết bị muốn dùng. Thanh mức âm giúp kiểm tra tín hiệu trước khi quay.
4. Chọn **Nhẹ / Cân bằng / Rõ chữ**, thư mục lưu, rồi **Bắt đầu ghi**. Đếm ngược 3 giây; cửa sổ chính tự thu nhỏ.
5. Dùng toolbar nổi để **Tạm dừng / Tiếp tục / Dừng**, bật/tắt micro hoặc tiếng máy, chụp ảnh PNG, mở công cụ **Vẽ**, hoặc trở về ứng dụng.

Ở chế độ **Ứng dụng**, danh sách cửa sổ đang mở hiển thị tên tiến trình, tiêu đề, kích thước và PID; có ô tìm kiếm và nút quét lại. Nhấp một hàng để chọn đúng cửa sổ cần ghi.

Toolbar kéo được bằng vùng sáu chấm bên trái. **Gọn** thu về thời gian và các nút cần thiết; **Mở** trở lại thanh đầy đủ.
Toolbar và viền của ứng dụng được loại khỏi video/ảnh chụp. Chú thích được ghép vào khung hình trước khi mã hóa.
Windows vẫn có thể hiển thị viền hệ thống quanh nguồn ghi.

Trước khi ghi, nhấn **Esc** khi đang ở công cụ Thao tác hoặc bấm nút **X** để đóng toàn bộ khung/toolbar và đổi nguồn. Nếu đang dùng công cụ vẽ, lần Esc đầu trở về Thao tác, lần tiếp theo đóng khung. Trong khi đang ghi, Esc chỉ thoát công cụ vẽ để tránh dừng nhầm phiên.

### Chú thích trong lúc quay

- **Thao tác**: chuột điều khiển phần mềm bên dưới.
- **Bút / Đánh dấu / Mũi tên / Khung / Tròn**: kéo trực tiếp trong vùng quay.
- **Chữ**: bấm ô **Chữ: …**, nhập nội dung, nhấn Enter, chọn công cụ Chữ rồi bấm lên vùng quay.
- **Số**: bấm để đặt các số thứ tự. Có bảng màu và lựa chọn độ dày nét.
- **Undo / Redo / Xóa**: hoàn tác, làm lại hoặc xóa toàn bộ chú thích.
- **Esc** khi khung/toolbar đang nhận bàn phím: trở về Thao tác. **Ctrl+Z / Ctrl+Y** hoàn tác/làm lại.
- **Tỷ lệ** trong thanh Vẽ: tự do, 16:9, 4:3; áp dụng khi kéo đổi kích thước khung trước khi quay.
- Bật **Nổi con trỏ** và **Hiệu ứng click** ở cửa sổ chính trước khi ghi nếu cần hướng dẫn thao tác.

Chú thích đã xuất là một phần của ảnh/video, không phải lớp chỉnh sửa độc lập sau khi lưu.
**Chụp ảnh** lưu PNG ở kích thước nguồn, gồm chú thích; không dừng phiên quay.

Có thêm chế độ **Toàn màn hình** và **Cửa sổ**. Vùng chọn thuộc một màn hình.
Không đổi kích thước vùng trong một phiên, kể cả khi tạm dừng; hãy dừng rồi chọn lại để xuất một video mới.
Khi tạm dừng, ứng dụng hoàn thiện đoạn hiện tại rồi mới cho tiếp tục.
Khi đóng cửa sổ chính trong lúc ghi, ứng dụng dừng và lưu xong trước khi thoát.

**Ctrl+Shift+F9**: bắt đầu/dừng; **Ctrl+Shift+F10**: tạm dừng/tiếp tục.
Tab **Thư viện** mở các video/ảnh đã lưu. Tab **Cài đặt** chứa thông số nâng cao và phục hồi phiên.

### Chỉnh sửa sau khi ghi

Tab **Chỉnh sửa** dùng bố cục bàn dựng ba cột: Media/Text/Effects bên trái, Player ở giữa, thuộc tính và Export bên phải, timeline nhiều lớp ở dưới. Editor mở MP4, MKV, WebM hoặc MOV mà không nạp toàn bộ video vào RAM.

- Ở tab Media, kéo trực tiếp trên preview để chọn vùng crop. Hai bảng bên và timeline có thể ẩn/hiện; nội dung bảng có thanh cuộn nên dùng được trên màn hình 1280×720.
- Kéo playhead rồi dùng **Tách**, **Cắt trước** hoặc **Cắt sau**. Mỗi đoạn sau khi tách là một clip thật: bấm để chọn, xóa, đưa sang trái/phải và đổi tốc độ 0,25×–4×. Hình và âm thanh được đổi tốc độ đồng bộ khi xuất.
- Timeline hiển thị waveform của track âm thanh bằng proxy nhẹ tạo nền; video không có audio vẫn chỉnh sửa bình thường.
- Thêm nhiều lớp chữ với preset **Tối giản, Đậm, Viền, Neon, Nhãn, Bóng**. Chọn chữ ngay trên Player để kéo vị trí; tay nắm tròn nằm ngoài khung chữ dùng để phóng/thu.
- Chữ bắt dính vào tâm và safe-zone 5%, kèm đường guide. Có thể tắt **Bắt dính** trên Player.
- Editor có Undo/Redo riêng bằng nút trên thanh đầu hoặc **Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z**; không ảnh hưởng Undo/Redo của toolbar chú thích.
- Inspector chữ có tab **Cơ bản** và **Animation**; Animation chia riêng **In / Out** với các preset Tĩnh, Fade, Slide Up, Pop, Typewriter.
- Preset video: **Không hiệu ứng, Soft Fade, Zoom chậm**.
- Xuất **kích thước gốc, 720p, 1080p, vuông 1080×1080 hoặc dọc 1080×1920**.
- Lưu **MP4/H.264, MKV/H.264 hoặc WebM/VP9**; file gốc không bị thay đổi.

Các ý tưởng tổ chức preset được tham khảo ở mức thiết kế từ React Bits và Remotion; ứng dụng không nhúng React/Node. Xem `DESIGN_REFERENCES.md`.

Giao diện có thể đổi giữa **Tiếng Việt** và **English** ở thanh trên hoặc tab **Cài đặt**. Lựa chọn được lưu và áp dụng cho toolbar nổi cùng màn hình chọn vùng.

Toolbar dùng biểu tượng nhất quán cho cả nhóm chú thích, Undo/Redo/Xóa, có nhãn trợ giúp khi rê chuột và luôn bị loại khỏi video. Asset gốc nằm trong `assets/toolbar-symbols.svg` và `assets/editor-symbols.svg`; icon ứng dụng có các cỡ 16, 24, 32, 48, 64, 96 và 128 px, đồng thời được nhúng vào EXE Windows.

## Cấu hình mặc định

- 30 fps, H.264 8 Mbps; AAC stereo 48 kHz / 192 kbps khi bật âm thanh.
- Giữ kích thước vùng chọn. Có tùy chọn thu nhỏ tối đa 1080p hoặc 720p, không phóng to ảnh.
- Ưu tiên Intel Quick Sync; tự dùng CPU nếu phép thử khởi tạo QSV thất bại.
- Khi quay **Toàn màn hình/Vùng chọn** ở kích thước gốc với QSV, FFmpeg nhận trực tiếp texture D3D11 từ Desktop Duplication, ánh xạ sang video memory QSV và chuyển sang bề mặt NV12 trong bộ mã hóa. Dữ liệu BGRA không đi qua stdin hay bị sao chép từng frame vào RAM.
- Chế độ **Ứng dụng**, lựa chọn CPU và trường hợp cần thu nhỏ độ phân giải dùng đường WGC dự phòng để giữ đúng hành vi quay cửa sổ. Đường này dùng pool tối đa ba buffer và một buffer ghép chú thích tái sử dụng.
- Thu tiếng máy và con trỏ; micro tắt sẵn.
- MP4 được đặt tên theo ngày, giờ và mã phiên, tránh ghi đè file.
- Cài đặt lưu tại `%LOCALAPPDATA%\ScreenRecorder\settings.json`.
- Thư mục video mặc định là `recordings` cạnh ứng dụng; có thể đổi trên giao diện.

## Lưu tạm và phục hồi

Mỗi phiên có thư mục riêng trong `<thư mục video>\.sessions`.
Video được mã hóa trực tiếp ra MKV. Âm thanh được ghi PCM theo mốc thời gian QPC của Windows,
sau đó trộn và nén AAC khi hoàn thiện từng đoạn. RAM không tăng theo độ dài phiên ghi.
Các buffer khung hình có kích thước cố định và được tái sử dụng; lớp chú thích/hiệu ứng con trỏ cũng ghép vào buffer giữ lại giữa các frame. Với BGRA 1920×1080, mỗi bản sao tránh được tương đương khoảng 7,9 MiB.
PCM stereo 48 kHz cần khoảng **1,38 GB/giờ cho mỗi nguồn âm thanh**.
Trong lúc xuất cần thêm dung lượng cho video tạm và MP4. Chuẩn bị khoảng 12–15 GB trống
cho một giờ ở 8 Mbps và hai nguồn âm thanh.

Xuất thành công và xác thực được MP4 thì dữ liệu tạm của phiên được dọn.
Nếu xuất lỗi, ứng dụng giữ thư mục phiên và báo đường dẫn. Có thể phục hồi từ giao diện
hoặc chạy từ PowerShell:

```powershell
.\ScreenRecorder.exe --recover "D:\DuongDanVideo\.sessions\Quay_..."
```

Phục hồi cố gắng giữ những khung đã ghi xuống đĩa; không thể bảo đảm cứu phần đang nằm
trong bộ đệm khi mất điện hoặc tiến trình bị dừng cưỡng bức.

## Giới hạn

- Windows 10 22H2 / Windows 11, bản x64. Đã phát triển trên i7-8700, UHD 630, RAM 32 GB.
- Khi quay cửa sổ, giữ cửa sổ nguồn mở, không thu nhỏ và không đổi kích thước.
  Nếu nguồn đóng hoặc đổi kích thước, ứng dụng lưu phần đã ghi và thông báo.
- Quay âm thanh toàn bộ thiết bị phát đã chọn, chưa tách âm thanh riêng từng ứng dụng.
- Đổi/rút thiết bị âm thanh trong lúc ghi có thể kết thúc phiên; nên chọn thiết bị trước.
- Nội dung được ứng dụng nguồn bảo vệ có thể không ghi được.
- Bản đầu tiên xuất SDR/H.264, chưa xử lý HDR hay ghép nhiều màn hình.
- Nếu máy báo không mã hóa kịp, chọn 720p, giảm vùng quay hoặc đóng tác vụ nặng.
- Tự chuyển CPU chỉ ở bước khởi tạo; lỗi GPU xảy ra giữa phiên sẽ được báo và giữ dữ liệu phục hồi.

## Biên dịch

Cần Rust MSVC x64 và Visual C++ Build Tools / Windows SDK.

```powershell
python tools\fetch_ffmpeg.py
cargo test --locked
cargo clippy --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File tools\package.ps1
```

`Cargo.lock` cố định dependency. Script tải FFmpeg kiểm tra SHA-256 do nhà phát hành cung cấp.
Nguồn tải và checksum được lưu trong `vendor\SOURCE.txt`.

## Kiểm tra

```powershell
cargo run -- --smoke-test 10 --pause
cargo run -- --smoke-test 5 --mic
cargo run -- --smoke-test 5 --mute --cpu
cargo run -- --test-pattern
cargo run -- --editor-smoke path\to\input.mp4 test-output\editor-smoke.mp4
cargo run --example check_region
python tools\check_studio.py
```

Phép thử ghi tạo file tại `test-output`. Để kiểm tra một cửa sổ cụ thể, thêm `--hwnd <handle>`.
Kết quả xác thực bản giao nằm trong `VALIDATION.md`.

## Mã nguồn

- `src/app.rs`: giao diện và trạng thái điều khiển.
- `src/native.rs`: chọn vùng theo pixel vật lý, danh sách nguồn và phím tắt.
- `src/overlay.rs`: khung/toolbar Win32, tương tác và loại trừ khỏi capture.
- `src/studio.rs`: chú thích, ghép alpha, hiệu ứng chuột và xuất PNG.
- `src/capture.rs`: Windows Graphics Capture, cắt vùng, bộ đệm khung hình giới hạn.
- `src/audio.rs`: WASAPI, thu loopback/micro, giữ thời gian và khoảng im lặng.
- `src/editor.rs`: preview theo frame, crop/trim, lớp chữ động, preset và xuất đa định dạng.
- `src/recorder.rs`: thử encoder, ghi, tạm dừng, xuất và phục hồi.

Mã ứng dụng: MIT. FFmpeg đi kèm là chương trình riêng có giấy phép GPL v3;
xem `THIRD_PARTY_NOTICES.md` và các thông báo giấy phép trong bộ portable.
