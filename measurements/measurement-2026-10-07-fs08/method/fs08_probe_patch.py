#!/usr/bin/env python3
"""fs08_probe_patch.py <tree> -- instrument a SCRATCH COPY of the NewTOS tree
(base desk-rebuild e2dfeef) with the FS08/INPUT01 probe. Never run on the
shared tree. Every anchor must match exactly once (assert)."""
import pathlib, shutil, sys

T = pathlib.Path(sys.argv[1])
HERE = pathlib.Path(__file__).resolve().parent


def sub(rel, old, new, count=1):
    p = T / rel
    s = p.read_text()
    n = s.count(old)
    assert n == count, f"{rel}: anchor found {n}x, want {count}: {old[:70]!r}"
    p.write_text(s.replace(old, new))


shutil.copy(HERE / "fs08probe.rs", T / "tos-api/src/fs08probe.rs")
sub("tos-api/src/lib.rs", "pub mod cookie;\n", "pub mod cookie;\npub mod fs08probe;\n")

# GEMDOS dispatcher: per-opcode time, Fread sizes, per-handle accounting
sub("tos-api/src/gemdos/mod.rs",
    "    let t0 = trace::call_begin();\n    let ret = dispatch_inner(args);\n    trace::call_end(args, t0, ret);\n    ret\n",
    "    let t0 = trace::call_begin();\n    let fs08 = crate::fs08probe::gd_begin();\n    crate::fs08probe::in_gemdos(true);\n    let ret = dispatch_inner(args);\n    crate::fs08probe::in_gemdos(false);\n    crate::fs08probe::gd_end(args, fs08, ret);\n    trace::call_end(args, t0, ret);\n    ret\n")

# Fopen: the path name for the per-file record
sub("tos-api/src/bdos/fs/file.rs",
    "    let mode = (args.arg_u16(1) & 0x77) as u8;\n",
    "    let mode = (args.arg_u16(1) & 0x77) as u8;\n    crate::fs08probe::note_open_path(path.as_bytes());\n")

# Fread: chain walk from the file start + per-cluster advance
sub("tos-api/src/bdos/fs/file.rs",
    "    let clsizb = vol.clsizb;\n    let recsiz = vol.recsiz;\n\n    // Walk to the cluster containing the current position.\n",
    "    let clsizb = vol.clsizb;\n    let recsiz = vol.recsiz;\n    crate::fs08probe::clsizb(clsizb);\n\n    // Walk to the cluster containing the current position.\n")
sub("tos-api/src/bdos/fs/file.rs",
    "    for _ in 0..skip {\n        match vol.fat_entry(cluster) {\n",
    "    for _ in 0..skip {\n        crate::fs08probe::walk_skip();\n        match vol.fat_entry(cluster) {\n")
sub("tos-api/src/bdos/fs/file.rs",
    "        byte_in_cluster = 0;\n        match vol.fat_entry(cluster) {\n            Some(nx) if !vol.is_eoc(nx) => cluster = nx,\n",
    "        byte_in_cluster = 0;\n        crate::fs08probe::walk_adv();\n        match vol.fat_entry(cluster) {\n            Some(nx) if !vol.is_eoc(nx) => cluster = nx,\n")

# FAT: cache misses caused by fat_entry
sub("tos-api/src/bdos/fs/fat.rs",
    "    pub fn fat_entry(&self, cluster: u32) -> Option<u32> {\n",
    "    pub fn fat_entry(&self, cluster: u32) -> Option<u32> {\n"
    "        let m0 = super::cache::stats().1;\n"
    "        let r = self.fat_entry_inner(cluster);\n"
    "        crate::fs08probe::fat_entry_done(super::cache::stats().1.wrapping_sub(m0));\n"
    "        r\n    }\n\n"
    "    fn fat_entry_inner(&self, cluster: u32) -> Option<u32> {\n")

