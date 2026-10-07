#!/usr/bin/env python3
"""zvprof_patch.py <tree> -- instrument a SCRATCH COPY of the NewTOS tree
(base desk-rebuild 0b2e05a) with the ZVPROF profiler (zvprof.rs next to this
script). Never run on the shared tree. Every anchor must match exactly once."""
import pathlib, re, shutil, sys

T = pathlib.Path(sys.argv[1])
HERE = pathlib.Path(__file__).resolve().parent
assert (T / "tos-api/src/lib.rs").exists()
assert "scratchpad" in str(T.resolve()), "refusing to patch a tree outside the scratchpad"


def sub(rel, old, new, count=1):
    p = T / rel
    s = p.read_text()
    n = s.count(old)
    assert n == count, f"{rel}: anchor found {n}x, want {count}: {old[:80]!r}"
    p.write_text(s.replace(old, new))


shutil.copy(HERE / "zvprof.rs", T / "tos-api/src/zvprof.rs")
sub("tos-api/src/lib.rs", "pub mod cookie;\n", "pub mod cookie;\npub mod zvprof; // ZVPROF-TEMP\n")

# ---- trap entry / exit, memprot sync, trap-tail flush, Pexec, Pterm
X = "kernel/src/arch/aarch64/exceptions.rs"
sub(X,
    "pub extern \"C\" fn rust_sync_handler(frame: &mut ExceptionFrame) {\n    sync_handler(frame);\n",
    "pub extern \"C\" fn rust_sync_handler(frame: &mut ExceptionFrame) {\n"
    "    let zp_outer = tos_api::zvprof::trap_enter(frame.spsr, frame.esr, frame.gpr[0], frame.gpr[8]);\n"
    "    sync_handler(frame);\n")
# memprot: every sync (trap tail AND the direct calls, e.g. in Mshrink) and every refresh
M = "tos-api/src/bdos/process/memprot.rs"
sub(M,
    "pub unsafe fn sync() {\n",
    "pub unsafe fn sync() {\n"
    "    let zp = crate::zvprof::now();\n"
    "    unsafe { sync_zp() };\n"
    "    crate::zvprof::mp_done(zp);\n}\n"
    "unsafe fn sync_zp() {\n")
sub(M,
    "unsafe fn refresh(lo: u64, hi: u64, full: u32) {\n",
    "unsafe fn refresh(lo: u64, hi: u64, full: u32) {\n"
    "    let zp = crate::zvprof::now();\n"
    "    unsafe { refresh_zp(lo, hi, full) };\n"
    "    crate::zvprof::rf_done(zp, lo, hi);\n}\n"
    "unsafe fn refresh_zp(lo: u64, hi: u64, full: u32) {\n")
sub(X,
    "            r.total,\n        );\n    }\n}\n\n/// The body of [`rust_sync_handler`].\n",
    "            r.total,\n        );\n    }\n"
    "    tos_api::zvprof::trap_exit(zp_outer, frame.spsr);\n}\n\n/// The body of [`rust_sync_handler`].\n")
sub(X,
    "    tos_api::gemdos::trace::flush_if_requested();\n",
    "    tos_api::gemdos::trace::flush_if_requested();\n    tos_api::zvprof::maybe_flush_tail();\n")
sub(X,
    "        if func == 0x4B {\n            handle_pexec(frame);\n            return;\n        }\n",
    "        if func == 0x4B {\n"
    "            tos_api::zvprof::px_begin(frame.gpr[0] as u16);\n"
    "            handle_pexec(frame);\n"
    "            tos_api::zvprof::px_done();\n"
    "            return;\n        }\n")
sub(X,
    "    let path_bytes = &path_buf[..path_len];\n",
    "    let path_bytes = &path_buf[..path_len];\n    tos_api::zvprof::px_name(path_bytes);\n")
sub(X,
    "fn handle_pterm(frame: &mut ExceptionFrame, func: u16) {\n",
    "fn handle_pterm(frame: &mut ExceptionFrame, func: u16) {\n    tos_api::zvprof::on_pterm(func);\n")

# ---- IRQ time
G = "kernel/src/drivers/gic.rs"
sub(G,
    "    let gic = board_gic();\n    let intid = gic.acknowledge();\n",
    "    let zp_irq = tos_api::zvprof::now();\n    let gic = board_gic();\n    let intid = gic.acknowledge();\n")
sub(G,
    "    gic.end_of_interrupt(intid);\n}\n",
    "    gic.end_of_interrupt(intid);\n    tos_api::zvprof::irq_done(zp_irq);\n}\n")

