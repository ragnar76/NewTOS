#!/usr/bin/env python3
"""apply-instrumentation.py <scratch-tree> <dir-with-CRCHILD.NAP-and-CRASHTST.NAP>

Scratch-only instrumentation for the ARCH03 0c crash runs. NEVER applied to
the shared tree; the product code under test (exceptions.rs, crash.rs,
uaccess.rs, console.rs) keeps its logic -- only an input source and two
launch calls are added.

board-rpi4 (QEMU raspi4b has no USB keyboard and drops serial RX on this
board profile):
  * a scripted IKBD injector after every usb::poll() (pump_input and both
    blocking read loops; the crash prompt's wait polls through pump_input),
    recipe of measurement-2026-10-04-keys02;
  * it answers the crash prompt: once the prompt has been waiting >= 400
    ticks (2 s, so a screendump can see it) it feeds Space make+break;
  * once the program is gone (RUN null for 800 ticks after having been set)
    it feeds Alt-C once (desktop: open drive C) -- the "desktop still usable"
    check;
  * crash.rs gets one scratch static, WAITING = hz_200 at the prompt (0 =
    not waiting), so the injector knows when the prompt is up.

board-qemu-virt (no SD card on this board): the two test programs are put
into the RAM program registry and launched from the boot path at the AUTO
folder's position, before the desktop -- first CRCHILD with no tail (case
'w', parent = the kernel launcher, like the desktop's #Z launch), then
CRASHTST (EL0 parent, six crashing children + one clean child). The crash
prompt is answered over the serial line by the host driver (qemu-virt is the
CONF_SERIAL_CONSOLE profile: serial RX = console keys).
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])
progs = pathlib.Path(sys.argv[2]).resolve()

# --- crash.rs: the WAITING marker ------------------------------------------
cr = t / "kernel/src/arch/aarch64/crash.rs"
s = cr.read_text()
old = """    // `bconin2();`
    loop {"""
assert s.count(old) == 1, "crash.rs wait anchor"
s = s.replace(old, """    // `bconin2();`
    WAITING.store(tos_api::bios::tick::hz_200().max(1), core::sync::atomic::Ordering::Relaxed); // C0C-TEMP
    loop {""")
old = """    // `cprintf("\\n");`
    let _ = w.write_str("\\n");"""
assert s.count(old) == 1, "crash.rs end anchor"
s = s.replace(old, """    WAITING.store(0, core::sync::atomic::Ordering::Relaxed); // C0C-TEMP
    // `cprintf("\\n");`
    let _ = w.write_str("\\n");""")
s += """
/// C0C-TEMP: hz_200 when the crash prompt started waiting, 0 = not waiting.
pub static WAITING: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
"""
cr.write_text(s)

# --- host_env.rs: the injector (board-rpi4 only) ----------------------------
he = t / "kernel/src/host_env.rs"
s = he.read_text()
anchor = "crate::drivers::usb::poll();\n"
n = s.count(anchor)
assert n == 2, f"host_env.rs: expected 2 usb::poll sites, found {n}"  # A13: was 3 before pump_devices()
s = s.replace(anchor, "crate::drivers::usb::poll();\n        #[cfg(feature = \"board-rpi4\")] c0c_inject::step(); // C0C-TEMP\n", 1)
s += '''
// C0C-TEMP BEGIN -- scripted IKBD keys (QEMU raspi4b has no USB keyboard)
#[cfg(feature = "board-rpi4")]
pub mod c0c_inject {
    use core::sync::atomic::{AtomicU32, Ordering};
    static ARMED: AtomicU32 = AtomicU32::new(0);
    static LAST_RUN: AtomicU32 = AtomicU32::new(0);
    static ALTC: AtomicU32 = AtomicU32::new(0); // next Alt-C byte index
    static ALTC_AT: AtomicU32 = AtomicU32::new(0);
    const ALT_C: [u8; 4] = [0x38, 0x2E, 0xAE, 0xB8];
    pub fn step() {
        let now = tos_api::bios::tick::hz_200();
        let run = !tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null();
        if run {
            ARMED.store(1, Ordering::Relaxed);
            LAST_RUN.store(now, Ordering::Relaxed);
        }
        if ARMED.load(Ordering::Relaxed) == 0 {
            return;
        }
        let w = crate::arch::aarch64::crash::WAITING.load(Ordering::Relaxed);
        if w != 0 && now.wrapping_sub(w) >= 400 {
            crate::kprintln!("[c0c] inject Space at hz_200 {} (prompt waited {})", now, now.wrapping_sub(w));
            crate::arch::aarch64::crash::WAITING.store(0, Ordering::Relaxed);
            crate::ikbd::feed(0x39);
            crate::ikbd::feed(0xB9);
            return;
        }
        let i = ALTC.load(Ordering::Relaxed) as usize;
        if !run && i < ALT_C.len() && now.wrapping_sub(LAST_RUN.load(Ordering::Relaxed)) >= 800
            && now.wrapping_sub(ALTC_AT.load(Ordering::Relaxed)) >= 10
        {
            crate::kprintln!("[c0c] inject Alt-C byte {:#04x} at hz_200 {}", ALT_C[i], now);
            crate::ikbd::feed(ALT_C[i]);
            ALTC.store(i as u32 + 1, Ordering::Relaxed);
            ALTC_AT.store(now, Ordering::Relaxed);
        }
    }
}
// C0C-TEMP END
'''
he.write_text(s)

# --- lib.rs: registry + boot launch (board-qemu-virt only) -----------------
lib = t / "kernel/src/lib.rs"
s = lib.read_text()
old = """    launch::run_auto_folder();
"""
assert s.count(old) == 1, "lib.rs auto-folder anchor"
s = s.replace(old, f"""    launch::run_auto_folder();

    // C0C-TEMP BEGIN -- crash programs from the RAM registry (no disk on virt)
    #[cfg(feature = "board-qemu-virt")]
    {{
        static CRCHILD: &[u8] = include_bytes!("{progs}/CRCHILD.NAP");
        static CRASHTST: &[u8] = include_bytes!("{progs}/CRASHTST.NAP");
        tos_api::bdos::process::registry::register(b"C:\\\\CRCHILD.NAP", CRCHILD);
        tos_api::bdos::process::registry::register(b"C:\\\\CRASHTST.NAP", CRASHTST);
        kprintln!("[c0c] launch C:\\\\CRCHILD.NAP (no tail = case w), kernel parent");
        let rc = launch::run_program_with_tail_rc(b"C:\\\\CRCHILD.NAP", &[]);
        kprintln!("[c0c] back in the boot path after CRCHILD, launch rc={{}}", rc);
        let rc = launch::run_program_with_tail_rc(b"C:\\\\CRASHTST.NAP", &[]);
        kprintln!("[c0c] back in the boot path after CRASHTST, launch rc={{}}", rc);
    }}
    // C0C-TEMP END
""")
lib.write_text(s)
print("instrumented", t)
