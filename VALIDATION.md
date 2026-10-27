# Repository cleanup (2026-10-28)

Downloaded dependencies, build caches, obsolete TaoClip source/runtime, old release
archives and extracted application copies were moved to the Windows Recycle Bin.
Recordings, legacy projects, test media, Git history and the tested portable ZIP
were retained. Ignore rules exclude local data and generated files from Git.
Packaging now restores missing FFmpeg as well as Concat; library fixtures generate
their own input rather than requiring TaoClip's old test output.

Python scripts and the PowerShell packaging script were syntax-checked. Rust source
was unchanged during cleanup; the application tests below were run on October 26,
not rerun after removing build caches. The current source ZIP is regenerated;
the portable ZIP remains the previously tested build.

---

# Concat replacement — ScreenRecorder 0.9.0 (2026-10-26)

This section supersedes earlier editor validation below. Concat 0.2.8 is the
unmodified official Windows x64 build; recorder capture code was retained.

- `cargo test --locked`: **26 passed** (includes Concat project metadata and
  prioritisation while preserving legacy project bytes).
- `cargo clippy --locked -- -D warnings`: passed.
- `tools/check_concat.py`: passed against development and packaged executables.
  Synthetic 640×360 videos at **30, 60, 30000/1001 fps**, silent and AAC tone,
  Vietnamese filenames/spaces. Automatic import at time zero; save/close/reopen;
  real split edit persisted across processes; second import reused the project
  and retained both clips; real Concat H.264 exports decoded successfully, with
  exact rates/dimensions, expected durations, and non-silent decoded audio.
- Existing output/source overwrite requests, nonexistent media, corrupt video,
  and unsupported `.taoclip` launch requests were rejected. Legacy fixture
  contents were unchanged. Catalogue indexed all three Concat projects as Ready.
- Packaged GUI: real recorded video opened with one clip, 962×632 at 30 fps;
  accessibility confirmed its name, timeline duration and clip count. Ctrl+S
  displayed **Project saved**; Ctrl+Q closed the saved window. A duplicate Edit
  request focused the same process and did not change the project bytes.
- `tools/check_recording_handoff.py`: packaged Rust recorder captured only its
  own test-pattern window, silent, with pause/resume; 4.132 s H.264 decoded.
- `tools/check_recording_ui.py ... edit 1 vi`: final 0.9.0 binary completed
  Ready → Countdown → Starting → Recording → Pausing → Paused → Recording →
  Finishing → Ready, then opened Concat successfully. Recorded file decoded.

Evidence in `test-output/concat-acceptance-1792997205542684700/acceptance.json`,
`test-output/concat-window/`, and `test-output/phase2-ui/edit-1-vi/acceptance.json`.
Copies of reports are packaged under `validation/concat/`.

Limits: native screenshot capture repeatedly timed out for Concat and the
computer-use tool could not obtain click geometry. GUI project opening, saving,
closing and export-dialog settings were checked via accessibility/keyboard;
**the final GUI Export button was not automated**. Actual export/save/reopen
was verified through the bundled engine's public MCP API, not a mock or the
recorder's old export code. No claim is made for every Concat effect, AI model,
codec, GPU, or lengthy production edit. Concat remains beta. GUI exports outside
watched folders are not automatically linked into the recorder catalogue.

At this validation date the old TaoClip source/runtime and all existing project
files were retained. The October 28 cleanup removed those obsolete source/runtime
copies while preserving the project files.
The portable archive includes Concat, notices and the upstream source archive;
it excludes Electron/TaoClip, user recordings, project data and profiles.

---

# Kiểm chứng Phase 2 — Giao diện quay 0.8.0 — 26/10/2026

Môi trường: Windows x64, màn hình 1280×720. Giữ bộ quay Rust/WGC/WASAPI/FFmpeg,
thư viện Phase 1 và editor TaoClip/Electron. Không thay framework. Các phần bên dưới
là lịch sử kiểm chứng; kết quả Phase 2 được ghi riêng ở đây.

## Phạm vi đã triển khai

