#!/usr/bin/env python3
r"""Decode a NewTOS GEMDOS trap trace (`C:\GDTRACE.BIN`) into readable text.

The trace is produced by `tos_api::gemdos::trace` (kernel built with
`--features gemdos-trace`). The target writes fixed-size binary records and
does no formatting at all — every byte of interpretation happens here, which is
the `docs/TOS-REFERENCE-DUMPER.md` §3 rule applied to a live trace.

    tools/gdtrace.py GDTRACE.BIN                 # full trace
    tools/gdtrace.py GDTRACE.BIN --summary       # per-opcode call counts
    tools/gdtrace.py GDTRACE.BIN --op 0x40       # only Fwrite
    tools/gdtrace.py GDTRACE.BIN --files         # per-file open/read/write/close story
    tools/gdtrace.py GDTRACE.BIN --errors        # only calls that failed
    tools/gdtrace.py GDTRACE.BIN --tail 200      # last 200 records
    tools/gdtrace.py GDTRACE.BIN --hex           # add a hex dump of every payload

File layout (little-endian, native AArch64 order):

  * 256-byte ASCII header line, space-padded, `\n`-terminated
  * N x 256-byte records, oldest first (prologue span, then ring span)

Record layout — must stay in lockstep with `struct Rec` in
`tos-api/src/gemdos/trace.rs`:

  off  size  field
    0     4  seq        u32   record number, gapless across a ring wrap
    4     2  opcode     u16   GEMDOS function id (x8)
    6     1  kind       u8    1=CALL 2=MARK 3=FRAME
    7     1  flags      u8
    8     8  t_ticks    u64   CNTPCT_EL0 at dispatch entry
   16     8  dur_ticks  u64   dispatch entry -> exit
   24     8  ret        i64   the Long result, sign-extended
   32    56  args[7]    u64   x0..x6 as the caller passed them
   88     1  slot       u8    GEMDOS process slot, 0xFF = in-kernel caller
   89     1  abi        u8    0 = LP64, 1 = ILP32
   90     1  cap_kind   u8
   91     1  cap_src    u8    argument the payload came from, 0xFF = none
   92     2  cap_len    u16   payload bytes valid in cap
   94     2  cap2_off   u16   offset of the second payload part
   96   160  cap        u8[]
"""

import argparse
import struct
import sys
from collections import Counter, OrderedDict

FORMAT = b"NEWTOS-GDTRACE v1 "
HDR_LEN = 256
REC_LEN = 256
REC_FMT = "<IHBBQQq7QBBBBHH160s"

KIND_CALL, KIND_MARK, KIND_FRAME = 1, 2, 3
KIND_NAME = {1: "CALL", 2: "MARK", 3: "FRAME"}

F_KERNEL, F_ILP32, F_CLAMPED, F_NO_RESULT = 0x01, 0x02, 0x04, 0x08

CAP_NONE, CAP_PATH, CAP_PATH2, CAP_PATH_DTA = 0, 1, 2, 3
CAP_DTA, CAP_DATA, CAP_DFREE, CAP_DOSTIME, CAP_CONSOLE = 4, 5, 6, 7, 8

MARK_FLUSH = 1

# Bytes of a captured data buffer shown inline in the default view. The whole
# payload is always in the file and `--hex` prints all of it.
HEAD_BYTES = 40

# GEMDOS function names. EmuTOS `bdos/bdosmain.c:142-291` (the `funcs[]`
# dispatch table) for 0x00..0x57; the Atari names are the ones an application
# programmer knows, so both appear where they differ.
GD_NAME = {
    0x00: "Pterm0", 0x01: "Cconin", 0x02: "Cconout", 0x03: "Cauxin",
    0x04: "Cauxout", 0x05: "Cprnout", 0x06: "Crawio", 0x07: "Crawcin",
    0x08: "Cnecin", 0x09: "Cconws", 0x0A: "Cconrs", 0x0B: "Cconis",
    0x0E: "Dsetdrv", 0x10: "Cconos", 0x11: "Cprnos", 0x12: "Cauxis",
    0x13: "Cauxos", 0x14: "Maddalt", 0x15: "Srealloc",
    0x19: "Dgetdrv", 0x1A: "Fsetdta",
    0x2A: "Tgetdate", 0x2B: "Tsetdate", 0x2C: "Tgettime", 0x2D: "Tsettime",
    0x2F: "Fgetdta", 0x30: "Sversion", 0x31: "Ptermres",
    0x36: "Dfree", 0x39: "Dcreate", 0x3A: "Ddelete", 0x3B: "Dsetpath",
    0x3C: "Fcreate", 0x3D: "Fopen", 0x3E: "Fclose", 0x3F: "Fread",
    0x40: "Fwrite", 0x41: "Fdelete", 0x42: "Fseek", 0x43: "Fattrib",
    0x44: "Mxalloc", 0x45: "Fdup", 0x46: "Fforce", 0x47: "Dgetpath",
    0x48: "Malloc", 0x49: "Mfree", 0x4A: "Mshrink", 0x4B: "Pexec",
    0x4C: "Pterm", 0x4D: "Pwait", 0x4E: "Fsfirst", 0x4F: "Fsnext",
    0x56: "Frename", 0x57: "Fdatime",
}

