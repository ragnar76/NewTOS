# BIOS Trap-13 Dispatcher

> **Phase 00 deliverable D2 (Trap 13).**
> Source: EmuTOS @ `1a7484ab`.
>
> The BIOS dispatcher is a single assembly function that performs an
> indirect call through `bios_vecs[]` (a 12-entry table). There is no
> C-level dispatcher.

---

## 1. Vector installation

The trap-13 vector is set up in the early boot path through the
generic exception-vector table in `bios/vectors.S` (the table is
installed by `vecs_init()` in `bios/bios.c`). The handler is
`_biostrap` at `bios/vectors.S:1268`.

The vector for trap 14 (XBIOS) is installed in the same step and shares
the trampoline body (see `_dispatcher.md` for XBIOS).

---

## 2. `_biostrap` entry (`bios/vectors.S:1268`)

```asm
_biostrap:
    move.w  _bios_ent, d1            ; bios_ent = ARRAY_SIZE(bios_vecs) = 12
    lea     _bios_vecs, a0           ; a0 -> table base
    jra     biosxbios
```

- `_bios_ent` (`bios/bios.c:1267`) = `ARRAY_SIZE(bios_vecs)` = 12.
- `_bios_vecs` (`bios/bios.c:1252`) is the function pointer table.

Falls through to the shared `biosxbios:` body at `bios/vectors.S:1282`.

---

## 3. Shared `biosxbios` trampoline (`bios/vectors.S:1282-1361`)

Stack frame on trap entry (m68k):

```
+(2 long-frame): format/vector word
+2: PC of caller (LONG)
 0: SR of caller (WORD)
```

Above the exception frame, on the caller's stack (USP if user mode):

```
+(2*N): arg N
…
+2: arg 1
 0: function ID (WORD)
```

Trampoline steps:

1. Load `_savptr.w` (a pointer in low memory ST-RAM, 4 bytes at the
   `_savptr` symbol) into `A1`. This is the **per-trap save area
   stack-pointer**, which lets nested BIOS↔XBIOS calls share state.
2. Pop the SR off the supervisor stack into D0; store it into the
   save area (`-(a1)`).
3. Pop the return PC off the supervisor stack and save it.
4. If `_longframe.w` is non-zero (CPU ≥ 68010), pop the format/vector
   word and save it.
   ColdFire takes a different branch (`bx_cf_nofiller`) that always
   discards the filler word.
5. Save `D3-D7/A3-A7` into the save area
   (10 longwords on m68k, also 10 on ColdFire via `lea -40(a1),a1`).
6. Update `_savptr.w` to the new top of save area.
7. Branch on `bit 13` of saved SR: if user mode, `move.l usp, sp` so
   that the C function sees the caller's user stack as its argument
   stack.
8. `move.w (sp)+, d0` consumes the function ID from the argument
   stack. `cmp.w d1, d0` against the table size; if `d0 >= d1`,
   branch `bx_ret_exc` (return whatever D0 carries, which becomes the
   "function not implemented" return = function number itself —
   identical to the `xbios_unimpl` shim).
9. Index into the table:
   ```
   move.l d0, d1
   add.l  d1, d1
   add.l  d1, d1     ; *4 (LONG-pointer table)
   add.l  d1, a0
   move.l (a0), a0   ; a0 = handler address
   jsr    (a0)
   ```
10. After the handler returns (with result in D0), restore registers
    from the save area, restore exception frame, `rte`.

The save-area mechanism lets the caller's full register set survive
across the BIOS call **without** the C compiler having to emit
volatile-register save code in every BIOS handler.

---

## 4. `bios_vecs[]` table (`bios/bios.c:1252`)

```c
const PFLONG bios_vecs[] = {
    VEC(bios_0, getmpb),       /* 0x00 */
    VEC(bios_1, bconstat),     /* 0x01 */
    VEC(bios_2, bconin),       /* 0x02 */
    VEC(bios_3, bconout),      /* 0x03 */
    VEC(bios_4, lrwabs),       /* 0x04 */
    VEC(bios_5, setexc),       /* 0x05 */
    VEC(bios_6, tickcal),      /* 0x06 */
    VEC(bios_7, getbpb),       /* 0x07 */
    VEC(bios_8, bcostat),      /* 0x08 */
    VEC(bios_9, mediach),      /* 0x09 */
    VEC(bios_a, drvmap),       /* 0x0A */
    VEC(bios_b, kbshift),      /* 0x0B */
};
const UWORD bios_ent = ARRAY_SIZE(bios_vecs);
```