| Yêu cầu | Kết quả |
|---|---|
| Ba nguồn quay | Màn hình / Cửa sổ / Vùng chọn; thumbnail thật, tên, kích thước, dấu Đã chọn; tìm cửa sổ. Không tự chuyển sang nguồn khác khi nguồn mất. |
| Nền và cache | Một worker thumbnail, queue 8, cache 64, timeout lấy khung hình 1,2 giây; inventory nguồn/thiết bị cập nhật khoảng 2 giây. Không chụp thumbnail trong luồng vẽ UI. |
| Hai nhóm âm thanh | Micro và Tiếng máy có bật/tắt, endpoint, mức tín hiệu WASAPI; thiết bị đã chọn mất kết nối được báo rõ. Không fallback âm thầm cho ID đã mất; không bật track chưa được chuẩn bị khi đang quay. |
| Preset thực | Nhẹ: tối đa 720p/4 Mbps; Cân bằng: 1080p/8 Mbps; Chất lượng cao: kích thước gốc/16 Mbps. Cả ba 30 fps, encoder tự động. Có Nâng cao, nhận diện cấu hình tùy chỉnh. |
| Trạng thái và thao tác chính | Quay luôn ở thanh dưới; sẵn sàng, đếm ngược/hủy, chuẩn bị, đang quay, đang tạm dừng, tạm dừng, đang lưu, lỗi. Đường dẫn lưu và tóm tắt luồng/chất lượng luôn hiện. |
| Hai luồng | Quay nhanh hiển thị kết quả; Quay rồi chỉnh sửa chỉ mở editor sau Saved thành công. Bản lưu kèm cảnh báo không tự mở editor. |
| Kết quả | Thumbnail/metadata lấy từ cache thư viện; Xem, Chỉnh sửa hoặc Tiếp tục chỉnh sửa, Mở thư mục, Quay tiếp. Việc tạo thumbnail có thể cần vài giây sau khi file ổn định. |
| Toolbar | Vùng kéo lớn hơn, bản gọn có cả micro/tiếng máy, trạng thái bật/tắt/mất thiết bị, tooltip; Tab/Shift+Tab, Enter/Space; Alt+mũi tên di chuyển và giới hạn trong màn hình. |
| Thống nhất và hai ngôn ngữ | Palette/Segoe UI thống nhất recorder, library và editor; VI/EN được truyền khi mở hoặc tái sử dụng editor. Không thay đổi cấu trúc project. |
| Cửa sổ nhỏ | Cửa sổ tự giới hạn theo màn hình/DPI; nội dung cuộn, các nút quay vẫn cố định. Editor tự co workspace; nội dung hộp Xuất cuộn, nút bắt đầu xuất luôn trong viewport. |

## Kiểm thử và bằng chứng

- Rust: `cargo test --locked`: **25 đạt, 0 lỗi**. Bao gồm preset thực, tương thích preferences cũ, thiết bị âm thanh mất ID, region bàn phím, thumbnail và toàn bộ test thư viện Phase 1.
- `cargo clippy --locked --all-targets -- -D warnings` và build release đạt.
- Editor: unit, launch-request, library bridge đạt; build/pack thành công. Kiểm tra locale từ recorder, palette, nút Xuất và nút bắt đầu xuất ở viewport nhỏ/deviceScaleFactor 1,5: **4 assertion đạt**.
- `tools/check_recording_ui.py`: chạy EXE portable với profile riêng, chọn đúng cửa sổ mẫu theo PID, gọi handler thật và nhận event recorder thật. **Quay nhanh** và **Quay rồi chỉnh sửa** đi qua Countdown → Recording → Paused → Recording → Finishing → Ready; MP4 không audio được giải mã toàn bộ, chỉ luồng chỉnh sửa mở editor.
- **Nguồn biến mất**: đóng cửa sổ mẫu trong khi quay; ứng dụng trả kết quả/cảnh báo, không tự mở editor. **Thiết bị không còn tồn tại**: ID microphone không hợp lệ bị chặn trước khi quay, hiển thị lỗi.
- `editor/e2e/library-roundtrip.cjs` trên hai EXE portable: video tên tiếng Việt/khoảng trắng, chia thành hai clip → lưu `.taoclip` → đóng → mở đúng hai clip và tên project → xuất; xác nhận SHA-256 media gốc không đổi, provenance và catalogue giữ liên kết; Rust Continue mở project đã lưu. Đây là hồi quy chức năng Phase 1, không chỉ kiểm tra file tồn tại.
- `cargo run --release --example check_studio`: QSV thật, chú thích, pause/resume đạt; **0 điểm ảnh toolbar lọt vào video**. Bài này không kiểm tra mute audio phần cứng (`live_audio_mute: false`).
- `cargo run --release --example check_region`: vùng có gốc (100,80), video **640×360**, đạt.
- Xem framebuffer thật của recorder tại app zoom 100%, 125%, 150%, VI/EN; nguồn, kết quả, lỗi thiết bị, toolbar đầy đủ/bản gọn. Đã sửa lỗi cắt nút Mở thư mục tại 125% và giữ nút bắt đầu xuất của editor trong viewport nhỏ.

