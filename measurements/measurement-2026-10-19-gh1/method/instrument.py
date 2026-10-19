#!/usr/bin/env python3
"""instrument.py <scratch-tree> -- GH1 serial instrumentation, applied to a SCRATCH copy only (never the workspace).
Adds a log hook tos_api::GH1_LOG (set by the kernel to a kprintln shim) and "[gh1]" lines at:
  aes dispatch_in   every completed AES call except evnt_multi(25) yields: slot/pid, opcode, int_in[0..4], done/yield; evnt_multi int_out
  pd::post_msg      every message posted: target identity, msg[0..8]
  window.rs         compose_except (clip), draw_frame_pieces (handle, pc), update_beg/update_end (count, owner)
Each anchor must occur exactly once."""
import pathlib, sys
T = pathlib.Path(sys.argv[1])
def sub(rel, old, new):
    p = T / rel; s = p.read_text()
    if s.count(old) != 1: sys.exit(f"anchor x{s.count(old)} in {rel}: {old[:60]!r}")
    p.write_text(s.replace(old, new))
p = T / "tos-api/src/lib.rs"
p.write_text(p.read_text() + '''
// GH1-TEMP scratch instrumentation
#[doc(hidden)]
pub static mut GH1_LOG: Option<fn(core::fmt::Arguments)> = None;
#[doc(hidden)]
#[macro_export]
macro_rules! gh1 { ($($a:tt)*) => {{ #[allow(unused_unsafe)] unsafe { if let Some(f) = $crate::GH1_LOG { f(format_args!($($a)*)); } } }}; }
''')
sub("kernel/src/lib.rs", '    kprintln!("Phase 02 reached.");',
    '    kprintln!("Phase 02 reached.");\n    unsafe { tos_api::GH1_LOG = Some(|a| { kprintln!("[gh1] {}", a); }); } // GH1-TEMP')
sub("tos-api/src/aes/mod.rs", "    let sw = crysbind(&pb);\n",
    "    let sw = crysbind(&pb);\n"
    "    if sw.is_none() || pb.control(C_OPCODE) != 25 { crate::gh1!(\"call id={} pid={} op={} in=[{} {} {} {}] {}\", appl::current_identity(), appl::current_pid(), pb.control(C_OPCODE), pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3), if sw.is_none() {\"done\"} else {\"yield\"}); if sw.is_none() && pb.control(C_OPCODE) == 25 { crate::gh1!(\"  evnt_multi id={} out=[which={:#x} mx={} my={} mb={} ks={} key={:#x} bret={}]\", appl::current_identity(), local_intout[0], local_intout[1], local_intout[2], local_intout[3], local_intout[4], local_intout[5], local_intout[6]); } } // GH1-TEMP\n")
sub("tos-api/src/aes/pd.rs", "    let posted = match unsafe { by_index(idx) } {\n        Some(p) => p.queue.push(msg),",
    "    crate::gh1!(\"post to={} msg={:?}\", idx, msg); // GH1-TEMP\n    let posted = match unsafe { by_index(idx) } {\n        Some(p) => p.queue.push(msg),")
sub("tos-api/src/aes/window.rs", "    crate::vdi::cursor_hide(backend);\n    paint_bg_clip(backend, clip);\n",
    "    crate::gh1!(\"compose clip=({},{})-({},{}) skip={:?}\", clip.xmn, clip.ymn, clip.xmx, clip.ymx, skip); // GH1-TEMP\n    crate::vdi::cursor_hide(backend);\n    paint_bg_clip(backend, clip);\n")
sub("tos-api/src/aes/window.rs", "    let n = walk_rects(handle, pc.map(plus_shadow), &mut pieces);\n    if n == 0 {\n        return;\n    }\n",
    "    let n = walk_rects(handle, pc.map(plus_shadow), &mut pieces);\n    crate::gh1!(\"frame h={} top={} pc={:?} pieces={}\", handle, istop, pc.map(|g| (g.x, g.y, g.w, g.h)), n); // GH1-TEMP\n    if n == 0 {\n        return;\n    }\n")