# ---- AES wait = sched::sleep
S = "tos-api/src/bdos/process/sched.rs"
sub(S,
    "pub fn sleep(cond: u64) -> SleepOutcome {\n",
    "pub fn sleep(cond: u64) -> SleepOutcome {\n"
    "    let zp = crate::zvprof::sleep_begin();\n"
    "    let r = sleep_zp(cond);\n"
    "    crate::zvprof::sleep_end(zp);\n"
    "    r\n}\n"
    "fn sleep_zp(cond: u64) -> SleepOutcome {\n")

# ---- input pump (+ serial writer for the report)
H = "kernel/src/host_env.rs"
sub(H, "    fn pump_input(&self) {\n", "    fn pump_input(&self) {\n        let zp_pump = tos_api::zvprof::pump_begin();\n")
sub(H,
    "        crate::ikbd::poll_repeat();\n        core::hint::spin_loop();\n",
    "        crate::ikbd::poll_repeat();\n        core::hint::spin_loop();\n        tos_api::zvprof::pump_end(zp_pump);\n")
sub(H,
    "pub fn install() {\n",
    "fn zvprof_serial(b: &[u8]) {\n"
    "    use crate::hal::uart::Uart;\n"
    "    for &x in b {\n"
    "        crate::console::CONSOLE.write_byte(x);\n"
    "    }\n"
    "}\n\n"
    "pub fn install() {\n"
    "    tos_api::zvprof::SERIAL_OUT.store(zvprof_serial as fn(&[u8]) as usize, core::sync::atomic::Ordering::Relaxed);\n")

# ---- opcodes
sub("tos-api/src/vdi/mod.rs",
    "    let opcode = pb.contrl(C_OPCODE);\n",
    "    let opcode = pb.contrl(C_OPCODE);\n    crate::zvprof::set_op(crate::zvprof::C_VDI, (opcode as u16 as usize).min(255));\n")
sub("tos-api/src/aes/mod.rs",
    "    let opcode = pb.control(C_OPCODE);\n\n    match opcode {\n",
    "    let opcode = pb.control(C_OPCODE);\n    crate::zvprof::set_op(crate::zvprof::C_AES, (opcode as u16 as usize).min(255));\n\n    match opcode {\n")

# ---- clicks
sub("tos-api/src/aes/input.rs",
    "pub(crate) fn b_click(new: u8) {\n    b_delay_poll();\n    // \"ignore it unless it represents a change\" (aes/geminput.c:112-113)\n"
    "    if new == GL_BTRUE.load(Ordering::Relaxed) {\n        return;\n    }\n",
    "pub(crate) fn b_click(new: u8) {\n    b_delay_poll();\n    // \"ignore it unless it represents a change\" (aes/geminput.c:112-113)\n"
    "    if new == GL_BTRUE.load(Ordering::Relaxed) {\n        return;\n    }\n"
    "    if new != 0 {\n        crate::zvprof::press();\n    }\n")
sub("tos-api/src/aes/event.rs",
    "    fill_rets(pb);\n    pb.set_intout(5, key); // EV_KRET\n",
    "    crate::zvprof::evm_ret(fired, clicks);\n    fill_rets(pb);\n    pb.set_intout(5, key); // EV_KRET\n")

# ---- Pexec loader phases
L = "tos-api/src/bdos/process/loader.rs"
sub(L,
    "    let env_addr = unsafe { alloc_env_for(env, prgflags) }?;\n",
    "    let zp_alloc = crate::zvprof::now();\n    let env_addr = unsafe { alloc_env_for(env, prgflags) }?;\n")
sub(L,
    "        free_env();\n        return Err(LoadError::NoTpaMemory);\n    };\n    // The TPA takes the program's protection mode",
    "        free_env();\n        return Err(LoadError::NoTpaMemory);\n    };\n    crate::zvprof::px_phase(0, zp_alloc);\n    // The TPA takes the program's protection mode")
sub(L,
    "    tpa_bytes[..image_end as usize].fill(0);\n    if prgflags & PF_FASTLOAD == 0 {\n        tpa_bytes[image_end as usize..].fill(0);\n    }\n",
    "    let zp_z = crate::zvprof::now();\n"
    "    tpa_bytes[..image_end as usize].fill(0);\n    if prgflags & PF_FASTLOAD == 0 {\n        tpa_bytes[image_end as usize..].fill(0);\n"
    "        crate::zvprof::px_zero(tpa.len - image_end, tpa.len);\n    }\n"
    "    crate::zvprof::px_zero(image_end, tpa.len);\n"
    "    crate::zvprof::px_phase(1, zp_z);\n")
sub(L,
    "    let mut image_top = BASEPAGE_RESERVED;\n",
    "    let zp_r = crate::zvprof::now();\n    let mut image_top = BASEPAGE_RESERVED;\n")
sub(L,
    "    reloc::relocate(tpa_bytes, img, base)?;\n",
    "    crate::zvprof::px_phase(2, zp_r);\n    let zp_rl = crate::zvprof::now();\n"
    "    reloc::relocate(tpa_bytes, img, base)?;\n    crate::zvprof::px_phase(3, zp_rl);\n")
