#!/usr/bin/env python3
"""A41 part 2 trace (SCRATCH copy only): serial lines for rsrc_load / rsrc_free
return values (+ selector) and EL0 Mfree(addr) -> rc."""
import sys, pathlib
ws=pathlib.Path(sys.argv[1])
p=ws/"tos-api/src/aes/resource.rs"; s=p.read_text()
old="""        unsafe { report_load(pb.global_ptr(), &desc) };
    }
    pb.set_ret(i16::from(loaded.is_some()));
}"""
new="""        unsafe { report_load(pb.global_ptr(), &desc) };
    }
    // A41P2-TEMP (scratch copy only): serial evidence of the load.
    a41_log(format_args!("[a41] pid={} rsrc_load(\\"{}\\") -> {} ap_rscmem=0x{:x} ap_rsclen={} cicon_buf={} B",
        pid, path_str, i16::from(loaded.is_some()),
        loaded.map_or(0, |d| published_name(&d)), loaded.map_or(0, |d| d.blob_len), loaded.map_or(0, |d| d.cicon.len)));
    pb.set_ret(i16::from(loaded.is_some()));
}

// A41P2-TEMP BEGIN (scratch copy only)
pub fn a41_log(args: core::fmt::Arguments) {
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
// A41P2-TEMP END"""
assert old in s, "rsrc_load tail"; s=s.replace(old,new,1)
old="""    // (`xa_rsrc.c:1347-1362`); not adopted.
    pb.set_ret(rc);
}"""
new="""    // (`xa_rsrc.c:1347-1362`); not adopted.
    a41_log(format_args!("[a41] pid={} rsrc_free(selector 0x{:x}) -> {}", pid,
        unsafe { (pb.global_ptr() as *const u8).add(AP_RSCMEM_OFF).cast::<u32>().read_unaligned() }, rc));
    pb.set_ret(rc);
}"""
assert old in s, "rsrc_free tail"; s=s.replace(old,new,1)
old="""    let Some(idx) = (unsafe { selected(pid, pb.global_ptr()) }) else {
        pb.set_ret(0);
        return;
    };
    // SAFETY: single trap path; `idx` was just found in the table.
    let entry = unsafe { table()[idx] };"""
new="""    let Some(idx) = (unsafe { selected(pid, pb.global_ptr()) }) else {
        a41_log(format_args!("[a41] pid={} rsrc_free(selector 0x{:x}) -> 0 (names nothing)", pid,
            unsafe { (pb.global_ptr() as *const u8).add(AP_RSCMEM_OFF).cast::<u32>().read_unaligned() }));
        pb.set_ret(0);
        return;
    };
    // SAFETY: single trap path; `idx` was just found in the table.
    let entry = unsafe { table()[idx] };"""
assert old in s, "rsrc_free head"; s=s.replace(old,new,1)
p.write_text(s)
p=ws/"tos-api/src/gemdos/memory.rs"; s=p.read_text()
old="""    match usermem::free(slot, addr as u64) {
        // SAFETY: BDOS lock held; `m` came from `Mxalloc` and is on the
        // allocated list.
        Some(m) => unsafe { traps::mfree(m) },
        None => EIMBA,
    }
}"""
new="""    let rc = match usermem::free(slot, addr as u64) {
        // SAFETY: BDOS lock held; `m` came from `Mxalloc` and is on the
        // allocated list.
        Some(m) => unsafe { traps::mfree(m) },
        None => EIMBA,
    };
    // A41P2-TEMP (scratch copy only)
    crate::aes::resource::a41_log(format_args!("[a41] Mfree(0x{:x}) -> {}", addr, rc));
    rc
}"""
assert old in s, "mfree"; s=s.replace(old,new,1)
p.write_text(s)
print("patched")