sub("tos-api/src/aes/window.rs", "    let tas = UPDATE_LOCK.fetch_add(1, Ordering::Relaxed) + 1;\n",
    "    let tas = UPDATE_LOCK.fetch_add(1, Ordering::Relaxed) + 1;\n    crate::gh1!(\"BEG_UPDATE by id={} -> tas={} owner_was={}\", rlr, tas, UPDATE_OWNER.load(Ordering::Relaxed) as isize); // GH1-TEMP\n")
sub("tos-api/src/aes/window.rs", "    let v = UPDATE_LOCK.load(Ordering::Relaxed);\n    if v > 0 {\n        UPDATE_LOCK.store(v - 1, Ordering::Relaxed);",
    "    let v = UPDATE_LOCK.load(Ordering::Relaxed);\n    crate::gh1!(\"END_UPDATE by id={} tas_was={}\", super::appl::current_identity(), v); // GH1-TEMP\n    if v > 0 {\n        UPDATE_LOCK.store(v - 1, Ordering::Relaxed);")

# --- button-delivery instrumentation (event.rs / input.rs) ---
ev = "tos-api/src/aes/event.rs"; inp = "tos-api/src/aes/input.rs"
sub(ev, "                            clicks = pr_mclick;\n                            fired |= MU_BUTTON;\n",
    "                            clicks = pr_mclick;\n                            fired |= MU_BUTTON;\n                            crate::gh1!(\"BTN id={} quick pr_button pass={}\", idx, pass); // GH1-TEMP\n")
sub(ev, "                            clicks = arm_mclick;\n                            fired |= MU_BUTTON;\n",
    "                            clicks = arm_mclick;\n                            fired |= MU_BUTTON;\n                            crate::gh1!(\"BTN id={} quick level pass={}\", idx, pass); // GH1-TEMP\n")
sub(ev, "                    clicks = c; // apret(MU_BUTTON) (aes/gemevlib.c:276-277)\n                    fired |= MU_BUTTON;\n",
    "                    clicks = c; // apret(MU_BUTTON) (aes/gemevlib.c:276-277)\n                    fired |= MU_BUTTON;\n                    crate::gh1!(\"BTN id={} parked wait completed (post_button) pass={}\", idx, pass); // GH1-TEMP\n")
sub(ev, "                    if let Some(c) = got {\n                        clicks = c;\n                        fired |= MU_BUTTON;\n",
    "                    if let Some(c) = got {\n                        clicks = c;\n                        fired |= MU_BUTTON;\n                        crate::gh1!(\"BTN id={} arm abutton/bwait_take pass={}\", idx, pass); // GH1-TEMP\n")
sub(inp, "        if downorup(new, bflgclks, (p >> 8) as u8, p as u8) {\n            let clicks = i16::from(bflgclks as u8);\n",
    "        if downorup(new, bflgclks, (p >> 8) as u8, p as u8) {\n            crate::gh1!(\"post_button new={} completes wait #{} (mowner_pd={} app={})\", new, BWAIT.iter().position(|x| core::ptr::eq(x, w)).unwrap_or(99), GL_MOWNER_PD.load(Ordering::Relaxed) as isize, GL_MOWNER_APP.load(Ordering::Relaxed)); // GH1-TEMP\n            let clicks = i16::from(bflgclks as u8);\n")
sub(inp, "    let button = BUTTON.load(Ordering::Relaxed);\n    if button == 0 {\n        match owner {\n",
    "    let button = BUTTON.load(Ordering::Relaxed);\n    if button == 0 { crate::gh1!(\"set_mown -> {}\", match owner { Mown::Ctl => -1isize, Mown::Pd(p) => p as isize }); } // GH1-TEMP\n    if button == 0 {\n        match owner {\n")
print("instrumented", T)
