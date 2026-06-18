# AES Scrap Manager (`scrp_*` 80–82) + Shell Manager (`shel_*` 120–127)

> **PHASE-00b D8, Batch 7 (final).** Source: EmuTOS @ `1a7484ab` —
> `aes/gemsclib.c` (`sc_*`), `aes/gemshlib.c` (`sh_*`), dispatch
> `aes/gemsuper.c`.
>
> Call convention: [`_dispatcher.md`](./_dispatcher.md).

## Scrap Manager (`scrp_*`) — the clipboard

GEM's clipboard is **file-based**: a directory (e.g. `C:\CLIPBRD\`) where
applications write `SCRAP.*` files (`SCRAP.TXT`, `SCRAP.IMG`, …). The
Scrap Manager only manages the **path**; the actual data is exchanged as
files via GEMDOS.

| Op | Name | Dispatch | Effect |
|---:|------|----------|--------|
| 80 | `scrp_read`  | `sc_read(path)` | copy the current clipboard directory path into the user buffer `path` (addrin[0]); returns 1, or 0 if unset |
| 81 | `scrp_write` | `sc_write(path)` | set the clipboard directory path from `path`; creates it if needed. Returns 1 |
| 82 | `scrp_clear` | `sc_clear()` | delete the `SCRAP.*` files in the clipboard dir (PC-GEM ext., `CONF_WITH_PCGEM`) |

`SC_PATH` = addrin[0]. To copy: an app `scrp_read`s the path, writes
`SCRAP.TXT` there; to paste: `scrp_read` + read the file back.

## Shell Manager (`shel_*`) — process launch / desktop

The interface between the **desktop / shell** and launched applications:
how the next program is chosen, its command line passed, and shared
state (the `shel_get`/`shel_put` buffer) carried across launches.

| Op | Name | Dispatch | Effect |
|---:|------|----------|--------|
| 120 | `shel_read`  | `sh_read(cmd, tail)` | the launching command + command tail that started **this** app → user buffers `cmd`/`tail` |
| 121 | `shel_write` | `sh_write(doex, isgr, iscr, cmd, tail)` | tell the shell **what to run next**. `doex`: 1 = launch `cmd`, 0 = exit to desktop; `isgr`: 1 = GEM/graphic app, 0 = TOS; `iscr`: launch mode. `cmd`/`tail` = next program + tail |
| 122 | `shel_get`   | `sh_get(buffer, len)` | read `len` bytes of the **shell buffer** (the desktop's `EMUDESK.INF` / environment-ish blob) |
| 123 | `shel_put`   | `sh_put(data, len)` | write `len` bytes into the shell buffer |
| 124 | `shel_find`  | `sh_find(path)` | resolve a bare program name to a full path (searches like the shell would); rewrites `path` in place, returns 1/0 |
| 125 | `shel_envrn` | `sh_envrn(&value, search)` | find env-var `search` (e.g. `"PATH="`) in the environment; sets `*value` to the value |
| 126 | `shel_rdef`  | `sh_rdef(cmd, dir)` | read the shell's default cmd + dir (PC-GEM ext.) |
| 127 | `shel_wdef`  | `sh_wdef(cmd, dir)` | set the shell's default cmd + dir (PC-GEM ext.) |

`intin`: `SH_DOEX`/`SH_ISGR`/`SH_ISCR`=intin[0..2], `SH_LEN`=intin[0];
`SH_PCMD`/`SH_PTAIL`/`SH_PBUFFER`/`SH_PDATA`/`SH_PATH`/`SH_SRCH`=addrin[].

`sh_write(doex=0)` is how an application **quits to the desktop**: the AES
records "run the desktop next", and when the app calls `appl_exit`/`Pterm`,
the shell loop (`gemshlib.c`'s `sh_main`/`aes_run_rom_program`) launches
whatever `shel_write` selected. This is the AES↔shell launch loop that
drives the whole desktop.

There is a `default: unsupported = TRUE` arm in this dispatch version —
out-of-range opcodes are flagged (unlike older TOS, which fell through);
see [`_dispatcher.md`](./_dispatcher.md) §2.

---

## NewTOS implementation notes

- **Shell Manager is the bridge to our process model** (Phase 06 +
  GEMDOS `Pexec`/`Pterm`, already implemented): `shel_write` selects the
  next image, the shell loop `Pexec`s it. This is exactly the
  EmuCON/shell launch logic already prototyped in
  [kernel/src/shell.rs](../../kernel/src/shell.rs) (`run` command) — the
  AES `shel_*` calls formalise it with the desktop in the loop.
- `shel_find` reuses GEMDOS path search; `shel_read` reuses the `Pexec`
  command-tail (basepage `p_cmdlin`) already populated by our loader.
- The **desktop itself** (`DESK.PRG`/`EMUDESK`) is the canonical
  `shel_write` client: it loops `shel_write(doex=1, …)` to launch apps and
  re-launches itself on their exit (`doex=0`). Implementing `shel_*` is
  the precondition for running a real desktop.
- Scrap Manager is trivial given the file system (Phase 09): it's just a
  stored path string + GEMDOS file ops. Low effort, do it early for
  cut/paste demos.
- `shel_get`/`shel_put` shared buffer ≈ the desktop's persisted settings
  (`EMUDESK.INF`); back it with a kernel-held blob first, persist to disk
  later.

---

## AES extraction complete

All 11 managers (~80 functions) are now specified across Batches 0–7:
[`_dispatcher.md`](./_dispatcher.md), [`application.md`](./application.md),
[`event.md`](./event.md), [`object-form.md`](./object-form.md),
[`window-graphics.md`](./window-graphics.md), [`menu.md`](./menu.md),
[`resource-fileselector.md`](./resource-fileselector.md), and this file.
Implementation follows in Phase 13 (D2–D9), gated on VDI (Phase 12).
