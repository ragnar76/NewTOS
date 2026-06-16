# GEMDOS Trap-1 Dispatcher

> **Phase 00 deliverable D2 (Trap 1).**
> Source: EmuTOS @ `1a7484ab`.
>
> The GEMDOS dispatcher is two-stage: an assembly trampoline
> (`bdos/rwa.S:_enter`) saves caller state into the basepage, then calls
> the C function `osif()` (`bdos/bdosmain.c:498`) which performs
> table-driven dispatch.

---

## 1. Vector installation

`bdos/bdosmain.c:339-352` (`osinit_before_xmaddalt()`):

```c
Setexc(0x21, (long)enter);                 /* take over TRAP #1 */
old_trap2 = (PFVOID) Setexc(0x22, bdos_trap2);
```

- Trap 1 vector (CPU exception number 33 → byte address 0x84) →
  `_enter` in `rwa.S`.
- Trap 2 vector (number 34) → `_bdos_trap2` to intercept `Pterm0()` via
  the undocumented "trap-2 with d0=0" CP/M-68K compatibility path.
  Other trap-2 calls fall through to the previous handler.

Citation: `bdos/rwa.S:159` (`_enter:` label) and `bdos/rwa.S:145`
(`_bdos_trap2:` label).

---

## 2. Stack frame on entry

The m68k CPU pushes onto the supervisor stack:

```
+6 (long-frame only): format/vector word
+2: PC of caller (return address, LONG)
 0: SR of caller (WORD)
```

For a 68000 the format-word is absent (4-byte exception frame).
For 68010+ EmuTOS uses `_longframe` to detect the longer frame
(`bios/vectors.S:1287`-onward — but this affects BIOS/XBIOS; trap 1
takes a different path).

**Above the exception frame**, on the caller's stack (USP if user
mode, SSP if supervisor):

```
+(2*N): arg N (word, may be the high half of a long)
…
+2: arg 1 (word)
 0: function ID (word)
```

The dispatcher reads function ID from `pw[0]` and successive args from
`pw[1]`, `pw[2]`, … as 16-bit words. LONG/pointer arguments occupy two
consecutive slots and are reassembled by `*(long*)&pw[i]` or
`*((char**)&pw[i])` casts.

---

## 3. `_enter` trampoline (`bdos/rwa.S:159`)

Responsibilities:

1. Patch the supervisor exception frame for buggy 68000 stack
   pushes (`fix_trap` at top of `_enter`).
