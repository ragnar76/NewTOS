#!/usr/bin/env python3
"""A76 (SCRATCH copy only): the A41 part-2 scaffolding on a workspace copy —
Host::debug_write, aes::H02_TRACE, the H02 IKBD injector (apply_injector.py)
and serial trace lines for rsrc_load / rsrc_free / EL0 Mfree(addr) -> rc.
usage: prepare_scratch.py <ws> <script.rs>"""
import sys, pathlib, subprocess
ws = pathlib.Path(sys.argv[1]); script = sys.argv[2]
here = pathlib.Path(__file__).parent

def patch(rel, old, new, what):
    p = ws / rel; s = p.read_text()
    assert s.count(old) == 1, f"{rel}: anchor '{what}' not unique/found"
    p.write_text(s.replace(old, new, 1))

# Host::debug_write (serial-only sink used by the injector + trace)
patch("tos-api/src/host.rs",
"""    /// Non-blocking read from the console input device. Returns
    /// `Some(byte)` if a character is available, `None` otherwise.
    fn console_try_read(&self) -> Option<u8> {""",
"""    /// A76-TEMP (scratch copy only): serial-only debug write.
    fn debug_write(&self, s: &[u8]) {
        self.console_write_bytes(s)
    }

    /// Non-blocking read from the console input device. Returns
    /// `Some(byte)` if a character is available, `None` otherwise.
    fn console_try_read(&self) -> Option<u8> {""", "host debug_write")

p = ws / "tos-api/src/aes/mod.rs"
p.write_text(p.read_text() + "\n// A76-TEMP (scratch copy only): named by the injector's Step::Arm, unused.\npub static H02_TRACE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);\n")

subprocess.check_call([sys.executable, str(here / "apply_injector.py"), str(ws), script])

# --- trace: rsrc_load tail
patch("tos-api/src/aes/resource.rs",
"""        unsafe { report_load(pb.global_ptr(), &desc) };
    }
    pb.set_ret(i16::from(loaded.is_some()));
}""",
"""        unsafe { report_load(pb.global_ptr(), &desc) };
    }
    // A76-TEMP (scratch copy only): serial evidence of the load.
    a76_log(format_args!("[a76] owner={} rsrc_load(\\"{}\\") -> {} ap_rscmem=0x{:x} ap_rsclen={} cicon_buf={} B",
        owner, path_str, i16::from(loaded.is_some()),
        loaded.map_or(0, |d| published_name(&d)), loaded.map_or(0, |d| d.blob_len), loaded.map_or(0, |d| d.cicon.len)));
    pb.set_ret(i16::from(loaded.is_some()));
}

// A76-TEMP BEGIN (scratch copy only)
pub fn a76_log(args: core::fmt::Arguments) {
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
    let _ = w.write_str("\\r\\n");
}
// A76-TEMP END""", "rsrc_load tail")

# --- trace: rsrc_free tail + head
patch("tos-api/src/aes/resource.rs",
"""    // (`xa_rsrc.c:1347-1362`); not adopted.
    pb.set_ret(rc);
}""",
"""    // (`xa_rsrc.c:1347-1362`); not adopted.
    a76_log(format_args!("[a76] owner={} rsrc_free(selector 0x{:x}) -> {}", owner,
        unsafe { (pb.global_ptr() as *const u8).add(AP_RSCMEM_OFF).cast::<u32>().read_unaligned() }, rc));
    pb.set_ret(rc);
}""", "rsrc_free tail")
patch("tos-api/src/aes/resource.rs",
"""    let Some(idx) = (unsafe { selected(owner, pb.global_ptr()) }) else {
        pb.set_ret(0);
        return;
    };
    // SAFETY: single trap path; `idx` was just found in the table.
    let entry = unsafe { table()[idx] };""",
"""    let Some(idx) = (unsafe { selected(owner, pb.global_ptr()) }) else {
        a76_log(format_args!("[a76] owner={} rsrc_free(selector 0x{:x}) -> 0 (names nothing)", owner,
            unsafe { (pb.global_ptr() as *const u8).add(AP_RSCMEM_OFF).cast::<u32>().read_unaligned() }));
        pb.set_ret(0);
        return;
    };
    // SAFETY: single trap path; `idx` was just found in the table.
    let entry = unsafe { table()[idx] };""", "rsrc_free head")

# --- trace: EL0 Mfree -> rc (tree = el0_free; HEAD = inline match)
p = ws / "tos-api/src/gemdos/memory.rs"; s = p.read_text()
new_anchor = """    let rc = unsafe { traps::mfree(m) };
    if rc == E_OK {
        crate::aes::resource::on_mfree(slot, m);
    }
    rc
}"""
head_anchor = """    match usermem::free(slot, addr as u64) {
        // SAFETY: BDOS lock held; `m` came from `Mxalloc` and is on the
        // allocated list.
        Some(m) => unsafe { traps::mfree(m) },
        None => EIMBA,
    }
}"""
if s.count(new_anchor) == 1:
    s = s.replace(new_anchor, """    let rc = unsafe { traps::mfree(m) };
    if rc == E_OK {
        crate::aes::resource::on_mfree(slot, m);
    }
    // A76-TEMP (scratch copy only)
    crate::aes::resource::a76_log(format_args!("[a76] Mfree(0x{:x}) -> {}", user_va, rc));
    rc
}""", 1); print("mfree trace: el0_free (tree)")
elif s.count(head_anchor) == 1:
    s = s.replace(head_anchor, """    let rc = match usermem::free(slot, addr as u64) {
        // SAFETY: BDOS lock held; `m` came from `Mxalloc` and is on the
        // allocated list.
        Some(m) => unsafe { traps::mfree(m) },
        None => EIMBA,
    };
    // A76-TEMP (scratch copy only)
    crate::aes::resource::a76_log(format_args!("[a76] Mfree(0x{:x}) -> {}", addr, rc));
    rc
}""", 1); print("mfree trace: HEAD mfree")
else:
    raise SystemExit("memory.rs: no Mfree anchor")
p.write_text(s)
print("prepared", ws)
