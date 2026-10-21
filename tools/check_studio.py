import array,json,pathlib,subprocess,winsound,wave,math
root=pathlib.Path(__file__).resolve().parents[1]
tone=root/'test-output/test-tone-v2.wav'
with wave.open(str(tone),'wb') as f:
    f.setparams((2,2,48000,0,'NONE','not compressed'))
    f.writeframes(array.array('h',(int(1100*math.sin(2*math.pi*440*(i//2)/48000)) for i in range(48000*2*8))).tobytes())
proc=subprocess.Popen(['cargo','run','--release','--locked','--example','check_studio','--','--audio','--mic','--inspect-ui'],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,encoding='utf-8')
for line in proc.stdout:
    if line.startswith('Recording'):
        winsound.PlaySound(str(tone),winsound.SND_FILENAME|winsound.SND_ASYNC|winsound.SND_LOOP)
    if not line.startswith('Progress'):print(line.strip(),flush=True)
proc.wait();winsound.PlaySound(None,0)
assert proc.returncode==0
folder=root/'test-output/studio-v2'
result=json.loads((folder/'acceptance.json').read_text())
video=result['video']
ffmpeg=str(root/'vendor/ffmpeg.exe');ffprobe=str(root/'vendor/ffprobe.exe')
probe=json.loads(subprocess.check_output([ffprobe,'-v','error','-count_frames','-show_streams','-show_format','-of','json',video]))
v=next(s for s in probe['streams'] if s['codec_type']=='video')
assert v['r_frame_rate']=='30/1' and int(v['nb_read_frames'])>=270
p=subprocess.run([ffmpeg,'-v','error','-i',video,'-f','null','-'],capture_output=True)
assert p.returncode==0 and not p.stderr.strip(),p.stderr
raw=subprocess.check_output([ffmpeg,'-v','error','-i',video,'-vn','-ac','1','-ar','48000','-f','f32le','pipe:1'])
samples=array.array('f',raw)
def peak(a,b):return max(abs(x) for x in samples[int(a*48000):int(b*48000)])
peaks={'before_mute':peak(.5,1.5),'muted':peak(2.5,3.5),'after_unmute':peak(4.5,5)}
assert peaks['before_mute']>.01 and peaks['after_unmute']>.01,peaks
assert peaks['muted']<.0001,peaks
subprocess.run([ffmpeg,'-v','error','-y','-ss','5','-i',video,'-frames:v','1',str(folder/'video-annotations.png')],check=True)
result.update(audio_peaks=peaks,decode_ok=True,frames=int(v['nb_read_frames']),duration=float(probe['format']['duration']),microphone_enabled_during_recording=True)
(folder/'acceptance.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result,indent=2),flush=True)