Log: `test-output/phase2-rust-tests.log`, `phase2-clippy.log`, `phase2-release.log`,
`phase2-editor-tests.log`, `phase2-editor-ui.log`, `phase2-portable-quick.log`,
`phase2-portable-edit.log`, `phase2-portable-source-loss.log`,
`phase2-library-roundtrip.log`, `phase2-annotations.log`, `phase2-region.log`.
JSON và ảnh recorder nằm trong `test-output/phase2-ui`; toolbar trong
`test-output/phase2-toolbar`. Bằng chứng chọn lọc được kèm ở
`dist/ScreenRecorder/validation/phase2`, không kèm video thử hoặc recordings cá nhân.

## Giới hạn kiểm chứng còn lại

- 125%/150% dùng app zoom của egui và device scale của Electron trên màn hình 1280×720; **chưa thay Windows scaling thực**, chưa kiểm thử chuyển qua màn hình khác DPI. Không đồng nhất kiểm tra layout này với chứng nhận mọi cấu hình DPI Windows.
- Công cụ điều khiển Windows không chụp/click ổn định cửa sổ capture-excluded; đã dùng framebuffer opt-in cho recorder. Chụp editor qua CDP bị timeout, không có ảnh editor cuối đáng tin cậy; các kiểm tra DOM/timeline/export vẫn thực hiện trên Electron thật.
- Có kiểm tra focus Tab, handler trạng thái, logic region bàn phím, nhưng **chưa xác nhận thành công tổ hợp global hotkey vật lý** trong môi trường có cửa sổ recorder khác giữ hotkey. Có fallback khi cửa sổ chính nhận Ctrl+Shift+F9/F10 và chống kích hoạt trùng. Cần nghiệm thu phím tắt toàn cục/chuột kéo với một instance thông thường.
- Kiểm tra mất thiết bị bằng ID không tồn tại; **chưa rút/cắm USB audio vật lý** hoặc đo mức thu micro/loopback và mute trên phần cứng trong lượt Phase 2. Mất nguồn được thử bằng đóng cửa sổ thật.
- Chưa chạy stress quay nhiều giờ; portable chưa ký số. Các mục trên không được tính là đã kiểm chứng đạt.

Đã đọc và kiểm tra giấy phép Cap tại commit ghi trong `DESIGN_REFERENCES.md`;
chỉ học tổ chức/tương tác, không sao chép mã nguồn hoặc asset Cap.

---

# Kiểm chứng Phase 1 — Thư viện 0.7.0 — 26/10/2026

Môi trường: Windows 10 x64, workspace `D:\Khac\ScreenRecorder`. Bản chạy nằm trong
`dist/ScreenRecorder`, ZIP portable và ZIP mã nguồn nằm trong `dist`.
Các phần kiểm chứng phiên bản trước được giữ bên dưới và không thay thế kết quả Phase 1.

