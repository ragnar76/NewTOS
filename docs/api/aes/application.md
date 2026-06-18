# AES Application Manager (`appl_*`) — opcodes 10–19

> **PHASE-00b D8, Batch 1.** Source: EmuTOS @ `1a7484ab` —
> `aes/gemaplib.c`, dispatch in `aes/gemsuper.c:88-126`, message pipe in
> `aes/gemqueue.c`, process descriptors in `aes/gempd.c`.
>
> Call convention + AESPB layout: [`_dispatcher.md`](./_dispatcher.md).
> One file per manager (≈80 AES functions total).

## `AESGLOBAL` — the application's AES state (`global[]`)

`aes/gemrslib.h:15-27`. The `global` array of the AESPB is this struct;
`appl_init` fills it.

```c
typedef struct aesglobal {
/* +0x00 */ WORD    ap_version;   /* AES version (AES_VERSION) */
/* +0x02 */ WORD    ap_count;     /* max concurrent applications (=1) */
/* +0x04 */ WORD    ap_id;        /* this application's id (= PID) */
/* +0x06 */ LONG    ap_private;   /* free for the application */
/* +0x0A */ OBJECT **ap_ptree;    /* array of resource tree addresses */
            /* --- below: internal, not advertised to apps --- */
/* +0x0E */ RSHDR  *ap_rscmem;    /* loaded .RSC file in memory */
/* +0x12 */ UWORD   ap_rsclen;    /* .RSC length */
/* +0x14 */ WORD    ap_planes;    /* screen colour planes */
/* +0x16 */ LONG    ap_3resv;     /* ptr to the AES global area (THEGLO) */
/* +0x1A */ LONG    ap_4resv;     /* reserved (AES 4.00) */
} AESGLOBAL;
```

The application's identity is its **process id** (`AESPD.p_pid`); the AES
keeps a per-application process descriptor (`AESPD`, `aes/struct.h:119-`)
with the message queue (`p_qaddr`/`p_qindex`), name (`p_name`), link
(`p_link`) and flags (`p_flags`, `AP_OPEN = 0x0001` = "between
appl_init and appl_exit").

---

## `appl_init` (10)

`gemsuper.c:90` (inline) + `gemaplib.c:54` (`ap_init`).

| | |
|--|--|
| intin | — |
| addrin | — |
| Returns | `int_out[0]` = application id (`ap_id` = PID), or `-1` on failure |

Fills `global`: `ap_version = AES_VERSION`, `ap_count = 1`,
`ap_id = rlr->p_pid` (the running process), `ap_private = DESKTOP tree's
ROOT ob_spec`, `ap_planes = gl_nplanes`, `ap_3resv = &THEGLO`. Then
`ap_init()` sets `p_flags |= AP_OPEN` and returns the PID.

`rlr` = "run-list root" = the currently running AES process descriptor.

## `appl_exit` (19)

`gemaplib.c:251` (`ap_exit`). No args; returns 1.

Tears the application down: `wm_update(BEG_UPDATE)` (lock screen),
`mn_cleanup()` (drop its menu), `wait_for_accs(AC_CLOSE)` (let desk
accessories see the close), flush any pending queued message, `all_run()`
(re-schedule others), clear `AP_OPEN`.

## `appl_read` (11) / `appl_write` (12)

`gemsuper.c:101` → `ap_rdwr()` `gemaplib.c:69`. The inter-application
**message pipe** (`gemqueue.c`).

| | |
|--|--|
| intin[0] | `ap_rwid` — target/own application id |
| intin[1] | `ap_length` — byte count |
| addrin[0] | `ap_pbuff` — message buffer |
| Returns | non-zero on success |

`appl_read` uses `MU_MESAG` (read own pipe), `appl_write` uses `MU_SDMSG`
(send to another app). Fast path: a standard **16-byte** message read
when the pipe holds exactly 16 bytes → `memcpy` + clear, return 1.
Otherwise it builds a `QPB` (queue parameter block: `qpb_ppd`/`qpb_cnt`/
`qpb_buf`) and blocks via `ev_block()` (the AES scheduler) until the I/O
completes. A 16-byte message is the GEM standard (the `msg[8]` WORD array
used by `evnt_mesag`).

## `appl_find` (13)

`gemaplib.c:95` (`ap_find`). Find an application by name.

| | |
|--|--|
| addrin[0] | `ap_pname` — 8-char application name |
| Returns | the app's PID, or `-1` if not found (or `pname == NULL`) |

Looks the name up in the process-descriptor list via `fpdnm()`.

## `appl_yield` (17)

`gemsuper.c:117` → `dsptch()`. Cooperative yield to the AES scheduler.
PC-GEM extension, gated on `CONF_WITH_PCGEM`. No args.

## `appl_tplay` (14) / `appl_trecord` (15)

`gemaplib.c:119` (`ap_tplay`) / `:189` (`ap_trecd`). Event "tape"
play/record — replay or capture a stream of `EVNTREC` (button/keyboard/
mouse/timer events) for macros/testing.

| | |
|--|--|
| addrin[0] | `ap_tbuffer` — `EVNTREC *` |
| intin[0] | `ap_tlength` — record count |
| intin[1] | `ap_tscale` (tplay only) — playback speed |

Niche; needs the input subsystem. Low priority for NewTOS.

---

## NewTOS implementation notes

- `ap_id` = the **process id** from our Phase-06 process model — the AES
  application is a process; `appl_init`/`appl_exit` bracket its life,
  exactly as `Pexec`/`Pterm` do at the GEMDOS level.
- The **message pipe** maps onto our process/IPC model; the 16-byte
  standard message is what `evnt_mesag` (Batch 2) delivers — the two are
  designed together.
- `AESGLOBAL` is the application-supplied `global[]` array (in the app's
  memory) — a **user pointer**; apply the same boundary handling as other
  AES `addr*` pointers (see `_dispatcher.md` §4).
- The desk-accessory machinery (`wait_for_accs`, `AC_CLOSE`) presumes
  concurrent AES processes; NewTOS bring-up can start single-application
  (`ap_count = 1`) and add DA concurrency later.
