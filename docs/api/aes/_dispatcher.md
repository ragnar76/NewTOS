# AES Trap Dispatcher (GEM trap, function 200)

> **PHASE-00b deliverable D8 (AES), Batch 0.**
> Source: EmuTOS @ `1a7484ab`.
>
> The AES shares the **GEM trap** (m68k `trap #2`) with the VDI. The two
> are distinguished by the selector in `d0`: `d0 = 115` → VDI (`gsx2.c`),
> `d0 = 200` → AES (`gemsuper.c`). `d1` holds a pointer to a parameter
> block. This file documents the AES side; for the VDI parameter-block
> mechanics see [`../vdi/_dispatcher.md`](../vdi/_dispatcher.md).

---

## 1. The AES parameter block (`AESPB`)

`aes/gemsuper.h:13-20`. Unlike GEMDOS/BIOS/XBIOS (register ABI), the AES
takes a **parameter block of six array pointers**, exactly like the VDI:

```c
typedef struct {
    WORD *control;   /* opcode + array lengths */
    WORD *global;    /* AESGLOBAL — per-application state */
    WORD *intin;     /* integer inputs  */
    WORD *intout;    /* integer outputs */
    LONG *addrin;    /* address inputs  (pointers) */
    LONG *addrout;   /* address outputs (pointers) */
} AESPB;
```

### `control[]` layout (`include/aesdefs.h:121-135`)

| Index | Macro      | Meaning                          |
|------:|------------|----------------------------------|
| 0     | `OP_CODE`  | AES function opcode (10..127)    |
| 1     | `IN_LEN`   | number of `intin` words          |
| 2     | `OUT_LEN`  | number of `intout` words         |
| 3     | `AIN_LEN`  | number of `addrin` longs         |
| 4     | (addrout)  | number of `addrout` longs        |

Maximum array sizes copied by the dispatcher:

| Array    | Macro     | Size |
|----------|-----------|-----:|
| control  | `C_SIZE`  | 4 (5 words used; the copy is `C_SIZE`) |
| intin    | `I_SIZE`  | 16   |
| intout   | `O_SIZE`  | 7    |
| addrin   | `AI_SIZE` | 3    |
| addrout  | `AO_SIZE` | 1    |

The **return value** is `RET_CODE = int_out[0]` (`include/aesdefs.h:140`).

### `global[]` / `AESGLOBAL`

`control`/`intin`/`intout`/`addr*` are per-call; `global` points at the
calling application's **`AESGLOBAL`** (per-process AES state): AES version
(`ap_version`), concurrent-process count (`ap_count`), the application id
(`ap_id`), the private resource pointer (`ap_private`), plane count
(`ap_planes`), … Detailed in the `appl_init` spec (Batch 1).

---

## 2. Entry + dispatch

```
trap #2, d0=200, d1=AESPB*   (gemdosif.S)
        │
   super(cx, AESPB*)         aes/gemsuper.c:454  — AES entry
        │
   xif(AESPB*)               aes/gemsuper.c:428  — copy arrays in/out
        │  copies control[C_SIZE], intin[min(IN_LEN,I_SIZE)] onto the stack,
        │  calls the handler, then copies intout[O_SIZE] / addrout back.
        │
   crysbind(OP_CODE, global, control, intin, intout, addrin)
                             aes/gemsuper.c:77   — the giant opcode switch
```

`crysbind` is a single `switch(opcode)` that unpacks the `intin`/`addrin`
arguments for each function and calls the matching library routine
(`ap_*`, `ev_*`, `mn_*`, `ob_*`, `fm_*`, `gr_*`, `sc_*`, `fs_*`, `wm_*`,
`rs_*`, `sh_*`). Its `WORD` return becomes `RET_CODE` (`int_out[0]`).
There is **no "invalid opcode" error path** — an out-of-range opcode falls
through the switch and returns an undefined value (the AES is trusted).

---

## 3. The complete opcode map (`include/aesdefs.h`)

| Range | Manager | Opcodes |
|-------|---------|---------|
| 10–19 | **Application** (`appl_*`) | INIT 10, READ 11, WRITE 12, FIND 13, TPLAY 14, TRECORD 15, YIELD 17, EXIT 19 |
| 20–26 | **Event** (`evnt_*`) | KEYBD 20, BUTTON 21, MOUSE 22, MESAG 23, TIMER 24, MULTI 25, DCLICK 26 |
| 30–39 | **Menu** (`menu_*`) | BAR 30, ICHECK 31, IENABLE 32, TNORMAL 33, TEXT 34, REGISTER 35, POPUP 36, ATTACH 37, ISTART 38, SETTINGS 39 |
| 40–48 | **Object** (`objc_*`) | ADD 40, DELETE 41, DRAW 42, FIND 43, OFFSET 44, ORDER 45, EDIT 46, CHANGE 47, SYSVAR 48 |
| 50–56 | **Form** (`form_*`) | DO 50, DIAL 51, ALERT 52, ERROR 53, CENTER 54, KEYBD 55, BUTTON 56 |
| 70–79 | **Graphics** (`graf_*`) | RUBBOX 70, DRAGBOX 71, MBOX 72, GROWBOX 73, SHRINKBOX 74, WATCHBOX 75, SLIDEBOX 76, HANDLE 77, MOUSE 78, MKSTATE 79 |
| 80–82 | **Scrap** (`scrp_*`) | READ 80, WRITE 81, CLEAR 82 |
| 90–91 | **File selector** (`fsel_*`) | INPUT 90, EXINPUT 91 |
| 100–109 | **Window** (`wind_*`) | CREATE 100, OPEN 101, CLOSE 102, DELETE 103, GET 104, SET 105, FIND 106, UPDATE 107, CALC 108, NEW 109 |
| 110–114 | **Resource** (`rsrc_*`) | LOAD 110, FREE 111, GADDR 112, SADDR 113, OBFIX 114 |
| 120–127 | **Shell** (`shel_*`) | READ 120, WRITE 121, GET 122, PUT 123, FIND 124, ENVRN 125, RDEF 126, WDEF 127 |

≈ **80 functions** across 11 managers.

---

## 4. NewTOS implementation notes

- The AESPB is preserved **bit-exactly** (six array pointers; `WORD`
  arrays + two `LONG`/pointer arrays), so recompiled GEM programs see the
  same ABI.
- Trap mapping: like the VDI (`svc #2`), the AES is a parameter-block
  trap, distinguished from the VDI by the function selector (the m68k
  `d0=200` equivalent — exact NewTOS register TBD by ADR, consistent with
  the VDI selector choice).
- The AES is a GEM **application environment**: it runs on top of the VDI
  (Phase 12) for all drawing and on the process model (Phase 06) for
  `appl_*`/`shel_*`. It is the largest single subsystem; implementation is
  split into sub-phases per the [working list](./_working-list.md).
- Pointers in `addrin`/`addrout` are **user (application) pointers** on
  the Atari; NewTOS must apply the same user/kernel boundary handling as
  GEMDOS string reads (cf. the `Cconws`/`Pexec` `ldtrb` path).
