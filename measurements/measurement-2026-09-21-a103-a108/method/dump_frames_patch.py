# Scratch-only: append a test to aes_window_color_words.rs that writes the
# default_frames raster (device pixels) to a file named by env var.
import sys
p=sys.argv[1]
s=open(p).read()
s += '''
#[test]
fn zz_dump_default_frames_raster() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (host, mut pb) = fresh();
    default_frames(&mut pb);
    let px = host.raster.px.lock().unwrap().clone();
    let path = std::env::var("FRAME_DUMP").expect("FRAME_DUMP");
    let mut bytes = Vec::with_capacity(px.len() * 4);
    for v in px { bytes.extend_from_slice(&v.to_le_bytes()); }
    std::fs::write(path, bytes).unwrap();
}
'''
open(p,'w').write(s)