sub(L,
    "    populate_basepage(tpa_bytes, img, tail, env_addr, base, hitpa, prgflags);\n",
    "    let zp_bp = crate::zvprof::now();\n"
    "    populate_basepage(tpa_bytes, img, tail, env_addr, base, hitpa, prgflags);\n"
    "    crate::zvprof::px_phase(4, zp_bp);\n")
sub(L,
    "    unsafe { mmu::sync_code_cache(tpa.base_kva as usize, image_top as usize) };\n",
    "    let zp_ic = crate::zvprof::now();\n"
    "    unsafe { mmu::sync_code_cache(tpa.base_kva as usize, image_top as usize) };\n"
    "    crate::zvprof::px_phase(5, zp_ic);\n")
sub(L,
    "    let l0_pa = unsafe { install_slot(slot, tpa, entry_va, sp_el0)? };\n",
    "    let zp_is = crate::zvprof::now();\n"
    "    let l0_pa = unsafe { install_slot(slot, tpa, entry_va, sp_el0)? };\n"
    "    crate::zvprof::px_phase(6, zp_is);\n")

# ---- backend primitives
B = T / "kernel/src/vdi_backend.rs"
s = B.read_text()
guards = [
    ("fill_rect", "P_FILL"), ("fill_pattern", "P_PATTERN"), ("abline", "P_ABLINE"),
    ("draw_glyph_sized", "P_GLYPH"), ("blit_mono", "P_BLITMONO"), ("copy_rect", "P_COPYRECT"),
    ("read_row_rgb565", "P_READROW"), ("write_row_rgb565", "P_WRITEROW"), ("flush_rect", "P_FLUSHRECT"),
    ("clean", "P_CLEAN"),
]
for name, k in guards:
    pat = f"    fn {name}("
    assert s.count(pat) == 1, f"vdi_backend: {pat!r} found {s.count(pat)}x"
    i = s.index(pat)
    j = s.index("{\n", i)
    s = s[: j + 2] + f"        let _zp = tos_api::zvprof::P::new(tos_api::zvprof::{k}); // ZVPROF-TEMP\n" + s[j + 2:]
for name, k in [("put_device_pixel", "P_PUTPIX"), ("get_device_pixel", "P_GETPIX")]:
    pat = f"    fn {name}("
    assert s.count(pat) == 1
    i = s.index(pat)
    j = s.index("{\n", i)
    s = s[: j + 2] + f"        tos_api::zvprof::prim_cnt(tos_api::zvprof::{k}); // ZVPROF-TEMP\n" + s[j + 2:]
B.write_text(s)

V = "kernel/src/vdi_backend.rs"
sub(V,
    "        fb.fill_rect(x0 as u32, y0 as u32, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32, Self::pel_to_rgb(pel));\n",
    "        tos_api::zvprof::prim_px(tos_api::zvprof::P_FILL, ((x1 - x0 + 1) as u64) * ((y1 - y0 + 1) as u64));\n"
    "        fb.fill_rect(x0 as u32, y0 as u32, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32, Self::pel_to_rgb(pel));\n")
sub(V,
    "        let Some((tx0, ty0, tx1, ty1)) = dbox.clamp_to(scr_w as u16, scr_h as u16) else {\n            return;\n        };\n",
    "        let Some((tx0, ty0, tx1, ty1)) = dbox.clamp_to(scr_w as u16, scr_h as u16) else {\n            return;\n        };\n"
    "        tos_api::zvprof::prim_px(tos_api::zvprof::P_COPYRECT, ((tx1 - tx0 + 1) as u64) * ((ty1 - ty0 + 1) as u64));\n")
sub(V,
    "        let rows = (y1 - y0 + 1) as usize;\n",
    "        let rows = (y1 - y0 + 1) as usize;\n        tos_api::zvprof::prim_px(tos_api::zvprof::P_CLEAN, (row_bytes * rows) as u64);\n")
s = (T / V).read_text()
for name, k, expr in [
    ("read_row_rgb565", "P_READROW", "out.len() as u64"),
    ("write_row_rgb565", "P_WRITEROW", "src.len() as u64"),
    ("blit_mono", "P_BLITMONO", "(dst.w.max(0) as u64) * (dst.h.max(0) as u64)"),
    ("fill_pattern", "P_PATTERN", "(w.max(0) as u64) * (h.max(0) as u64)"),
]:
    g = f"        let _zp = tos_api::zvprof::P::new(tos_api::zvprof::{k}); // ZVPROF-TEMP\n"
    assert s.count(g) == 1
    s = s.replace(g, g + f"        tos_api::zvprof::prim_px(tos_api::zvprof::{k}, {expr});\n")
(T / V).write_text(s)
print("zvprof applied to", T)