# FreeMiNT extension calls (`references/freemint/sys/syscalls.master`). NewTOS
# implements only `Ssystem` (0x154) of these; every other id here returns
# `EINVFN`, which is worth seeing by name — "the application asked for
# `Fstat64` and did not get it" is a finding, "?0x14b -> -32" is noise.
MINT_NAME = {
    0x100: "Fpipe", 0x101: "Ffchown", 0x102: "Ffchmod", 0x103: "Fsync",
    0x104: "Fcntl", 0x105: "Finstat", 0x106: "Foutstat", 0x107: "Fgetchar",
    0x108: "Fputchar", 0x109: "Pwait", 0x10A: "Pnice", 0x10B: "Pgetpid",
    0x10C: "Pgetppid", 0x10D: "Pgetpgrp", 0x10E: "Psetpgrp", 0x10F: "Pgetuid",
    0x110: "Psetuid", 0x111: "Pkill", 0x112: "Psignal", 0x113: "Pvfork",
    0x114: "Pgetgid", 0x115: "Psetgid", 0x116: "Psigblock",
    0x117: "Psigsetmask", 0x118: "Pusrval", 0x119: "Pdomain",
    0x11A: "Psigreturn", 0x11B: "Pfork", 0x11C: "Pwait3", 0x11D: "Fselect",
    0x11E: "Prusage", 0x11F: "Psetlimit", 0x120: "Talarm", 0x121: "Pause",
    0x122: "Sysconf", 0x123: "Psigpending", 0x124: "Dpathconf",
    0x125: "Pmsg", 0x126: "Fmidipipe", 0x127: "Prenice", 0x128: "Dopendir",
    0x129: "Dreaddir", 0x12A: "Drewinddir", 0x12B: "Dclosedir",
    0x12C: "Fxattr", 0x12D: "Flink", 0x12E: "Fsymlink", 0x12F: "Freadlink",
    0x130: "Dcntl", 0x131: "Fchown", 0x132: "Fchmod", 0x133: "Pumask",
    0x134: "Psemaphore", 0x135: "Dlock", 0x136: "Psigpause",
    0x137: "Psigaction", 0x138: "Pgeteuid", 0x139: "Pgetegid",
    0x13A: "Pwaitpid", 0x13B: "Dgetcwd", 0x13C: "Salert", 0x13D: "Tmalarm",
    0x13E: "Psigintr", 0x13F: "Suptime", 0x140: "Ptrace", 0x141: "Mvalidate",
    0x142: "Dxreaddir", 0x143: "Pseteuid", 0x144: "Psetegid",
    0x145: "Pgetauid", 0x146: "Psetauid", 0x147: "Pgetgroups",
    0x148: "Psetgroups", 0x149: "Tsetitimer", 0x14A: "Dchroot",
    0x14B: "Fstat64", 0x14C: "Fseek64", 0x14D: "Dsetkey", 0x14E: "Psetreuid",
    0x14F: "Psetregid", 0x150: "Sync", 0x151: "Shutdown",
    0x152: "Dreadlabel", 0x153: "Dwritelabel", 0x154: "Ssystem",
    0x155: "Tgettimeofday", 0x156: "Tsettimeofday", 0x157: "Tadjtime",
    0x158: "Pgetpriority", 0x159: "Psetpriority", 0x15A: "Fpoll",
    0x15B: "Fwritev", 0x15C: "Freadv", 0x15D: "Ffstat64", 0x15E: "Psysctl",
    0x15F: "Semulation", 0x160: "Fsocket",
}
GD_NAME.update(MINT_NAME)

