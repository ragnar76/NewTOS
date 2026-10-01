#!/usr/bin/env python3
"""A60 trace instrumentation -- applied to the SCRATCH copy only.

(1) tos-api/src/aes/mod.rs  : at every AES trap from an EL0 process with AES pid 1,
    dump the 32 bytes of the caller's AESGLOBAL before ("pre") and after ("post")
    the call when they differ from the last dumped state; always log ops 10/110/111/112
    with their in/out words and addrout[0]; also dump TeraDesk's statics
    svicntree/svicnrshdr/icons (fixed link VAs of DESKTOP.NAP) when they change.
(2) tos-api/src/host.rs     : debug_write -> console_write_bytes (serial).
(3) tos-api/src/gemdos/memory.rs : log Malloc(amount) -> addr for EL0 callers.
"""
import sys, re, pathlib
ws = pathlib.Path(sys.argv[1])

# ---- (2) host.rs ------------------------------------------------------------
p = ws / "tos-api/src/host.rs"
s = p.read_text()
anchor = "    /// Non-blocking read from the console input device. Returns\n"
assert anchor in s
s = s.replace(anchor, "    /// A60-TEMP: serial-only debug write (audit scaffolding, scratch copy only).\n    fn debug_write(&self, s: &[u8]) {\n        self.console_write_bytes(s)\n    }\n\n" + anchor, 1)
p.write_text(s)

