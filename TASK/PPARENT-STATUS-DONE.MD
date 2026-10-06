# PPARENT — `p_parent` und Umgebung wie EmuTOS (Weg A, Stellvertreter-PDs) — ERLEDIGT

Anwenderentscheidung 2026-10-06 „Weg A“ (`TASK/ARCH03-POSTE9-STATUS-UNFINISHED.MD` §1;
Weg B = `TASK/PROC03.md`). Nichts committet. Beleg:
`measurements/measurement-2026-10-06-pparent/RUNINFO.TXT`.

## Spezifikation (vor dem Code)

- Neu: `docs/api/gemdos/process/_parent-and-environment.md` — EmuTOS-Extraktion
  (`proc_go` `bdos/proc.c:541`, `initial_basepage` `bdosmain.c:78,337-347`,
  AES-PD `bios/bios.c:862-865`, `default_env` `:626-636`, `alloc_env`
  `proc.c:413-432`, `ad_envrn` `gemstart.S:78-96`, Accessory `util/gemdos.c:29-47`)
  und die NewTOS-Abbildung.
- Angepasst: `4b-pexec.md`, `structures/basepage.md`, `gemdos/_state.md`,
  `aes/scrap-shell.md` (`shel_envrn`), `aes/accessories.md`,
  `aes/resource-fileselector.md` (Stufe 4 von `sh_find`), `COMPATIBILITY.md`,
  `docs/sdk/01,05,07`.

## Umsetzung

- `tos-api/src/bdos/process/standin.rs` (neu): eine Kernelseite mit
  `initial_basepage` (+0x000), AES-PD (+0x100), `double_nul`, AES-Umgebung
  `"PATH=\0X:\\\0\0"` (12 B); Feldinhalte nach EmuTOS; beim Booten read-only für
  alle veröffentlicht (`usermem::expose`, `PROT_PR`). `kernel_run()` =
  initial bis `enter_aes()`, danach AES-PD. `RUN` bleibt `null`.
- `pexec.rs`: `set_parent` schreibt bei Kernel-Aufrufer den Stellvertreter
  (Rechte unverändert nach echtem Aufrufer), neu `set_kernel_parent`.
- `loader.rs`: `EnvSpec::Inherit` ohne laufenden Prozess = Umgebung des
  Stellvertreters (`inherited_env`); `drive.rs`: `boot_drive()`.
- `aes/shell.rs`: `shel_envrn` durchsucht die AES-PD-Umgebung; der erfundene
  Default `"PATH=C:\;"` und `AES_ENV` sind weg.
- Kernel: `lib.rs` (init vor AUTO, `enter_aes` danach), `launch.rs`
  (`Inherit` + `set_kernel_parent`), `accs.rs` (`Inherit`, `p_parent` bleibt 0),
  Kommentar `exceptions.rs`.
- SDK: `sdk/libc/newtos-arch/heap_init.c` und `sdk/crt0.S`: mintlibs Test
  `p_parent == 0` (crt0.S:52-56), EGSBF-Heuristik entfernt, `Mshrink`-Ergebnis
  ignoriert wie mintlib; Kommentare `sdk/libc/crt0.S`, `cmdargs.c`.

## Prüfungen

| | Basis | jetzt |
|---|---|---|
| Hosttests | 1138/0/1 | **1145/0/1** (+7) |
| Release beide Boards | 64/94 | 64/94 |
| Clippy rpi4/virt | 76/125 | 76/125 |
| Drift | 31 | 31 |
| SDK-Hosttests | 13/0/0 | 13/0/0 |

QEMU raspi4b ON/OFF/STRICT: Sonde AUTO, Accessory (libc + SYSACC), `#Z`,
Pexec-Kind, Desktop-Doppelklick, ENVRN PASS; vollständige Post-E9-Reihe
zeilengleich mit vforkt-fix (S2ALL/S2ALLS, VFORKT, ACCFREE, ATEXIT, APITEST
106/0/5, TeraDesk/QED-Bilder bytegleich, …). Rot-Lauf auf altem Kernel: alle
Sonden FAIL.

## Offen / Hinweise

- **Neue SDK-Programme brauchen den neuen Kernel** (auf altem Kernel
  `p_parent == 0` → als Accessory gestartet). Karte nur gemeinsam
  aktualisieren (nur auf Ansage).
- Abweichungen durch Fehlen: AES-PD `p_hitpa = p+0x100`, `p_tbase = 0`;
  Stellvertreter read-only; `Pexec` aus dem Kernel-EmuCON bekommt den AES-PD
  statt eines EmuCON-PD; Kind eines Accessorys hat das Accessory als Eltern
  (vorbestehend) — alles PROC03.
- `sh_find` Stufe 4 (`PATH=`-Suche) weiter nicht gebaut; libc wertet `p_env`
  nicht nach `environ` aus (PORT09).
- `docs/STATUS.md` nicht angefasst (Lead).