# TOS error codes — `tos-types/src/error.rs`, values matching the Compendium.
ERR_NAME = {
    -1: "ERROR", -2: "EDRVNR", -3: "EUNCMD", -4: "E_CRC", -5: "EBADRQ",
    -6: "E_SEEK", -7: "EMEDIA", -8: "ESECNF", -9: "EPAPER", -10: "EWRITF",
    -11: "EREADF", -13: "EWRPRO", -14: "E_CHNG", -15: "EUNDEV", -16: "EBADSF",
    -17: "EOTHER", -25: "EINVAL", -32: "EINVFN", -33: "EFILNF", -34: "EPTHNF",
    -35: "ENHNDL", -36: "EACCDN", -37: "EIHNDL", -38: "EPERM", -39: "ENSMEM",
    -40: "EIMBA", -46: "EDRIVE", -48: "ENSAME", -49: "ENMFIL", -58: "ELOCKED",
    -59: "ENSLOCK", -64: "ERANGE", -65: "EINTRN", -66: "EPLFMT", -67: "EGSBF",
}

# FAT attribute bits (EmuTOS `include/bdosdefs.h` FA_*).
ATTR_BITS = [(0x01, "RO"), (0x02, "HID"), (0x04, "SYS"), (0x08, "VOL"),
             (0x10, "DIR"), (0x20, "ARC")]

# Fopen access modes (EmuTOS `bdos/fsopnclo.c`, the low 3 bits).
OPEN_MODE = {0: "RO", 1: "WO", 2: "RW"}

# Fseek origins.
SEEK_MODE = {0: "SET", 1: "CUR", 2: "END"}

# Pexec modes (EmuTOS `bdos/proc.c`).
PEXEC_MODE = {0: "LOADGO", 3: "LOAD", 4: "GO", 5: "BASEPAGE", 6: "GO_FREE",
              7: "LOADGO_FREE", 100: "CREATE_PSETLIMIT"}


def attrs(v):
    on = [n for b, n in ATTR_BITS if v & b]
    return "|".join(on) if on else "-"


def err(v):
    """Render a return value: named error, or the plain number."""
    if v < 0:
        return f"{v} {ERR_NAME.get(v, '?')}"
    return str(v)


def cstr(b, off=0):
    """The NUL-terminated string starting at `off`, and the offset past it."""
    end = b.find(b"\0", off)
    if end < 0:
        end = len(b)
    return b[off:end].decode("latin-1"), end + 1


def printable(b):
    """Bytes as a single-line, quoted, printable string."""
    out = []
    for c in b:
        if 32 <= c < 127:
            out.append(chr(c))
        elif c == 13:
            out.append("\\r")
        elif c == 10:
            out.append("\\n")
        elif c == 9:
            out.append("\\t")
        else:
            out.append(f"\\x{c:02x}")
    return "".join(out)


class Rec:
    __slots__ = ("seq", "op", "kind", "flags", "t", "dur", "ret", "args",
                 "slot", "abi", "cap_kind", "cap_src", "cap_len", "cap2_off",
                 "cap")

    def __init__(self, raw):
        f = struct.unpack(REC_FMT, raw)
        (self.seq, self.op, self.kind, self.flags, self.t, self.dur,
         self.ret) = f[:7]
        self.args = list(f[7:14])
        (self.slot, self.abi, self.cap_kind, self.cap_src, self.cap_len,
         self.cap2_off) = f[14:20]
        self.cap = f[20][:self.cap_len]

    @property
    def name(self):
        if self.kind == KIND_MARK:
            return "MARK"
        return GD_NAME.get(self.op, f"?{self.op:#x}")

    @property
    def slot_s(self):
        return "K" if self.slot == 0xFF else str(self.slot)

    def path(self):
        """The first captured string, or None."""
        if self.cap_kind in (CAP_PATH, CAP_PATH2, CAP_PATH_DTA, CAP_CONSOLE):
            return cstr(self.cap)[0]
        return None

    def dta(self):
        """The captured DTA as a dict, or None.

        Layout: EmuTOS `include/bdosdefs.h:48-55` — 21 reserved bytes, then
        `d_attrib`, `d_time`, `d_date`, `d_length`, `d_fname[14]`.
        """
        if self.cap_kind == CAP_DTA:
            b = self.cap
        elif self.cap_kind == CAP_PATH_DTA:
            b = self.cap[self.cap2_off:]
        else:
            return None
        if len(b) < 44:
            return None
        att = b[21]
        time, date = struct.unpack("<HH", b[22:26])
        length = struct.unpack("<I", b[26:30])[0]
        name = cstr(b, 30)[0]
        return {"attrib": att, "time": time, "date": date, "length": length,
                "name": name}


