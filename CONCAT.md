# Concat integration

ScreenRecorder 0.9.0 uses the **unmodified official Concat 0.2.8 Windows x64**
application, in `concat/`, as its default editor. The capture engine remains
the existing Rust implementation. No Electron or Node installation is needed.

## Editing

Click **Chỉnh sửa / Edit** on a recording. ScreenRecorder probes it, creates a
Concat project with its original dimensions and exact frame rate, places the
video (including its audio) at time zero, saves, releases the project lock and
opens Concat. Repeated clicks focus the existing editor. Save with **Ctrl+S**;
the next Edit opens the saved timeline without importing another copy.

Projects are stored under `%LOCALAPPDATA%/ScreenRecorder/library/concat-projects`.
The full project path is visible in Concat. Keep the entire project folder and
its source media. Concat has its own project lock and missing-media recovery.
Settings/cache are isolated under `%LOCALAPPDATA%/ScreenRecorder/concat-profile`.
Launch errors and logs are under the library's `concat-launches`/`concat-logs`.
`SCREENRECORDER_LIBRARY_DIR` overrides the library location for tests.

## Existing TaoClip projects

Existing `.taoclip` files and source media are **not deleted, overwritten or
migrated**. They remain visible as legacy projects.
They need TaoClip to continue their original edits; Concat does not read that
format. Clicking Edit on the original recording creates an independent Concat
project. Only Concat projects are selected automatically for new edits.
The obsolete TaoClip source/runtime backups were removed during repository
cleanup on 2026-10-28. TaoClip is no longer a build dependency. Any remaining
`editor/.testout/` contains preserved local test data and is ignored by Git.

## Export and library

Use Concat's Export command and choose a destination. Outputs saved in a watched
recording folder are discovered by the library as videos. The unmodified Concat
GUI does not publish TaoClip's custom export events: an arbitrary GUI export is
not automatically classified or linked as an export in ScreenRecorder. Saved
Concat project metadata and changes are refreshed by the library.

## Build and redistribution

Run `powershell -ExecutionPolicy Bypass -File tools/package.ps1`. The script
downloads a pinned MSI, verifies its SHA256 and administratively extracts its
files; it does not install Concat or change file associations. Build only the
recorder with `cargo build --release --locked` after preparing dependencies.

Concat is AGPL-3.0-or-later. Its LICENSE, third-party notices, exact upstream
source archive (including build scripts and Cargo.lock), and binary provenance
are included beside the executable. FFmpeg DLLs and other dependencies retain
their own licenses as described in Concat's THIRD_PARTY_NOTICES.md. Recorder
code is MIT; that label does not apply to the whole portable distribution.
No Concat code is linked into the recorder: the bridge uses public MCP over
stdio. The original name/icon identify the unmodified upstream application.

When republishing a portable build, keep its notices and source archive and
provide corresponding dependency sources/build information required by those
licenses, together with the recorder source. Optional AI models are not bundled.

## Diagnostics

`ScreenRecorder.exe --concat-check INPUT.mp4 OUTPUT.mp4` creates or opens the
recording's project, saves, closes and reopens it, verifies the timeline and
exports through the actual bundled Concat engine. It refuses an existing output
or an output equal to an input media file. A JSON report is written beside the
output. Use an isolated `SCREENRECORDER_LIBRARY_DIR` for automated tests.