# ---- (1) aes/mod.rs ---------------------------------------------------------
p = ws / "tos-api/src/aes/mod.rs"
s = p.read_text()
old = "    let t0 = trace::call_begin();\n\n    let sw = crysbind(&pb);\n"
assert old in s
new = """    let t0 = trace::call_begin();

    // A60-TEMP: snapshot the caller's AESGLOBAL before the call.
    let a60_on = a60_active();
    let a60_pre = if a60_on { unsafe { a60_read32(pb.global) } } else { [0u8; 32] };
    if a60_on { a60_pre_log(&pb, &a60_pre); }

    let sw = crysbind(&pb);

    // A60-TEMP: and after it (post copy-back happens below; the global is
    // written by the handler itself, so it is final here).
    if a60_on { a60_post_log(&pb, &a60_pre, sw.is_some()); }
"""
s = s.replace(old, new, 1)
s += r'''
// A60-TEMP BEGIN ------------------------------------------------------------
static mut A60_LAST: [u8; 32] = [0u8; 32];
static mut A60_SEEN: bool = false;
static mut A60_STATICS_LAST: [u64; 4] = [0; 4];

/// TeraDesk DESKTOP.NAP link VAs (llvm-nm, A60-fix build of 2026-09-19, sha256 fb37bb3b...):
///   aes_global 0x3259c  svicntree 0x30f70  svicnrshdr 0x30f78  icons 0x30ee0
///   colour_icons 0x3116c
const A60_SVICNTREE: u64 = 0x30f70;
const A60_SVICNRSHDR: u64 = 0x30f78;
const A60_ICONS: u64 = 0x30ee0;
const A60_COLOUR_ICONS: u64 = 0x3116c;

pub fn a60_pid() -> i16 { appl::current_pid() }

fn a60_active() -> bool {
    !crate::bdos::process::current().is_null() && appl::current_pid() == 1
}

unsafe fn a60_read32(g: *const i16) -> [u8; 32] {
    let mut b = [0u8; 32];
    if g.is_null() { return b; }
    unsafe { core::ptr::copy_nonoverlapping(g as *const u8, b.as_mut_ptr(), 32) };
    b
}

pub fn a60_log(args: core::fmt::Arguments) {
    struct W;
    impl core::fmt::Write for W {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            if let Some(h) = crate::host::host() { h.debug_write(s.as_bytes()); }
            Ok(())
        }
    }
    use core::fmt::Write as _;
    let mut w = W;
    let _ = w.write_fmt(args);
    let _ = w.write_str("\r\n");
}

fn a60_hex(tag: &str, op: i16, g: u64, b: &[u8; 32]) {
    struct W;
    impl core::fmt::Write for W {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            if let Some(h) = crate::host::host() { h.debug_write(s.as_bytes()); }
            Ok(())
        }
    }
    use core::fmt::Write as _;
    let mut w = W;
    let _ = write!(w, "[a60] {} op={} global=0x{:x} bytes=", tag, op, g);
    for (i, x) in b.iter().enumerate() {
        if i > 0 && i % 2 == 0 { let _ = w.write_str(" "); }
        let _ = write!(w, "{:02x}", x);
    }
    let _ = w.write_str("\r\n");
}

fn a60_statics(tag: &str, op: i16) {
    // Read TeraDesk's own statics through the live TTBR0 window (pid 1 = slot 0).
    let vals = unsafe {[
        (A60_SVICNTREE as *const u64).read_unaligned(),
        (A60_SVICNRSHDR as *const u64).read_unaligned(),
        (A60_ICONS as *const u64).read_unaligned(),
        (A60_COLOUR_ICONS as *const u16).read_unaligned() as u64,
    ]};
    let changed = unsafe { vals != A60_STATICS_LAST };
    if changed {
        unsafe { A60_STATICS_LAST = vals };
        a60_log(format_args!("[a60] {} op={} statics svicntree=0x{:x} svicnrshdr=0x{:x} icons=0x{:x} colour_icons={}",
            tag, op, vals[0], vals[1], vals[2], vals[3]));
    }
}

fn a60_pre_log(pb: &Aespb, pre: &[u8; 32]) {
    let op = pb.control(C_OPCODE);
    let g = pb.global as u64;
    let changed = unsafe { !A60_SEEN || *pre != A60_LAST };
    if changed {
        a60_hex("pre ", op, g, pre);
        unsafe { A60_LAST = *pre; A60_SEEN = true; }
    }
    a60_statics("pre ", op);
    match op {
        110 => {
            // rsrc_load: addrin[0] = filename (user VA, mapped)
            let s = pb.addrin(0) as *const u8;
            let mut name = [0u8; 32];
            let mut n = 0;
            unsafe { while n < 31 { let c = *s.add(n); if c == 0 { break; } name[n] = c; n += 1; } }
            a60_log(format_args!("[a60] call rsrc_load(\"{}\")", core::str::from_utf8(&name[..n]).unwrap_or("?")));
        }
        111 => a60_log(format_args!("[a60] call rsrc_free()")),
        112 => a60_log(format_args!("[a60] call rsrc_gaddr(type={}, idx={})", pb.intin(0), pb.intin(1))),
        10 => a60_log(format_args!("[a60] call appl_init()")),
        124 => {
            let s = pb.addrin(0) as *const u8;
            let mut name = [0u8; 64];
            let mut n = 0;
            unsafe { while n < 63 { let c = *s.add(n); if c == 0 { break; } name[n] = c; n += 1; } }
            a60_log(format_args!("[a60] call shel_find(\"{}\")", core::str::from_utf8(&name[..n]).unwrap_or("?")));
        }
        _ => {}
    }
}

fn a60_post_log(pb: &Aespb, pre: &[u8; 32], yielded: bool) {
    let op = pb.control(C_OPCODE);
    let g = pb.global as u64;
    let post = unsafe { a60_read32(pb.global) };
    if post != *pre {
        a60_hex("post", op, g, &post);
        unsafe { A60_LAST = post; A60_SEEN = true; }
    }
    let ret = unsafe { *pb.intout };
    match op {
        10 | 110 | 111 => a60_log(format_args!("[a60] ret  op={} int_out[0]={} yield={}", op, ret, yielded)),
        112 => {
            let ao = unsafe { pb.abi.read_ptr(pb.addrout, 0) };
            a60_log(format_args!("[a60] ret  op=112 rsrc_gaddr(type={}, idx={}) int_out[0]={} addrout[0]=0x{:x}", pb.intin(0), pb.intin(1), ret, ao));
        }
        124 => a60_log(format_args!("[a60] ret  op=124 shel_find int_out[0]={}", ret)),
        _ => {}
    }
    a60_statics("post", op);
}
// A60-TEMP END --------------------------------------------------------------
'''
p.write_text(s)

# ---- (3) gemdos/memory.rs ---------------------------------------------------
p = ws / "tos-api/src/gemdos/memory.rs"
s = p.read_text()
old = "    // SAFETY: BDOS lock held; `slot` is the running EL0 process.\n    unsafe { el0_alloc(slot, amount, mode) }\n}\n"
i = s.index("pub fn malloc(args: &TrapArgs) -> Long {")
j = s.index(old, i)
new = """    // SAFETY: BDOS lock held; `slot` is the running EL0 process.
    let r = unsafe { el0_alloc(slot, amount, mode) };
    // A60-TEMP
    if crate::aes::a60_pid() == 1 {
        crate::aes::a60_log(format_args!("[a60] Malloc({}) -> 0x{:x}", amount, r));
    }
    r
}
"""
s = s[:j] + new + s[j+len(old):]
p.write_text(s)
print("patched")