def parse_header(blob):
    line = blob[:HDR_LEN].decode("ascii", "replace").rstrip()
    fields = {}
    for tok in line.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            fields[k] = v
        else:
            fields.setdefault("_tags", []).append(tok)
    return line, fields


def dos_time(t):
    return f"{(t >> 11) & 31:02d}:{(t >> 5) & 63:02d}:{(t & 31) * 2:02d}"


def dos_date(d):
    return f"{1980 + ((d >> 9) & 127)}-{(d >> 5) & 15:02d}-{d & 31:02d}"


def detail(r):
    """The per-opcode field decoding.

    The capture side is deliberately dumb (registers plus a bounded byte
    payload); all opcode knowledge lives here, so adding an interpretation
    never means reflashing the machine.
    """
    a = r.args
    op = r.op

    if r.kind == KIND_MARK:
        if a[0] == MARK_FLUSH:
            return (f"tracer wrote C:\\GDTRACE.BIN here ({a[1]} records) — "
                    f"free space, FAT and a directory entry moved")
        return f"mark {a[0]}"

    p = r.path()
    q = f'"{p}"' if p is not None else ""

    if op == 0x3D:  # Fopen
        m = a[1] & 0x07
        return f"{q} mode={a[1] & 0x77:#04x}({OPEN_MODE.get(m, '?')}) -> handle {err(r.ret)}"
    if op == 0x3C:  # Fcreate
        return f"{q} attr={a[1] & 0xFF:#04x}({attrs(a[1])}) -> handle {err(r.ret)}"
    if op == 0x3E:  # Fclose
        return f"handle={a[0] & 0xFFFF} -> {err(r.ret)}"
    if op in (0x3F, 0x40):  # Fread / Fwrite
        want = a[1] & 0xFFFFFFFF
        if want >= 0x80000000:
            want -= 1 << 32
        s = (f"handle={a[0] & 0xFFFF} len={want} buf={a[2]:#x} "
             f"-> {err(r.ret)}")
        if r.cap_len:
            # Only the first HEAD_CHARS bytes inline: the point of the head is
            # to identify the content at a glance, and a 160-byte binary dump
            # per line makes the trace unreadable. `--hex` shows all of it.
            shown = r.cap[:HEAD_BYTES]
            s += '  head="' + printable(shown) + '"'
            if r.flags & F_CLAMPED or len(shown) < len(r.cap):
                s += " …"
        # A short *write* is a defect signal; a short *read* is just the end
        # of the file, and flagging it would train the reader to ignore the
        # flag that matters.
        if op == 0x40 and 0 <= r.ret != want:
            s += "   <-- SHORT WRITE"
        elif op == 0x3F and 0 <= r.ret < want:
            s += "   (end of file)"
        return s
    if op == 0x42:  # Fseek
        n = a[0] & 0xFFFFFFFF
        if n >= 0x80000000:
            n -= 1 << 32
        return (f"handle={a[1] & 0xFFFF} off={n} "
                f"whence={a[2] & 0xFFFF}({SEEK_MODE.get(a[2] & 0xFFFF, '?')}) "
                f"-> pos {err(r.ret)}")
    if op == 0x41:  # Fdelete
        return f"{q} -> {err(r.ret)}"
    if op == 0x43:  # Fattrib
        wrt = a[1] & 0xFFFF
        s = f"{q} {'SET' if wrt else 'GET'}"
        if wrt:
            s += f" mode={a[2] & 0xFF:#04x}({attrs(a[2])})"
        s += f" -> {err(r.ret)}"
        if r.ret >= 0:
            s += f" ({attrs(r.ret)})"
        return s
    if op == 0x56:  # Frename
        src, nxt = cstr(r.cap)
        dst = cstr(r.cap, nxt)[0] if r.cap_kind == CAP_PATH2 else ""
        return f'"{src}" -> "{dst}"  = {err(r.ret)}'
    if op == 0x57:  # Fdatime
        wrt = a[2] & 0xFFFF
        s = f"handle={a[1] & 0xFFFF} {'SET' if wrt else 'GET'}"
        if len(r.cap) >= 4:
            t, d = struct.unpack("<HH", r.cap[:4])
            s += f" {dos_date(d)} {dos_time(t)}"
        return s + f" -> {err(r.ret)}"
    if op == 0x4E:  # Fsfirst
        s = f"{q} attr={a[1] & 0xFF:#04x}({attrs(a[1])}) -> {err(r.ret)}"
        d = r.dta()
        if d:
            s += (f'\n{"":>52}DTA: "{d["name"]}" {d["length"]} bytes '
                  f'attr={d["attrib"]:#04x}({attrs(d["attrib"])}) '
                  f'{dos_date(d["date"])} {dos_time(d["time"])}')
        return s
    if op == 0x4F:  # Fsnext
        s = f"-> {err(r.ret)}"
        d = r.dta()
        if d:
            s += (f'\n{"":>52}DTA: "{d["name"]}" {d["length"]} bytes '
                  f'attr={d["attrib"]:#04x}({attrs(d["attrib"])}) '
                  f'{dos_date(d["date"])} {dos_time(d["time"])}')
        return s
    if op == 0x1A:  # Fsetdta
        return f"dta={a[0]:#x}"
    if op == 0x2F:  # Fgetdta
        return f"-> dta={r.ret & 0xFFFFFFFF:#x}"
    if op == 0x36:  # Dfree
        s = f"drv={a[1] & 0xFFFF} buf={a[0]:#x} -> {err(r.ret)}"
        if len(r.cap) >= 16:
            free, total, secsz, cl = struct.unpack("<IIII", r.cap[:16])
            s += (f"  free={free} total={total} clusters, "
                  f"{secsz} B/sector, {cl} sectors/cluster "
                  f"= {free * secsz * cl} bytes free")
        return s
    if op == 0x0E:  # Dsetdrv
        return f"drv={a[0] & 0xFFFF} ({chr(65 + (a[0] & 0xFFFF))}:) -> map={r.ret:#x}"
    if op == 0x19:  # Dgetdrv
        return f"-> {r.ret} ({chr(65 + (r.ret & 0xFF))}:)"
    if op in (0x39, 0x3A, 0x3B):  # Dcreate / Ddelete / Dsetpath
        return f"{q} -> {err(r.ret)}"
    if op == 0x47:  # Dgetpath
        return f"drv={a[1] & 0xFFFF} -> {err(r.ret)}  {q}"
    if op == 0x09:  # Cconws
        text = r.cap[:-1] if r.cap.endswith(b"\x00") else r.cap
        return '"' + printable(text) + '"'
    if op in (0x48, 0x44):  # Malloc / Mxalloc
        n = a[0] & 0xFFFFFFFF
        if n >= 0x80000000:
            n -= 1 << 32
        return f"len={n} -> {err(r.ret)}"
    if op in (0x49, 0x4A):  # Mfree / Mshrink
        return f"addr={a[0]:#x} len={a[1]} -> {err(r.ret)}"
    if op == 0x4B:  # Pexec (a FRAME record: no result)
        m = a[0] & 0xFFFF
        s = f"mode={m}({PEXEC_MODE.get(m, '?')})"
        if r.cap_len:
            path, nxt = cstr(r.cap)
            s += f' "{path}"'
            if r.cap_kind == CAP_PATH2:
                tail = cstr(r.cap, nxt)[0].encode("latin-1")
                s += ' tail="' + printable(tail) + '"'
        return s
    if op in (0x4C, 0x00, 0x31):  # Pterm / Pterm0 / Ptermres
        return f"rc={a[0] & 0xFFFF}"
    if op == 0x45:  # Fdup
        return f"handle={a[0] & 0xFFFF} -> {err(r.ret)}"
    if op == 0x46:  # Fforce
        return f"std={a[0] & 0xFFFF} <- handle={a[1] & 0xFFFF} -> {err(r.ret)}"

    # Generic: the registers the call actually used plus its result.
    used = [f"{v:#x}" for v in a[:4]]
    s = f"args=[{' '.join(used)}] -> {err(r.ret)}"
    if r.cap_len:
        s += f'  cap="{printable(r.cap)}"'
    return s