`VEC(wrapper, direct)` expands to `(PFLONG)direct` for normal builds.
With `DBGBIOS` defined, it expands to the wrapper which prints debug
info before calling the real function. NewTOS extracts against
`DBGBIOS = 0` (default).

| ID    | C function   | TOS name    | Defined in       | Notes                                  |
|------:|--------------|-------------|------------------|----------------------------------------|
| 0x00  | `getmpb`     | Getmpb      | `bios/bios.c`    | fills caller-supplied MPB              |
| 0x01  | `bconstat`   | Bconstat    | `bios/bios.c`    | indirect via `bconstat_vec[handle]`    |
| 0x02  | `bconin`     | Bconin      | `bios/bios.c`    | indirect via `bconin_vec[handle]`      |
| 0x03  | `bconout`    | Bconout     | `bios/bios.c`    | indirect via `bconout_vec[handle]`     |
| 0x04  | `lrwabs`     | Rwabs       | `bios/bios.c`    | extended-sector long form              |
| 0x05  | `setexc`     | Setexc      | `bios/bios.c`    | direct vector poke                     |
| 0x06  | `tickcal`    | Tickcal     | `bios/bios.c`    | constant `20`                          |
| 0x07  | `getbpb`     | Getbpb      | `bios/bios.c`    | calls `hdv_bpb`                        |
| 0x08  | `bcostat`    | Bcostat     | `bios/bios.c`    | indirect via `bcostat_vec[handle]`     |
| 0x09  | `mediach`    | Mediach     | `bios/bios.c`    | calls `hdv_mediach`                    |
| 0x0A  | `drvmap`     | Drvmap      | `bios/bios.c`    | returns `_drvbits`                     |
| 0x0B  | `kbshift`    | Kbshift     | `bios/bios.c`    | reads/writes `kbshift` byte at $E1B    |

There is no `xbios_unimpl`-equivalent for BIOS — every entry has a
real handler. Out-of-range IDs are caught by the `cmp.w d1,d0` test
and return the function number unchanged.

---

## 5. Indirection through device-vector tables

For character-device BIOS calls (`bconstat`/`bconin`/`bconout`/`bcostat`),
the `bios_*` C functions further dispatch through a per-handle table:

- `bconstat_vec[8]` initialised by `chardev_init()` at boot
  (`bios/chardev.c:55`).
- `bconin_vec[8]`, `bconout_vec[8]`, `bcostat_vec[8]` similarly.
- Handle range `0..7` → table lookup; `>= BCONMAP_START_HANDLE` (build
  config `BCONMAP_AVAILABLE`) → consult the `bconmap_root.maptab[]`
  table; otherwise return 0 (no-op).

| Handle | Device                                              |
|-------:|-----------------------------------------------------|
| 0      | Centronics / parallel printer (`parport.c`)         |
| 1      | RS-232 / serial (`serport.c`)                       |
| 2      | Console / screen (`conout.c:cputc`)                 |
| 3      | MIDI out (`midi.c`) / IKBD in (`ikbd.c` — swapped)  |
| 4      | IKBD out (`ikbd.c`) / MIDI in (`midi.c` — swapped)  |
| 5      | "raw" console (`bconout5` → `ascii_out` no VT52)    |
| 6,7    | unused, `char_dummy` / `charout_dummy`              |

Note the swap of MIDI vs. IKBD between in and out: see comment in
`bios/chardev.c:39` ("note that IKBD and MIDI bcostat() are swapped!").

---

## 6. Source-location summary

| Concern                         | File:line                       |
|---------------------------------|---------------------------------|
| Trap-13 entry                   | `bios/vectors.S:1268`           |
| Shared trampoline               | `bios/vectors.S:1282`           |
| Save-area pointer               | `bios/vectors.S:1448` (`_trap_save_area`) |
| Function table                  | `bios/bios.c:1252`              |
| Table size                      | `bios/bios.c:1267` (`_bios_ent`)|
| Per-handle device tables        | `bios/chardev.c:30-44`          |
| Char-device init                | `bios/chardev.c:64-79`          |

---

## NewTOS ABI mapping notes

(Empty during Phase 00. Filled in during Phase 07.)
