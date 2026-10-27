# Screen Recorder

Trình quay màn hình và chỉnh sửa clip Windows bằng Rust, egui, Windows Graphics Capture, WASAPI và FFmpeg.

## Editor Concat — 0.9.0

Editor mặc định đã thay bằng Concat 0.2.8. Bấm **Chỉnh sửa** tự tạo timeline đúng
kích thước/FPS; **Ctrl+S** lưu, lần mở sau giữ nguyên chỉnh sửa. Giải nén toàn bộ
ZIP, bao gồm thư mục `concat`. Các project `.taoclip` cũ được giữ nguyên và cần
TaoClip để mở; chọn video gốc để tạo bản dựng Concat độc lập.
Xem [CONCAT.md](CONCAT.md) về lưu trữ, build, giới hạn liên kết bản xuất và giấy phép.

## Chạy

Giải nén `dist\ScreenRecorder-portable.zip` rồi mở `ScreenRecorder.exe` trong thư mục `ScreenRecorder`.
Sau khi clone repo hoặc dọn cache, chạy `powershell -ExecutionPolicy Bypass -File tools\package.ps1`
để tải dependency và tạo lại `dist\ScreenRecorder\ScreenRecorder.exe`.
Xem [GITHUB_UPLOAD.md](GITHUB_UPLOAD.md) để cập nhật repo GitHub và phát hành bản portable.
Giữ `ffmpeg.exe` và `ffprobe.exe` bên cạnh ứng dụng.
Không cần cài Rust hoặc FFmpeg trên máy sử dụng.

1. Trong tab **Quay**, chọn **Màn hình**, **Cửa sổ** hoặc **Vùng chọn**. Nguồn được chọn có nền xanh và chữ **Đã chọn**; thumbnail và danh sách thiết bị cập nhật ở nền.
2. Với **Vùng chọn**, kéo chuột rồi nhấn **Enter**, hoặc dùng mũi tên để tạo/di chuyển vùng, **Shift + mũi tên** để đổi cỡ; **Esc** hủy. Có thể chọn toàn màn hình rồi nhập X/Y/rộng/cao bằng bàn phím ở cửa sổ chính.
3. Bật riêng **Micro** và **Tiếng máy**, chọn thiết bị và xem mức tín hiệu. Thiết bị đã chọn bị ngắt sẽ được báo rõ, không âm thầm đổi sang thiết bị khác.
4. Chọn preset và cách quay, kiểm tra đường dẫn **Lưu tại**, rồi bấm **Quay**. Đếm ngược 3 giây; có thể hủy. Cửa sổ chính tự thu nhỏ khi bắt đầu quay.
5. **Quay nhanh**: dừng rồi xem kết quả. **Quay rồi chỉnh sửa**: lưu thành công rồi mở video trong editor Concat. Màn hình kết quả có thumbnail, **Xem**, **Chỉnh sửa/Tiếp tục chỉnh sửa**, **Mở thư mục**.
6. Toolbar nổi có tạm dừng/tiếp tục/dừng, bật/tắt âm thanh, chụp PNG và chú thích. Kéo dải sáu chấm hoặc dải dưới; khi toolbar có focus dùng **Alt + mũi tên** để di chuyển. **Tab / Shift+Tab**, **Enter / Space** thao tác nút. Bản gọn vẫn có micro và tiếng máy.

| Preset | Giới hạn chiều cao | FPS | Bitrate | Encoder |
|---|---:|---:|---:|---|
| Nhẹ | 720 px | 30 | 4 Mbps | Tự động |
| Cân bằng | 1080 px | 30 | 8 Mbps | Tự động |
| Chất lượng cao | Kích thước gốc | 30 | 16 Mbps | Tự động |

Không phóng lớn nguồn nhỏ. Phần **Nâng cao** giữ encoder, bitrate, mức âm và hiệu ứng con trỏ; sửa thông số riêng hiển thị **Tùy chỉnh**. Preset mới không ghi đè cấu hình cũ cho đến khi được chọn.

Nút Quay/dừng và nơi lưu nằm ngoài vùng cuộn. Khi cửa sổ nhỏ hoặc scaling cao, nội dung cấu hình cuộn được; đường dẫn dài có tooltip. Cửa sổ editor tự vừa vùng làm việc và giữ nút Xuất trong tầm nhìn. Ngôn ngữ VI/EN của recorder được chuyển tới editor khi mở.

