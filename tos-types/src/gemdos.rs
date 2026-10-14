//! GEMDOS function numbers — the one set the trap clients name.
//!
//! The GEMDOS trap (`trap #1`, on NewTOS `svc #1`) selects its function by
//! number: on m68k the first word on the stack, natively the low 16 bits of
//! `x8` (`docs/ARCHITECTURE.md`, "Native AArch64 callers"). EmuTOS has no
//! named constants for them — its bindings are the `trap1(0x.., …)` macros of
//! `include/bdosbind.h:22-54`, whose names (`Fopen`, `Dsetdrv`, …) are the
//! Atari ones. The constants here are those names in upper case with the
//! macro's number, plus the three the bindings use that `bdosbind.h` does not
//! carry (`Cconout`, `Pterm`, the FreeMiNT `Ssystem`).
//!
//! **One home (DESK43 U6b).** The numbers used to be private copies in the
//! AES shell (`tos-api/src/aes/shell.rs`, which drives GEMDOS through
//! `TrapArgs`), in the kernel desktop's `dosbind.rs` and in the native program
//! runtime (`programs/newtos-gem`). The set is the functions those clients
//! call, not the whole GEMDOS table; a client that needs another one adds it
//! here. The GEMDOS dispatcher itself (`tos-api/src/gemdos/`) keeps its own
//! table, keyed by the same numbers (`docs/api/gemdos/`).

/// `Cconout(c)` — GEMDOS 0x02 (`docs/api/gemdos/`); not in `bdosbind.h`.
pub const CCONOUT: u16 = 0x02;
/// `Cconws(buf)` — `bdosbind.h:24`.
pub const CCONWS: u16 = 0x09;
/// `Dsetdrv(drv)` — `bdosbind.h:26`.
pub const DSETDRV: u16 = 0x0E;
/// `Dgetdrv()` — `bdosbind.h:30`.
pub const DGETDRV: u16 = 0x19;
/// `Fsetdta(buf)` — `bdosbind.h:31`.
pub const FSETDTA: u16 = 0x1A;
/// `Dfree(buf, driveno)` — `bdosbind.h:34`.
pub const DFREE: u16 = 0x36;
/// `Dcreate(path)` — `bdosbind.h:33`.
pub const DCREATE: u16 = 0x39;
/// `Ddelete(path)` — `bdosbind.h:35`.
pub const DDELETE: u16 = 0x3A;
/// `Dsetpath(path)` — `bdosbind.h:36`.
pub const DSETPATH: u16 = 0x3B;
/// `Fcreate(fname, attr)` — `bdosbind.h:37`.
pub const FCREATE: u16 = 0x3C;
/// `Fopen(fname, mode)` — `bdosbind.h:38`.
pub const FOPEN: u16 = 0x3D;
/// `Fclose(handle)` — `bdosbind.h:39`.
pub const FCLOSE: u16 = 0x3E;
/// `Fread(handle, count, buf)` — `bdosbind.h:40`.
pub const FREAD: u16 = 0x3F;
/// `Fwrite(handle, count, buf)` — `bdosbind.h:41`.
pub const FWRITE: u16 = 0x40;
/// `Fdelete(fname)` — `bdosbind.h:42`.
pub const FDELETE: u16 = 0x41;
/// `Fattrib(filename, wflag, attrib)` — `bdosbind.h:44`.
pub const FATTRIB: u16 = 0x43;
/// `Mxalloc(amount, mode)` — `bdosbind.h:45`.
pub const MXALLOC: u16 = 0x44;
/// `Dgetpath(path, driveno)` — `bdosbind.h:46`.
pub const DGETPATH: u16 = 0x47;
/// `Malloc(number)` — `bdosbind.h:47`.
pub const MALLOC: u16 = 0x48;
/// `Mfree(block)` — `bdosbind.h:48`.
pub const MFREE: u16 = 0x49;
/// `Pterm(retcode)` — GEMDOS 0x4C (`docs/api/gemdos/`); EmuTOS's desktop
/// issues it from assembly (`desk/deskstart.S`), so `bdosbind.h` has no macro.
pub const PTERM: u16 = 0x4C;
/// `Fsfirst(filename, attr)` — `bdosbind.h:51`.
pub const FSFIRST: u16 = 0x4E;
/// `Fsnext()` — `bdosbind.h:52`.
pub const FSNEXT: u16 = 0x4F;
/// `Frename(0, oldname, newname)` — `bdosbind.h:53`.
pub const FRENAME: u16 = 0x56;
/// `Fdatime(timeptr, handle, wflag)` — `bdosbind.h:54`.
pub const FDATIME: u16 = 0x57;
/// `Ssystem(mode, arg1, arg2)` — GEMDOS 0x154, a FreeMiNT extension NewTOS
/// implements (`docs/api/gemdos/154-ssystem.md`).
pub const SSYSTEM: u16 = 0x154;