| Yêu cầu | Kết quả và bằng chứng |
|---|---|
| Lưới/danh sách, thumbnail, tên, thời lượng, kích thước, dung lượng, ngày | Đã triển khai; xem trực quan framebuffer của ứng dụng thật với 1.504 mục, tên tiếng Việt, ảnh, video và mục thiếu/lỗi. |
| Tìm/lọc/sắp xếp, bỏ giới hạn 80 | Danh sách và lưới ảo hóa. Bài 20.000 mục: tìm + lọc yêu thích + sắp xếp mất 21,72 ms trong lần chạy debug cuối; viewport 700 px dựng 6 hàng. 120 khung cuộn giả lập bằng egui có p95 1,18 ms, đi tới hàng 15.978. Đây là số đo logic/layout, không phải FPS của màn hình hay tốc độ quét đĩa. |
| Đổi tên hiển thị, yêu thích, chọn nhiều, mở file/thư mục, Recycle Bin | Đã triển khai. UI tìm tên Unicode, đổi chế độ, chọn cả 10 kết quả và yêu thích nhiều mục đã được thao tác. Test xác nhận tên/yêu thích được ghi ngay cả khi đóng ứng dụng ngay sau yêu cầu; tên và byte file gốc không đổi. Test IFileOperation đưa đúng file fixture vào Recycle Bin, giữ file không chọn. |
| Đang hoàn thiện / sẵn sàng / thiếu file / cần phục hồi | Test phiên còn hoạt động, phiên gián đoạn, file mất và file lỗi đạt. Video của phiên chưa hoàn thiện không bị báo sẵn sàng. |
| Liên kết gốc–project–bản xuất, tiếp tục project | E2E trên editor đã đóng gói: chia clip thành hai, lưu `.taoclip`, đóng editor, mở lại đúng hai clip và tên project. Launcher Rust khi nhận video gốc tiếp tục mở project đã lưu. |
| Editor thông báo lưu/xuất | Event journal ghi nguyên tử; E2E xác nhận catalogue Rust nhận project/bản xuất và liên kết hai chiều. Bản xuất có sidecar để xây dựng lại liên kết. |
| Quét/metadata/thumbnail nền, cache và cập nhật | Coordinator riêng, 2 worker metadata, hàng đợi giới hạn, timeout FFprobe/FFmpeg. Test cache hit, file đổi, thumbnail tái tạo, video không audio và file hỏng đạt. UI không thực hiện probe/quét trong paint. |
| Dữ liệu phiên bản, ghi an toàn, xây dựng lại | Schema v1, atomic replace + fsync, bản sao dự phòng, khóa một writer. Test index hỏng/backup, đổi đường dẫn, giữ root cũ khi đổi thư mục, rebuild từ media/project/export sidecar đạt. Nhận diện file đã có trong recordings. |

## Kiểm thử đã chạy

- `cargo test --locked -- --nocapture`: **21 passed, 0 failed**.
- `cargo clippy --locked -- -D warnings`: đạt, không cảnh báo.
- `cargo build --release --locked`, `npm run build`, `npm run pack`: đạt.
- `npm run test:unit`, `npm run test:launch`, `node test-library-bridge.cjs`: đạt.
- E2E tích hợp editor: tự nhập timeline, preview, split/undo, mở trùng, cửa sổ riêng,
  video không audio, lưu và xuất đạt.
- `python tools/check_recording_handoff.py`: bản portable quay cửa sổ mẫu, pause/resume,
  MP4 **4,2 giây**, giải mã toàn bộ thành công; không quay desktop/micro/tiếng máy trong bài này.
- Dùng chính bản ghi vừa tạo chạy `editor/e2e/library-roundtrip.cjs` với hai EXE portable:
  lưu → đóng → tiếp tục → xuất → quét thư viện đạt toàn bộ 15 assertion.
  SHA-256 trước/sau xác nhận file gốc giữ nguyên byte; không có renderer exception.

Log tại `test-output/phase1-rust-tests-final.log`, `phase1-clippy-final.log`,
`phase1-release-final.log`, `phase1-editor-unit.log`, `phase1-recording-handoff.log`,
`phase1-portable-recording-roundtrip.log`. Dữ liệu roundtrip được chỉ tới bởi
`editor/.testout/library-latest.json`. Các bằng chứng chọn lọc được kèm trong
`dist/ScreenRecorder/validation` và ZIP portable; không kèm media thử hay recordings.

## Kiểm tra trực quan và giới hạn

Đã xem ảnh lưới, danh sách đang tìm kiếm, chọn nhiều và chi tiết của cửa sổ egui thật;
không thấy chữ/điều khiển chồng lấn ở kích thước 1120×650. Nội dung editor sau phục hồi
được xác nhận bằng DOM/timeline và video xuất; ảnh CDP lần cuối bị đen, không dùng làm bằng chứng trực quan.
Công cụ Windows vẫn lỗi chụp WGC/click tọa độ trên cửa sổ egui; dùng bàn phím/accessibility
để thao tác và chế độ chụp framebuffer `--inspect-ui` để kiểm tra hình ảnh.
Không tuyên bố đã kiểm chứng kéo thanh cuộn bằng chuột hoặc FPS desktop; bài cuộn lớn
là kiểm thử egui tự động. Mở file/thư mục dùng shell Windows; không tự động mở ứng dụng
media mặc định trong bài hồi quy.