Nếu nguồn hoặc âm thanh mất trong khi quay và bộ quay vẫn cứu được MP4, ứng dụng hiển thị kết quả kèm cảnh báo; không tự mở editor. Nếu không lưu được, lỗi và phiên phục hồi được giữ. Kết nối thiết bị mới trước khi bắt đầu bản ghi để tạo đúng track âm thanh.

Ở chế độ **Cửa sổ**, dùng ô tìm kiếm và chọn đúng thumbnail/tiêu đề. Danh sách tự cập nhật khoảng 2 giây; mất nguồn không làm chuyển sang cửa sổ khác. Một worker tạo thumbnail, hàng đợi tối đa 8, cache tối đa 64 ảnh; nguồn không trả khung hình bị giới hạn thời gian chờ.
Toolbar kéo được bằng vùng sáu chấm hoặc dải trạng thái dưới. Thu gọn/mở rộng bằng nút tương ứng; tên chức năng hiện khi rê chuột hoặc chuyển focus bàn phím.
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

### Thư viện — Phase 1 (0.7.0)

- Chuyển **Lưới / Danh sách**, tìm theo tên tiếng Việt, lọc Video / Ảnh / Project / Bản xuất, lọc Yêu thích và sắp xếp ngày tạo / tên / dung lượng. Danh sách ảo không giới hạn 80 file.
- Bấm tên một mục để xem chi tiết và các liên kết; menu **…** chứa thao tác mở, chỉnh sửa, đổi tên hiển thị, yêu thích và Recycle Bin. Ô chọn hỗ trợ nhiều mục; **Chọn kết quả** chọn toàn bộ kết quả lọc, kể cả mục ngoài màn hình.
- **Đổi tên** chỉ sửa tên trong thư viện, không đổi tên hay di chuyển media. Xóa chuyển đúng các file đã chọn vào Recycle Bin; không tự xóa các nguồn, project hoặc bản xuất liên quan. Không xóa phiên đang ghi/hoàn thiện.
- Trạng thái gồm **Đang hoàn thiện**, **Sẵn sàng**, **Thiếu file**, **Cần phục hồi**. Phiên bị gián đoạn trong `.sessions` có nút phục hồi. File media lỗi có **Đọc lại**; project thiếu media mở được trong editor để liên kết lại.
- **Tiếp tục chỉnh sửa** ưu tiên project Concat (`concat.json`) đã lưu, kể cả khi bấm Chỉnh sửa từ video gốc. Nếu project mất/hỏng, ứng dụng báo lỗi thay vì âm thầm tạo timeline mới. Lưu project trước khi đóng để bảo đảm phục hồi đúng nội dung.
- Cầu nối đăng ký project Concat vào thư viện. Bản xuất từ giao diện Concat được quét như video nếu lưu trong thư mục đang quản lý; chưa tự phân loại/liên kết bản xuất ngoài thư mục này. Các liên kết và sidecar TaoClip cũ vẫn được đọc.
- **Thư mục & dữ liệu → Thêm thư mục…** bổ sung thư mục cần quản lý; đổi nơi lưu bản ghi vẫn giữ thư mục cũ trong thư viện. Quét thư mục con ở nền, bỏ symlink/junction và thư mục ẩn, kiểm tra thay đổi khoảng 3 giây/lượt. Hai worker đọc metadata/thumbnail, không chạy FFmpeg trong luồng vẽ giao diện.
- Đổi tên/di chuyển file trong cùng ổ đĩa và các thư mục đã quản lý thường được nhận diện bằng Windows file ID. Khi chuyển ra ngoài thư mục đã biết hoặc sang ổ đĩa khác, dùng **Tìm lại file…** trên mục Thiếu file (hoặc thêm thư mục đích). Khi Concat báo thiếu media, dùng chức năng liên kết lại media trong Concat; file project chỉ đổi khi lưu.
- Metadata thời lượng/kích thước video và ảnh được đọc bằng FFprobe; thumbnail 320×180 tối đa được lưu trên đĩa, GPU cache tối đa 128 ảnh. File mới cần ổn định ít nhất 2 giây trước khi đọc. FFprobe/FFmpeg có timeout riêng, file lỗi không chặn toàn bộ hàng đợi.

Dữ liệu thư viện v1 nằm tại `%LOCALAPPDATA%\ScreenRecorder\library`: `index.json`, `index.backup.json`, `roots.json`, `events/`, `thumbnails/`. Một coordinator giữ khóa ghi độc quyền; file tạm được flush rồi thay thế nguyên tử bằng Windows MoveFileEx. Khi đóng ứng dụng, các lệnh còn chờ được ghi xong. Chỉ mục lỗi được phục hồi từ backup, hoặc xây dựng lại bằng danh sách thư mục, file gốc, project và nhật ký editor; bản chỉ mục không đọc được được giữ riêng. **Xây dựng lại cache** giữ tên hiển thị/yêu thích hiện có. Nếu mất cả hai chỉ mục thì không thể suy ra tên hiển thị/yêu thích từ media; cần sao lưu thư mục dữ liệu nếu muốn bảo toàn các thông tin này.