def render(recs, hz, t0, hexdump=False, gaps=True, out=sys.stdout):
    def ms(t):
        return (t - t0) * 1000.0 / hz if hz else 0.0

    print(f"{'seq':>7} {'t_ms':>11} {'dur_us':>9} {'sl':>3} {'op':>5} "
          f"{'name':<10} detail", file=out)
    print("-" * 118, file=out)
    prev_seq = None
    for r in recs:
        if gaps and prev_seq is not None and r.seq != prev_seq + 1:
            print(f"{'':>7} {'':>11} {'':>9} {'':>3} {'':>5} "
                  f"{'~~~~~~~~~~':<10} {r.seq - prev_seq - 1} records lost "
                  f"here (ring wrapped)", file=out)
        prev_seq = r.seq
        dur_us = (r.dur * 1_000_000 // hz) if hz else 0
        tag = ""
        if r.kind == KIND_FRAME:
            tag = "*"          # no result: the call returns by frame mutation
        elif r.kind == KIND_MARK:
            tag = "#"
        if r.flags & F_ILP32:
            tag += "3"
        print(f"{r.seq:>7} {ms(r.t):>11.3f} {dur_us:>9} {r.slot_s:>3} "
              f"{r.op:>5x} {r.name + tag:<10} {detail(r)}", file=out)
        if hexdump and r.cap_len:
            for i in range(0, len(r.cap), 32):
                chunk = r.cap[i:i + 32]
                hexs = " ".join(f"{c:02x}" for c in chunk)
                print(f"{'':>36}{i:04x}  {hexs:<96} |{printable(chunk)}|",
                      file=out)


def summary(recs, hz, out=sys.stdout):
    calls = [r for r in recs if r.kind in (KIND_CALL, KIND_FRAME)]
    n = Counter(r.op for r in calls)
    fails = Counter(r.op for r in calls if r.kind == KIND_CALL and r.ret < 0)
    dur = Counter()
    for r in calls:
        dur[r.op] += r.dur
    print(f"{'op':>5} {'name':<10} {'calls':>7} {'failed':>7} "
          f"{'total_us':>10} {'mean_us':>9}", file=out)
    print("-" * 54, file=out)
    for op, c in n.most_common():
        tot = (dur[op] * 1_000_000 // hz) if hz else 0
        print(f"{op:>5x} {GD_NAME.get(op, '?'):<10} {c:>7} {fails[op]:>7} "
              f"{tot:>10} {tot / c:>9.1f}", file=out)
    print(f"\n{len(calls)} calls, {sum(fails.values())} of them failed",
          file=out)


def files_report(recs, hz, t0, out=sys.stdout):
    """Per-file story: every open/create, what was done through the handle,
    and what a later directory scan said the file had become.

    This is the view for a write-path defect: `Fcreate` -> `Fwrite` -> `Fclose`
    all succeeding while a subsequent `Fsfirst` reports a length of 0 is the
    whole bug, visible in four lines.
    """
    def ms(t):
        return (t - t0) * 1000.0 / hz if hz else 0.0

    # handle -> (path, opened_at, how)
    live = {}
    story = OrderedDict()

    def entry(path):
        return story.setdefault(path, {"events": [], "written": 0, "read": 0})

    for r in recs:
        if r.kind != KIND_CALL:
            continue
        if r.op in (0x3D, 0x3C) and r.ret >= 0:
            path = r.path() or "?"
            live[r.ret] = path
            e = entry(path)
            e["events"].append((r.t, "Fcreate" if r.op == 0x3C else "Fopen",
                                f"handle {r.ret}"))
        elif r.op in (0x3D, 0x3C):
            path = r.path() or "?"
            entry(path)["events"].append(
                (r.t, "Fcreate" if r.op == 0x3C else "Fopen", err(r.ret)))
        elif r.op in (0x3F, 0x40):
            h = r.args[0] & 0xFFFF
            path = live.get(h)
            if path is None:
                continue
            e = entry(path)
            want = r.args[1] & 0xFFFFFFFF
            if r.op == 0x40:
                e["written"] += max(r.ret, 0)
            else:
                e["read"] += max(r.ret, 0)
            e["events"].append((r.t, "Fwrite" if r.op == 0x40 else "Fread",
                                f"{want} asked, {err(r.ret)} done"))
        elif r.op == 0x42:
            h = r.args[1] & 0xFFFF
            path = live.get(h)
            if path:
                entry(path)["events"].append((r.t, "Fseek", err(r.ret)))
        elif r.op == 0x3E:
            h = r.args[0] & 0xFFFF
            path = live.pop(h, None)
            if path:
                entry(path)["events"].append((r.t, "Fclose", err(r.ret)))
        elif r.op in (0x4E, 0x4F):
            d = r.dta()
            if not d:
                continue
            # A directory scan is the only place the trace learns what the file
            # system now *believes* about a file, so attach it to every path
            # whose last component matches.
            for path, e in story.items():
                tailname = path.replace("/", "\\").split("\\")[-1].upper()
                if tailname == d["name"].upper():
                    e["events"].append(
                        (r.t, "seen by " + ("Fsfirst" if r.op == 0x4E
                                            else "Fsnext"),
                         f'length={d["length"]} attr={attrs(d["attrib"])}'))
        elif r.op == 0x41:
            path = r.path()
            if path:
                entry(path)["events"].append((r.t, "Fdelete", err(r.ret)))

    for path, e in story.items():
        print(f"\n{path}", file=out)
        print(f"  wrote {e['written']} bytes, read {e['read']} bytes", file=out)
        for t, what, how in e["events"]:
            print(f"    {ms(t):>10.3f} ms  {what:<16} {how}", file=out)
        # The calibration case, named where it happens.
        seen = [how for _, what, how in e["events"] if what.startswith("seen")]
        if e["written"] > 0 and seen and "length=0" in seen[-1]:
            print(f"    !! {e['written']} bytes were written, but the "
                  f"directory entry says length=0", file=out)


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file")
    ap.add_argument("--op", action="append", default=[],
                    help="only this GEMDOS opcode (decimal, 0x.., or a name "
                         "like Fopen); repeatable")
    ap.add_argument("--tail", type=int, help="only the last N records")
    ap.add_argument("--head", type=int, help="only the first N records")
    ap.add_argument("--slot", type=int, help="only this process slot")
    ap.add_argument("--errors", action="store_true",
                    help="only calls that returned a negative value")
    ap.add_argument("--summary", action="store_true",
                    help="per-opcode call counts and cost instead of the trace")
    ap.add_argument("--files", action="store_true",
                    help="per-file open/read/write/close story")
    ap.add_argument("--hex", action="store_true",
                    help="hex-dump every captured payload")
    args = ap.parse_args()

    blob = open(args.file, "rb").read()
    if not blob.startswith(FORMAT):
        sys.exit(f"{args.file}: not a NewTOS GEMDOS trace "
                 f"(header {blob[:24]!r})")
    line, hdr = parse_header(blob)
    print(line)
    hz = int(hdr.get("hz", 0))
    t0 = int(hdr.get("t0", 0))
    if hdr.get("sup", "0") != "0":
        print(f"WARNING: {hdr['sup']} hook entries were suppressed by the "
              f"re-entrancy guard — the tracer saw itself.")
    body = blob[HDR_LEN:]
    recs = [Rec(body[i:i + REC_LEN])
            for i in range(0, len(body) - REC_LEN + 1, REC_LEN)]
    print(f"{len(recs)} records, {hz} Hz timer\n")

    names = {v.lower(): k for k, v in GD_NAME.items()}
    filtered = False
    ops = set()
    for o in args.op:
        if o.lower() in names:
            ops.add(names[o.lower()])
        else:
            ops.add(int(o, 0))
    if ops:
        recs = [r for r in recs if r.op in ops]
        filtered = True
    if args.slot is not None:
        recs = [r for r in recs if r.slot == args.slot]
        filtered = True
    if args.errors:
        recs = [r for r in recs if r.kind == KIND_CALL and r.ret < 0]
        filtered = True
    if args.head:
        recs = recs[:args.head]
    if args.tail:
        recs = recs[-args.tail:]
        filtered = True

    if args.summary:
        summary(recs, hz)
    elif args.files:
        files_report(recs, hz, t0)
    else:
        # The "records lost" note means the ring wrapped, so it may only be
        # drawn on an unfiltered view — a filter makes the sequence numbers
        # jump for a completely different reason.
        render(recs, hz, t0, hexdump=args.hex, gaps=not filtered)


if __name__ == "__main__":
    main()