# EMMC: per-command phase timing
E = "kernel/src/drivers/bcm2711/emmc.rs"
sub(E,
    "    fn read_one(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {\n"
    "        if !self.ready.load(Ordering::Acquire) {\n"
    "            return Err(BlockError::NotReady);\n"
    "        }\n"
    "        if !Self::wait_clear(STATUS, SR_DAT_INHIBIT, Phase::DatInhibit) {\n"
    "            return Err(BlockError::Timeout);\n"
    "        }\n",
    "    fn read_one(&self, lba: u64, buf: &mut [u8]) -> Result<(), BlockError> {\n"
    "        let t0 = tos_api::fs08probe::now();\n"
    "        let mut ph = [0u64; 5];\n"
    "        let r = self.read_one_t(lba, buf, &mut ph);\n"
    "        tos_api::fs08probe::emmc_read(ph, tos_api::fs08probe::now().wrapping_sub(t0), r.is_ok());\n"
    "        r\n    }\n\n"
    "    fn read_one_t(&self, lba: u64, buf: &mut [u8], ph: &mut [u64; 5]) -> Result<(), BlockError> {\n"
    "        use tos_api::fs08probe::now;\n"
    "        if !self.ready.load(Ordering::Acquire) {\n"
    "            return Err(BlockError::NotReady);\n"
    "        }\n"
    "        let mut t = now();\n"
    "        if !Self::wait_clear(STATUS, SR_DAT_INHIBIT, Phase::DatInhibit) {\n"
    "            return Err(BlockError::Timeout);\n"
    "        }\n"
    "        ph[0] = now().wrapping_sub(t);\n"
    "        t = now();\n")
sub(E,
    "        let status = self.command(READ_SINGLE, addr)?;\n"
    "        if let Err(e) = check_r1(17, status) {\n"
    "            Self::recover_no_abort(Phase::ReadRdy);\n"
    "            return Err(e);\n"
    "        }\n"
    "        Self::wait_interrupt(INT_READ_RDY, Phase::ReadRdy)?;\n"
    "        for word in buf.chunks_exact_mut(4) {\n"
    "            let w = rd(DATA);\n"
    "            word.copy_from_slice(&w.to_le_bytes());\n"
    "        }\n"
    "        Self::wait_interrupt(INT_DATA_DONE, Phase::ReadDone)?;\n"
    "        Ok(())\n",
    "        let status = self.command(READ_SINGLE, addr)?;\n"
    "        if let Err(e) = check_r1(17, status) {\n"
    "            Self::recover_no_abort(Phase::ReadRdy);\n"
    "            return Err(e);\n"
    "        }\n"
    "        ph[1] = now().wrapping_sub(t);\n"
    "        t = now();\n"
    "        Self::wait_interrupt(INT_READ_RDY, Phase::ReadRdy)?;\n"
    "        ph[2] = now().wrapping_sub(t);\n"
    "        t = now();\n"
    "        for word in buf.chunks_exact_mut(4) {\n"
    "            let w = rd(DATA);\n"
    "            word.copy_from_slice(&w.to_le_bytes());\n"
    "        }\n"
    "        ph[3] = now().wrapping_sub(t);\n"
    "        t = now();\n"
    "        Self::wait_interrupt(INT_DATA_DONE, Phase::ReadDone)?;\n"
    "        ph[4] = now().wrapping_sub(t);\n"
    "        Ok(())\n")
sub(E,
    "    fn write_one(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {\n",
    "    fn write_one(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {\n"
    "        let t0 = tos_api::fs08probe::now();\n"
    "        let r = self.write_one_inner(lba, buf);\n"
    "        tos_api::fs08probe::emmc_write(tos_api::fs08probe::now().wrapping_sub(t0), r.is_ok());\n"
    "        r\n    }\n\n"
    "    fn write_one_inner(&self, lba: u64, buf: &[u8]) -> Result<(), BlockError> {\n")
sub(E,
    "        for (i, sector) in buf.chunks_exact_mut(SECTOR_SIZE).enumerate() {\n",
    "        tos_api::fs08probe::blk_read((buf.len() / SECTOR_SIZE) as u64);\n"
    "        for (i, sector) in buf.chunks_exact_mut(SECTOR_SIZE).enumerate() {\n")
sub(E,
    "        for (i, sector) in buf.chunks_exact(SECTOR_SIZE).enumerate() {\n",
    "        tos_api::fs08probe::blk_write((buf.len() / SECTOR_SIZE) as u64);\n"
    "        for (i, sector) in buf.chunks_exact(SECTOR_SIZE).enumerate() {\n")