Chỉ một cửa sổ recorder được ghi vào cùng một thư viện tại một thời điểm; cửa sổ thứ hai báo khóa đang được sử dụng. Các cửa sổ editor vẫn hoạt động độc lập và ghi sự kiện riêng, không sửa trực tiếp chỉ mục.

Chẩn đoán trên thư viện thử riêng:

```powershell
$env:SCREENRECORDER_LIBRARY_DIR = 'D:\ThuVienThu\catalog'
.\ScreenRecorder.exe --library-scan 'D:\ThuVienThu\recordings' 'D:\ThuVienThu\report.json'
```

Lệnh chờ tối đa 45 giây để metadata ổn định và xuất báo cáo JSON. `SCREENRECORDER_SETTINGS_PATH` dùng profile cài đặt riêng khi kiểm thử. `--inspect-ui` cùng `SCREENRECORDER_UI_CAPTURE=<đường dẫn PNG>` xuất framebuffer cửa sổ, phục vụ kiểm tra giao diện; không bật trong sử dụng thông thường.

Kiểm thử Phase 2 bằng `python tools/check_recording_ui.py dist/ScreenRecorder/ScreenRecorder.exe quick 1 vi` hoặc thay `quick` bằng `edit`, `source-loss`, `disconnected`, `ready`; hai tham số cuối là app zoom và ngôn ngữ. Công cụ tự tạo cửa sổ mẫu và profile riêng, chỉ ghi cửa sổ mẫu của chính lượt chạy. Zoom phục vụ kiểm tra layout, không thay thiết lập DPI của Windows. Kết quả và giới hạn kiểm chứng nằm trong `VALIDATION.md`.

### Chỉnh sửa sau khi ghi

ScreenRecorder mở **Concat 0.2.8** trong cửa sổ riêng. Bấm **Chỉnh sửa** tự đưa video
vào timeline với kích thước/FPS gốc. Âm thanh nằm trong clip video. **Ctrl+S** lưu;
lần bấm tiếp theo mở đúng project đã chỉnh. Video gốc không bị sửa.

Concat hỗ trợ nhiều track, cắt/ghép, chữ, keyframe, hiệu ứng và xuất MP4. Model AI
là tùy chọn tải riêng. Các project TaoClip cũ được giữ nguyên và cần TaoClip để mở.
Chi tiết vị trí project, giới hạn và giấy phép: [CONCAT.md](CONCAT.md).

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

Cần Rust MSVC x64, Visual C++ Build Tools / Windows SDK và Python 3.

```powershell
python tools\fetch_ffmpeg.py
python -X utf8 tools/prepare_concat.py
cargo test --locked
cargo clippy --locked -- -D warnings
powershell -ExecutionPolicy Bypass -File tools\package.ps1
```

Script đóng gói tải bản Concat cố định, xác minh SHA-256, giải nén runtime (không cài
vào hệ thống), dựng Rust release và tạo ZIP. Không cần npm/Node/Electron.

```powershell
python -X utf8 tools/prepare_concat.py
cargo run
python -X utf8 tools/check_concat.py
```

`Cargo.lock` cố định dependency Rust. Nguồn và checksum của Concat được ghi trong
`concat/UPSTREAM.json` của bản portable; mã nguồn Concat đúng commit được đóng kèm.

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
- `src/editor_launcher.rs`: mở editor đi kèm, truyền video và giao diện báo lỗi.
- `src/concat_bridge.rs`: cầu nối MCP tạo/lưu/mở timeline và giám sát Concat.
- `editor/.testout/` (nếu có): dữ liệu kiểm thử cũ chỉ giữ ở máy, không đưa lên GitHub. Mã và runtime TaoClip dự phòng đã được dọn.
- `src/editor.rs`: bộ dựng cũ giữ lại cho lệnh hồi quy `--editor-smoke`, không dùng trong giao diện chính.
- `src/recorder.rs`: thử encoder, ghi, tạm dừng, xuất và phục hồi.

Mã recorder: MIT. Concat: AGPL-3.0-or-later. FFmpeg đi kèm có giấy phép GPL v3;
xem `THIRD_PARTY_NOTICES.md` và các thông báo giấy phép trong bộ portable.
