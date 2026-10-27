//! WASAPI capture with QPC packet timestamps: silence and device-clock drift do not
//! compress the recording timeline. Raw PCM files avoid WAV's 4 GiB size limit.
use anyhow::{bail, Context, Result};
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
};
use windows::Win32::{
    Media::Audio::{Endpoints::IAudioMeterInformation, *},
    System::{Com::*, Performance::*},
};

pub fn clock_100ns() -> Result<i64> {
    unsafe {
        let mut counter = 0;
        let mut freq = 0;
        QueryPerformanceCounter(&mut counter)?;
        QueryPerformanceFrequency(&mut freq)?;
        Ok((counter as i128 * 10_000_000 / freq as i128) as i64)
    }
}

struct ComGuard;
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct Input {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    file: File,
    end_sample: u64,
    mic: bool,
}
impl Drop for Input {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

impl Input {
    unsafe fn new(mic: bool, path: PathBuf, id: &str) -> Result<Self> {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = endpoint(&enumerator, mic, id).with_context(|| {
            if mic {
                "Không có micro mặc định."
            } else {
                "Không có thiết bị phát âm thanh mặc định."
            }
        })?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let format = WAVEFORMATEX {
            wFormatTag: 3,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 384_000,
            nBlockAlign: 8,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        let mut flags =
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        if !mic {
            flags |= AUDCLNT_STREAMFLAGS_LOOPBACK;
        }
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, &format, None)
            .context("Không khởi tạo được âm thanh 48 kHz. Kiểm tra thiết bị âm thanh và quyền micro của Windows.")?;
        let capture: IAudioCaptureClient = client.GetService()?;
        Ok(Self {
            client,
            capture,
            file: File::create(path)?,
            end_sample: 0,
            mic,
        })
    }