2. Detect undocumented function 0x20 ("toggle/inquire processor/stack
   state", aka `Super`):
   - If invoked from user mode and the caller's USP top word is 0x20,
     branch to `x20_usr` and bypass the C dispatcher.
   - Same for supervisor-mode entry (`x20_sup`).
   This is the only function not handled by `osif()` and therefore not
   present in `funcs[]`.
3. For all other calls:
   - Save D0 + A3-A6 into `P_REGSAVE(a6)` of `_run` (current basepage).
   - Save the caller's stack pointer into `P_A7SAVE(a6)`.
   - Switch to the BDOS internal stack (`fstrt`, 4000 bytes BSS at
     `bdos/rwa.S:fstrt`).
   - Build a pointer to the caller's argument area in A0.
   - Call `osif(a0)` (`callos:` at `bdos/rwa.S:206`).
4. Fall through to `_gouser` to restore caller state and `rte`.

Stack-pointer convention preserved across the call so that programs
can call into BDOS from either mode.

---

## 4. `osif()` C dispatcher (`bdos/bdosmain.c:431`)

```c
long osif(short *pw);
long osif(short *pw)
{
    int fn = pw[0];
    if (fn > MAX_FNCALL)               // 0x57 in this build
        return EINVFN;                 // -32

    if (setjmp(errbuf)) { ... }        // hard-error recovery

    f = &funcs[fn];                    // look up FND record
    typ = f->stdio_typ;

    /* (1) standard-handle redirection: see Step 5 below */

    /* (2) Fopen/Fcreate special-name interception (CON: AUX: PRN:) */

    /* (3) function-by-arity dispatch: */
    switch (f->wparms) {
        case 0: rc = f->fncall.v(); break;
        case 1: rc = f->fncall.w(pw[1]); break;
        case 2: rc = f->fncall.ww(pw[1], pw[2]); break;
        case 3: rc = f->fncall.www(pw[1], pw[2], pw[3]); break;
        case 4: rc = f->fncall.wwww(pw[1]…pw[4]); break;
        case 5: rc = f->fncall.wwwww(pw[1]…pw[5]); break;
        case 7: rc = f->fncall.wwwwwww(pw[1]…pw[7]); break;
        default: rc = EINTRN;
    }
    return rc;
}
```

`MAX_FNCALL` is `ARRAY_SIZE(funcs) - 1` → highest entry in this build is
`0x57` (`xgsdtof`), giving a table size of 88.

Unimplemented IDs map to `ni()` (`bdos/bdosmain.c:316`) which returns
`EINVFN` (-32).

---

## 5. Standard-handle (`p_uft[]`) interception

Before calling the function, `osif()` checks whether the call refers
to a standard handle that has been redirected via `Fforce` (0x46) /
`Fdup` (0x45). The relevant test is at `bdos/bdosmain.c:498-575`:

```c
if (typ && fn && ((fn<12) || ((fn>=16) && (fn<=19)))) {
    h = run->p_uft[typ & 0x7f];
    if (h > 0) {
        /* standard device routed to a file: handle inline */
        switch (fn) {
            case 1: case 3: case 7: case 8:  /* read 1 byte */
                if (xread(h, 1L, &ctmp) != 1L) return EOF_INDICATOR;
                return ctmp;
            case 6:  /* Crawio: 0xFF=read else write */
                if (pw[1] != 0xFF) goto rawout;
                /* else FALLTHROUGH to read */
            case 2: case 4: case 5:  /* write 1 byte */
            rawout: xwrite(h, 1L, ((char*)&pw[1]) + 1);  return 0;
            case 9:  /* Cconws: write strlen(p) bytes */
                pb2 = *((char**)&pw[1]);
                xwrite(h, strlen(pb2), pb2);  return 0;
            case 10: /* Cconrs: line-edit emulation over file */
                /* manual byte-by-byte read with CR-terminate */
                ...
            case 11: case 18:                       /* status read */
                if (eof(h)) return 0L;
                FALLTHROUGH;
            case 16: case 17: case 19:              /* status write */
                return -1L;
        }
    }
    typ = 0;  /* fall through to regular dispatch */
}
```

The `0x80`/`0x81`/`0x82`/`0x83` `stdio_typ` flags in `funcs[]` encode
which standard handle each function uses. The dispatcher's interception
means **applications that redirect stdin/stdout via `Fforce` see the
console functions automatically reading from / writing to the file**.

---

## 6. Function table (`funcs[]`, `bdos/bdosmain.c:142-289`)

Each entry is:

```c
typedef struct {
    union {
        long (*v)(void);
        long (*w)(short);
        long (*ww)(short,short);
        ...
    } fncall;
    UBYTE stdio_typ;   /* 0, 0x80, 0x81, 0x82, or 0x83 */
    UBYTE wparms;      /* number of WORD-sized parameter slots */
} FND;
```

`stdio_typ` bits:
- `0x80` = function consumes a standard handle.
- low bits: `0` = stdprn, `1` = stdout, `2` = stdaux, `3` = stdprn (written
  as `0x80|0,0x80|1,…`). Concretely: 0x80 stdin, 0x81 stdout, 0x82 stdaux,
  0x83 stdprn.
- `0` = no standard-handle interception.

`wparms` is in WORDs; LONG/ptr count as 2 WORDs.

The full table (only entries that resolve to functions, with build
configuration `CONF_WITH_TT` = 1 baseline assumed; flagged as cfg where
relevant):

| ID    | EmuTOS func   | TOS name    | typ  | wparms | Notes                       |
|------:|---------------|-------------|------|-------:|-----------------------------|
| 0x00  | `x0term`      | Pterm0      | 0    |   0    |                             |
| 0x01  | `xconin`      | Cconin      | 0x80 |   0    |                             |
| 0x02  | `xconout`     | Cconout     | 0x81 |   1    |                             |
| 0x03  | `xauxin`      | Cauxin      | 0x82 |   0    |                             |
| 0x04  | `xauxout`     | Cauxout     | 0x82 |   1    |                             |
| 0x05  | `xprtout`     | Cprnout     | 0x83 |   1    |                             |
| 0x06  | `xrawio`      | Crawio      | 0    |   1    |                             |
| 0x07  | `xrawcin`     | Crawcin     | 0x80 |   0    |                             |
| 0x08  | `xnecin`      | Cnecin      | 0x80 |   0    |                             |
| 0x09  | `xconws`      | Cconws      | 0x81 |   2    | (long ptr)                  |
| 0x0A  | `xconrs`      | Cconrs      | 0x80 |   2    | (long ptr)                  |
| 0x0B  | `xconstat`    | Cconis      | 0x80 |   0    |                             |
| 0x0E  | `xsetdrv`     | Dsetdrv     | 0    |   1    |                             |
| 0x10  | `xconostat`   | Cconos      | 0x81 |   0    |                             |
| 0x11  | `xprtostat`   | Cprnos      | 0x83 |   0    |                             |
| 0x12  | `xauxistat`   | Cauxis      | 0x82 |   0    |                             |
| 0x13  | `xauxostat`   | Cauxos      | 0x82 |   0    |                             |
| 0x14  | `xmaddalt`    | Maddalt     | 0    |   4    | cfg `CONF_WITH_ALT_RAM`     |
| 0x15  | `srealloc`    | Srealloc    | 0    |   2    | cfg `CONF_WITH_VIDEL`       |
| 0x19  | `xgetdrv`     | Dgetdrv     | 0    |   0    |                             |
| 0x1A  | `xsetdta`     | Fsetdta     | 0    |   2    |                             |
| 0x2A  | `xgetdate`    | Tgetdate    | 0    |   0    |                             |
| 0x2B  | `xsetdate`    | Tsetdate    | 0    |   1    |                             |
| 0x2C  | `xgettime`    | Tgettime    | 0    |   0    |                             |
| 0x2D  | `xsettime`    | Tsettime    | 0    |   1    |                             |
| 0x2F  | `xgetdta`     | Fgetdta     | 0    |   0    |                             |
| 0x30  | `xgetver`     | Sversion    | 0    |   0    |                             |
| 0x31  | `xtermres`    | Ptermres    | 0    |   3    |                             |
| 0x36  | `xgetfree`    | Dfree       | 0    |   3    |                             |
| 0x39  | `xmkdir`      | Dcreate     | 0    |   2    |                             |
| 0x3A  | `xrmdir`      | Ddelete     | 0    |   2    |                             |
| 0x3B  | `xchdir`      | Dsetpath    | 0    |   2    |                             |
| 0x3C  | `xcreat`      | Fcreate     | 0    |   3    |                             |
| 0x3D  | `xopen`       | Fopen       | 0    |   3    |                             |
| 0x3E  | `xclose`      | Fclose      | 0    |   1    |                             |
| 0x3F  | `xread`       | Fread       | 0x82 |   5    | (typ flag for redirection)  |
| 0x40  | `xwrite`      | Fwrite      | 0x82 |   5    |                             |
| 0x41  | `xunlink`     | Fdelete     | 0    |   2    |                             |
| 0x42  | `xlseek`      | Fseek       | 0x81 |   4    |                             |
| 0x43  | `xchmod`      | Fattrib     | 0    |   4    |                             |
| 0x44  | `xmxalloc`    | Mxalloc     | 0    |   3    |                             |
| 0x45  | `xdup`        | Fdup        | 0    |   1    |                             |
| 0x46  | `xforce`      | Fforce      | 0    |   2    |                             |
| 0x47  | `xgetdir`     | Dgetpath    | 0    |   3    |                             |
| 0x48  | `xmalloc`     | Malloc      | 0    |   2    |                             |
| 0x49  | `xmfree`      | Mfree       | 0    |   2    |                             |
| 0x4A  | `xsetblk`     | Mshrink     | 0    |   5    |                             |
| 0x4B  | `xexec`       | Pexec       | 0    |   7    |                             |
| 0x4C  | `xterm`       | Pterm       | 0    |   1    |                             |
| 0x4E  | `xsfirst`     | Fsfirst     | 0    |   3    |                             |
| 0x4F  | `xsnext`      | Fsnext      | 0    |   0    |                             |
| 0x56  | `xrename`     | Frename     | 0    |   5    |                             |
| 0x57  | `xgsdtof`     | Fdatime     | 0    |   4    |                             |

All other entries (0x0C, 0x0D, 0x0F, 0x16-0x18, 0x1B-0x29, 0x2E, 0x32-0x35,
0x37, 0x38, 0x48-0x4D minus 0x48/0x49/0x4A, 0x4D, 0x50-0x55) → `ni` →
`EINVFN`.

`Super` (0x20) is **not** in `funcs[]` because it's intercepted by
`_enter` itself.

---

## 7. Special-name interception (`Fopen`/`Fcreate`)

Before dispatching `xopen` (0x3D) or `xcreat` (0x3C), `osif()` checks
the path against a small table (`bdos/bdosmain.c:96-104`):

```c
{ "CON:", 0x0000ffffL },  { "con:", 0x0000ffffL },
{ "AUX:", 0x0000fffeL },  { "aux:", 0x0000fffeL },
{ "PRN:", 0x0000fffdL },  { "prn:", 0x0000fffdL },
```

If matched, `osif()` returns the synthetic device handle directly
without calling `xopen`/`xcreat`. Mixed-case names (`Con:`) are not
recognised.

---

## 8. Hard-error recovery (`setjmp`/`longjmp`)

`osif()` runs the function under `setjmp(errbuf)`. Any code path may
`longjmp(errbuf, errcode)` from within `_iorec` / FAT code. On longjmp:

- If `errcode == E_CHNG` (media change), the dispatcher rebuilds the
  drive's BPB + frees DND tree and OFDs, then `goto restrt` to retry
  the original function. (`bdos/bdosmain.c:454-489`).
- Otherwise, the BCBs for `errdrv` are invalidated and the error is
  returned to caller as if it were the function's return value.

For console functions this longjmp path is unreachable; only file/disk
operations use it.

---

## 9. Return path

`osif()` returns a LONG. `_enter` falls through to `_gouser`
(`bdos/rwa.S:240`) which:
1. Stores the returned LONG in `P_D0SAVE(a6)`.
2. Restores caller's stack pointer + saved registers.
3. Restores SR and PC from the saved exception frame.
4. Executes `rte` (or the ColdFire-specific moral equivalent).

The caller resumes with D0 = return value; D1, D2, A0, A1, A2 are not
preserved (consistent with documented TOS calling convention).

---

## 10. Source-location summary

| Concern                        | File:line                    |
|--------------------------------|------------------------------|
| Trap-1 vector install          | `bdos/bdosmain.c:339`        |
| Asm trampoline                 | `bdos/rwa.S:159` (`_enter`)  |
| C dispatcher                   | `bdos/bdosmain.c:431`        |
| Function table                 | `bdos/bdosmain.c:142-289`    |
| FND struct                     | `bdos/bdosmain.c:117-129`    |
| Stdhandle redirection          | `bdos/bdosmain.c:498-575`    |
| Special name interception      | `bdos/bdosmain.c:646-660`    |
| `Super`/`x20` short-circuit    | `bdos/rwa.S:175-201`         |
| Hard-error recovery            | `bdos/bdosmain.c:454-489`    |
| Return / register restore      | `bdos/rwa.S:240` (`_gouser`) |

---

## NewTOS ABI mapping notes

(Empty during Phase 00. Filled in during Phase 02.)