sub(E,
    "    // Whatever the identification itself ran into, once (the per-failure\n",
    "    tos_api::fs08probe::set_m1([\n"
    "        ver, caps, c0, c1,\n"
    "        rate(super::mailbox::CLOCK_ID_EMMC),\n"
    "        rate(super::mailbox::CLOCK_ID_EMMC2),\n"
    "    ]);\n"
    "    fn fs08_serial(b: &[u8]) {\n"
    "        use crate::hal::uart::Uart;\n"
    "        for &x in b {\n"
    "            crate::console::CONSOLE.write_byte(x);\n"
    "        }\n"
    "    }\n"
    "    tos_api::fs08probe::SERIAL_OUT.store(fs08_serial as fn(&[u8]) as usize, Ordering::Relaxed);\n\n"
    "    // Whatever the identification itself ran into, once (the per-failure\n")

# Input pump: count, own time, gaps
H = "kernel/src/host_env.rs"
sub(H, "    fn pump_input(&self) {\n", "    fn pump_input(&self) {\n        tos_api::fs08probe::maybe_flush_from_pump();\n        let fs08_t0 = tos_api::fs08probe::now();\n")
sub(H,
    "        crate::ikbd::poll_repeat();\n        core::hint::spin_loop();\n",
    "        crate::ikbd::poll_repeat();\n        core::hint::spin_loop();\n        tos_api::fs08probe::on_pump(fs08_t0);\n")

# IRQ counts
G = "kernel/src/drivers/gic.rs"
sub(G, "        crate::console::pl011_rx_drain();\n", "        tos_api::fs08probe::irq(1);\n        crate::console::pl011_rx_drain();\n")
sub(G, "        crate::drivers::timer::handle_irq();\n", "        tos_api::fs08probe::irq(0);\n        crate::drivers::timer::handle_irq();\n")
sub(G, "    // Unknown / unrouted interrupts are silently EOIed; PHASE-03 will\n",
    "    else {\n        tos_api::fs08probe::irq(2);\n    }\n    // Unknown / unrouted interrupts are silently EOIed; PHASE-03 will\n")

# SVC: per-class dispatch time, Pexec, report flush
X = "kernel/src/arch/aarch64/exceptions.rs"
sub(X, "    let result: i64 = match class {\n",
    "    let fs08_t0 = tos_api::fs08probe::now();\n"
    "    let fs08_cls = match class {\n"
    "        TrapClass::Gemdos => 0,\n"
    "        TrapClass::Gem => if args.regs[0] == 115 { 1 } else { 2 },\n"
    "        TrapClass::Bios => 3,\n"
    "        TrapClass::Xbios => 4,\n"
    "    };\n"
    "    let result: i64 = match class {\n")
sub(X, "    // Mid-syscall cooperative yield (S2 restartable block, design §4.3 + §8.2\n",
    "    tos_api::fs08probe::svc_done(fs08_cls, fs08_t0);\n\n"
    "    // Mid-syscall cooperative yield (S2 restartable block, design §4.3 + §8.2\n")
sub(X, "    tos_api::gemdos::trace::flush_if_requested();\n",
    "    tos_api::gemdos::trace::flush_if_requested();\n"
    "    tos_api::fs08probe::maybe_flush(matches!(class, TrapClass::Gem));\n")
sub(X, "        if func == 0x4B {\n            handle_pexec(frame);\n            return;\n        }\n",
    "        if func == 0x4B {\n"
    "            let fs08_mode = frame.gpr[0] as u16;\n"
    "            let fs08_b = tos_api::fs08probe::pexec_begin();\n"
    "            handle_pexec(frame);\n"
    "            tos_api::fs08probe::pexec_done(fs08_mode, fs08_b);\n"
    "            return;\n        }\n")
sub(X, "    let path_bytes = &path_buf[..path_len];\n",
    "    let path_bytes = &path_buf[..path_len];\n    tos_api::fs08probe::note_pexec_name(path_bytes);\n")
print("fs08 probe applied to", T)