    unsafe fn drain(
        &mut self,
        epoch: i64,
        gain: f32,
        live: &Option<crate::studio::Shared>,
    ) -> Result<()> {
        if let Some(l) = live {
            l.lock().unwrap().levels[usize::from(self.mic)] *= 0.94;
        }
        while self.capture.GetNextPacketSize()? > 0 {
            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            let mut qpc = 0;
            self.capture
                .GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
            // Copy before releasing WASAPI's buffer, including the null silent-buffer case.
            let mut bytes = if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                vec![0; frames as usize * 8]
            } else {
                std::slice::from_raw_parts(data, frames as usize * 8).to_vec()
            };
            self.capture.ReleaseBuffer(frames)?;
            let mut peak = 0.0f32;
            for sample in bytes.chunks_exact_mut(4) {
                let v = f32::from_le_bytes(sample.try_into().unwrap());
                let v = if v.is_finite() {
                    (v * gain).clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                peak = peak.max(v.abs());
                sample.copy_from_slice(&v.to_le_bytes());
            }
            if let Some(l) = live {
                l.lock().unwrap().levels[usize::from(self.mic)] = peak;
            }
            let timestamp = if flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0 {
                clock_100ns()? - frames as i64 * 10_000_000 / 48_000
            } else {
                qpc as i64
            };
            let sample = ((timestamp - epoch) as i128 * 48_000 / 10_000_000) as i64;
            let now = ((clock_100ns()? - epoch).max(0) as u64 * 48_000 / 10_000_000) + 48_000;
            if sample > now as i64 {
                bail!("Thiết bị âm thanh trả về mốc thời gian không hợp lệ.");
            }
            let skip = (-sample).max(0).min(frames as i64) as usize;
            if skip == frames as usize {
                continue;
            }
            let position = sample.max(0) as u64;
            // Tiny overlaps from clock rounding are removed; gaps remain zero-filled.
            let overlap = self
                .end_sample
                .saturating_sub(position)
                .min((frames as usize - skip) as u64) as usize;
            if skip + overlap >= frames as usize {
                continue;
            }
            let start = position + overlap as u64;
            self.file.seek(SeekFrom::Start(start * 8))?;
            self.file.write_all(&bytes[(skip + overlap) * 8..])?;
            self.end_sample = start + (frames as usize - skip - overlap) as u64;
        }
        Ok(())
    }
}

pub struct AudioSession {
    stop: Arc<AtomicBool>,
    start: Option<mpsc::Sender<i64>>,
    thread: Option<JoinHandle<Result<()>>>,
}
impl AudioSession {
    pub fn prepare(
        paths: Vec<(bool, PathBuf)>,
        prefs: crate::model::Preferences,
        live: Option<crate::studio::Shared>,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (start_tx, start_rx) = mpsc::channel();
        let thread = std::thread::spawn(move || unsafe {
            let result = (|| -> Result<()> {
                CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
                let _com = ComGuard;
                let mut inputs = Vec::new();
                for (mic, path) in &paths {
                    File::create(path)?;
                    if input_enabled(*mic, &prefs, &live) {
                        inputs.push(Input::new(
                            *mic,
                            path.clone(),
                            if *mic {
                                &prefs.mic_device
                            } else {
                                &prefs.system_device
                            },
                        )?);
                    }
                }
                let _ = ready_tx.send(Ok(()));
                let epoch = match start_rx.recv() {
                    Ok(e) => e,
                    Err(_) => return Ok(()),
                };
                for input in &inputs {
                    input.client.Start()?;
                }
                while !flag.load(Ordering::Relaxed) {
                    for (mic, path) in &paths {
                        if input_enabled(*mic, &prefs, &live)
                            && !inputs.iter().any(|i| i.mic == *mic)
                        {
                            let input = Input::new(
                                *mic,
                                path.clone(),
                                if *mic {
                                    &prefs.mic_device
                                } else {
                                    &prefs.system_device
                                },
                            )?;
                            input.client.Start()?;
                            inputs.push(input);
                        }
                    }
                    for input in &mut inputs {
                        let gain = input_gain(input.mic, &prefs, &live);
                        input.drain(epoch, gain, &live)?;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(4));
                }
                for input in &mut inputs {
                    let gain = input_gain(input.mic, &prefs, &live);
                    input.drain(epoch, gain, &live)?;
                    input.file.sync_all()?;
                }
                Ok(())
            })();
            if let Err(ref e) = result {
                let _ = ready_tx.send(Err(format!("{e:#}")));
            }
            result
        });
        match ready_rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(Self {
                stop,
                start: Some(start_tx),
                thread: Some(thread),
            }),
            other => {
                stop.store(true, Ordering::Relaxed);
                drop(start_tx);
                bail!("Không chuẩn bị được âm thanh: {other:?}")
            }
        }
    }
    pub fn start(&mut self, epoch: i64) -> Result<()> {
        self.start
            .take()
            .unwrap()
            .send(epoch)
            .context("Luồng âm thanh đã đóng")
    }
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| t.is_finished())
    }
    pub fn finish(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        self.start.take();
        if let Some(t) = self.thread.take() {
            t.join()
                .map_err(|_| anyhow::anyhow!("Luồng âm thanh bị lỗi"))??;
        }
        Ok(())
    }
}
impl Drop for AudioSession {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn input_gain(
    mic: bool,
    prefs: &crate::model::Preferences,
    live: &Option<crate::studio::Shared>,
) -> f32 {
    if let Some(l) = live {
        let l = l.lock().unwrap();
        if mic {
            if l.mic_on {
                l.mic_gain
            } else {
                0.0
            }
        } else if l.system_on {
            l.system_gain
        } else {
            0.0
        }
    } else if mic {
        prefs.mic_volume
    } else {
        prefs.system_volume
    }
}
unsafe fn endpoint(
    e: &IMMDeviceEnumerator,
    mic: bool,
    id: &str,
) -> windows::core::Result<IMMDevice> {
    if id.is_empty() {
        e.GetDefaultAudioEndpoint(if mic { eCapture } else { eRender }, eConsole)
    } else {
        let id: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        e.GetDevice(windows::core::PCWSTR(id.as_ptr()))
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
}
pub fn device_available(devices: &[Device], id: &str) -> bool {
    if id.is_empty() {
        !devices.is_empty()
    } else {
        devices.iter().any(|d| d.id == id)
    }
}

pub fn devices(mic: bool) -> Vec<Device> {
    std::thread::spawn(move || unsafe {
        let result = (|| -> Result<Vec<Device>> {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let _com = ComGuard;
            let e: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let list =
                e.EnumAudioEndpoints(if mic { eCapture } else { eRender }, DEVICE_STATE_ACTIVE)?;
            let mut devices = Vec::new();
            for i in 0..list.GetCount()? {
                let d = list.Item(i)?;
                let id = d.GetId()?;
                let value = id.to_string()?;
                CoTaskMemFree(Some(id.0.cast()));
                let store = d.OpenPropertyStore(STGM_READ)?;
                let name = store.GetValue(
                    &windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
                )?;
                let ptr = windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc(
                    &name,
                )?;
                let name = ptr.to_string()?;
                CoTaskMemFree(Some(ptr.0.cast()));
                devices.push(Device { id: value, name });
            }
            Ok(devices)
        })();
        result.unwrap_or_default()
    })
    .join()
    .unwrap_or_default()
}
/// Endpoint peak meters do not write or retain audio samples.
pub struct Meter(Arc<AtomicBool>);
impl Drop for Meter {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
pub fn meters(prefs: crate::model::Preferences, live: crate::studio::Shared) -> Meter {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || unsafe {
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return;
        }
        let _com = ComGuard;
        let Ok(e) =
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        else {
            return;
        };
        let mut meters = Vec::new();
        let mut refreshed = std::time::Instant::now() - std::time::Duration::from_secs(3);
        while !flag.load(Ordering::Relaxed) {
            if refreshed.elapsed() >= std::time::Duration::from_secs(2) {
                meters.clear();
                for (mic, id) in [(false, &prefs.system_device), (true, &prefs.mic_device)] {
                    if let Ok(d) = endpoint(&e, mic, id) {
                        if d.GetState().ok() == Some(DEVICE_STATE_ACTIVE) {
                            if let Ok(m) = d.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None) {
                                meters.push((usize::from(mic), m));
                            }
                        }
                    }
                }
                refreshed = std::time::Instant::now();
            }
            let mut peaks = [0.0; 2];
            for (i, m) in &meters {
                if let Ok(peak) = m.GetPeakValue() {
                    peaks[*i] = peak;
                }
            }
            {
                let mut l = live.lock().unwrap();
                if l.status == crate::studio::Status::Ready {
                    l.levels = peaks;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
    });
    Meter(stop)
}

fn input_enabled(
    mic: bool,
    prefs: &crate::model::Preferences,
    live: &Option<crate::studio::Shared>,
) -> bool {
    if let Some(l) = live {
        let l = l.lock().unwrap();
        if mic {
            l.mic_on
        } else {
            l.system_on
        }
    } else if mic {
        prefs.microphone
    } else {
        prefs.system_audio
    }
}

#[cfg(test)]
mod availability_tests {
    #[test]
    fn removed_explicit_endpoint_never_falls_back_to_default() {
        let devices = vec![super::Device {
            id: "connected".into(),
            name: "Speaker".into(),
        }];
        assert!(super::device_available(&devices, ""));
        assert!(super::device_available(&devices, "connected"));
        assert!(!super::device_available(&devices, "unplugged"));
        assert!(!super::device_available(&[], ""));
    }
}