- Thay đổi file được phát hiện bằng polling khoảng 3 giây; quét ban đầu/thumbnail còn cần
  thời gian tùy ổ đĩa và số lượng media. Tìm/cuộn dùng snapshot/cache đã nạp.
- Di chuyển trong cùng volume được theo dõi bằng file ID và ngày tạo. Di chuyển sang
  volume khác hoặc ra ngoài các root đã biết cần chọn lại file/thêm thư mục trong UI.
- Nếu mất cả index và backup, có thể dựng lại file/metadata/liên kết có journal hoặc
  sidecar, nhưng không thể khôi phục tên hiển thị/yêu thích đã mất. Không sửa file media
  để lưu các thông tin đó. Journal/sidecar cần được giữ khi sao lưu thư viện.
- Không chạy lại stress quay nhiều giờ, DPI hỗn hợp hoặc đo audio phần cứng trong Phase 1.
  Portable chưa ký số. Các giới hạn này không được tính là đã kiểm chứng thành công.

Cap chỉ được dùng tham khảo tổ chức và tương tác; đã kiểm tra giấy phép AGPLv3 và ngoại lệ
theo commit được ghi trong `DESIGN_REFERENCES.md`. Không sao chép mã nguồn hoặc asset Cap.

---

# Kiểm chứng tích hợp ScreenRecorder Editor — 26/10/2026

Windows 10 x64. Bộ quay Rust được giữ nguyên; giao diện chính gọi editor Electron riêng.
Nguồn TaoClip: commit `f28f074b304b9f12294e16f747a26ec8e6f30d87`.

- `cargo test --locked`: **14 passed**, gồm kiểm tra đường dẫn video có tiếng Việt/dấu cách và file không hợp lệ.
- `cargo clippy --locked -- -D warnings`: đạt.
- Rust release và React/Electron portable: dựng thành công.
- `npm run test:unit`: schema/project, track, timeline, commands và renderer static checks đều đạt.
- `node test-launch-request.cjs`: đạt, gồm thiếu file, sai định dạng, thư mục và đường dẫn Unicode.
- `node test-ffmpeg.cjs`: **105 PASS / 0 FAIL**.
- E2E trên bản phát triển và bản Electron đã đóng gói: tự nhập video vào timeline, phát preview,
  thumbnail, tỷ lệ gốc, split/undo, mở lại không nhập trùng, cửa sổ riêng cho video khác,
  video không audio, lưu project và xuất MP4 có hình/tiếng đều đạt.
- Cửa sổ rộng 960 px: nút Xuất vẫn nằm trong phần giao diện có thể thao tác.
- File không tồn tại: editor hiển thị lỗi và vẫn có giao diện sử dụng được.
- `python tools/check_recording_handoff.py`: Rust ghi **chỉ cửa sổ mẫu**, không desktop/audio/micro,
  có pause/resume; MP4 H.264 dài **4,166 giây**, giải mã toàn bộ thành công.
- E2E lần cuối dùng video Rust vừa ghi và `dist/ScreenRecorder/ScreenRecorder.exe --open-editor`:
  đường dẫn đến đúng editor đi kèm; mở lại cùng clip không tạo cửa sổ mới, clip khác mở riêng;
  split/undo, lưu project và xuất MP4 đúng thời lượng/luồng media đều đạt.
- Đã đối chiếu file handoff/main/preload và cấu hình filter trong `app.asar` với nguồn.
- Bộ portable chứa editor, FFmpeg/FFprobe và thông báo giấy phép Rust, JavaScript, Electron/TaoClip/Twemoji.
  Thư mục recordings và dữ liệu kiểm thử không nằm trong ZIP.

Ảnh kiểm chứng: `editor/.testout/recorder-integration/editor.png`.
Báo cáo quay mẫu: `test-output/handoff-recording.json`.
Log chi tiết: `rust-tests.log`, `rust-clippy.log`, `editor-ffmpeg-tests.log`,
`editor-integration-dev.log`, `editor-integration-packaged.log`, `editor-integration-rust.log`.

Phạm vi: kiểm thử handoff tự động đi qua entrypoint launcher dùng chung với các nút Chỉnh sửa;
không có bài tự động nhấn các nút egui của cửa sổ quay. AI/model tải riêng và dịch vụ trực tuyến
không thuộc đợt kiểm chứng này. Editor vẫn xuất 30 fps theo upstream. Bộ portable chưa ký số.

---

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
