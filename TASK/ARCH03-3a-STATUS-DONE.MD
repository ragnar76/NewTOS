# ARCH03 Stufe 3a — Quittung

**Status: IMPLEMENTIERT (Teilschritte 1–7) · in QEMU auf beiden Boards belegt ·
Abnahme durch den Tester offen · Pi offen** · 2026-10-05 · Zweig `arch03`, Basis
HEAD `54f0d32`, Baum vorher sauber, nichts committet · Belege
[measurements/measurement-2026-10-05-arch03-3a/](../measurements/measurement-2026-10-05-arch03-3a/)
(`RUNINFO.TXT`, `checks/`, `serial/`, `screendumps/`, `method/`, `bin/`).

Jeder Kernelzugriff auf Programmspeicher aus der Inventur
(`docs/analysis/arch03-stage3-access-inventory.md`) läuft jetzt entweder mit
den Rechten des Aufrufers über `uaccess` (fehlerabgefangen) oder wird mit
dessen Rechten geprüft und dann über den Kernel-Alias erreicht — unabhängig
davon, welches `TTBR0` gerade gilt. Rechte je Prozess sind **nicht**
eingeschaltet (Stufe 3b). Für korrekte Programme ändert sich nichts
(Screendumps bytegleich mit Stufe 2); ein schlechter Zeiger beendet das
aufrufende Programm mit dem EmuTOS-Absturzbericht statt den Kernel anzuhalten.

## Teilschritte

| | Stand | Prüfpunkt |
|---|---|---|
| 1 Aufrufer-EL in der Trap-Dispatch | fertig | s12 |
| 2 GEMDOS-Pfade/Puffer/DTA/Konsole (F1) | fertig | s12 |
| 3 BIOS/XBIOS, vier ldtr/sttr-Stellen (F5) | fertig | s37 |
| 4 AES/VDI-Parameterblöcke, CONTRL, MFDB | fertig | s37 |
| 5 objc_edit/menu_text, AESGLOBAL-Schreiber (F2) | fertig | s37 |
| 6 Menü-Tracking über `mntree_accessor` (F3) | fertig, ohne Zwei-Client-Lauf | s37 |
| 7 `vex_*` (F4) | Entscheidung: dokumentierter No-op, offen für den Lead | s37 |
| Mxalloc-/PRGFLAGS-Schutznibble | Kommentar korrigiert, Dekoder + Hosttests, nicht verdrahtet | s37 |

**Offen gelegt:** 1+2 und 3–7 wurden je gemeinsam geprüft, nicht nach jedem
Teilschritt einzeln — der Baum enthielt die Änderungen des nächsten
Teilschritts schon, bevor dessen Prüfpunkt genommen war, und eine saubere
Trennung hätte Git-Operationen am geteilten Baum gebraucht.

## Was getan wurde

| Bereich | Datei / Funktion |
|---|---|
| Grundlage | `tos-api/src/uaccess.rs`: `Space { User, Kernel }` (`from_spsr`, `get/put_u8..u64`, `copy_from/to`, `read_cstr`, `kptr` = Prüfung mit Aufruferrechten seitenweise + Kernel-Alias, Seite 0 über den eigenen Seite-0-Alias, Schreibprobe `ldtrb`+`sttrb` mit maskierten IRQs, `kptr_or_null`), `FAULTED`; Kopierschleifen mit 8-Byte-`ldtr/sttr` (2 neue Fixup-Einträge) |
| 1 | `tos-api/src/trap.rs` `TrapArgs::space` + `TrapArgs::kernel`; `exceptions.rs` setzt `Space::from_spsr(frame.spsr)`; alle kernel-eigenen `TrapArgs` = `Kernel` |
| 2 | `bdos/fs/mod.rs` `path_arg` statt `cstr` (Fopen, Fcreate, Fdelete, Fattrib, Frename×2, Dcreate, Ddelete, Dsetpath, Fsfirst); `file.rs` Fread/Fwrite/Fdatime/`console_read`/`console_write`; `find.rs` `cur_dta_in` + `put_state`/`makbuf`/`get_state` (DTA aus `p_xdta` = User, `KERNEL_DTA` = Kernel); `dirops.rs` Dgetpath; `drive.rs` Dfree; `console.rs` Cconws; `gemdos/trace.rs` (Feature) |
| 3 | `console.rs` Cconrs (`LineEditIo::faulted`, `LineEdit::Faulted`); `ssystem.rs`; `bios/vectors.rs` Setexc (Seite-0-Alias, ohne Prozess `uaccess`); `bios/chardev.rs` conterm über `current_conterm`; `bios/mod.rs` Rwabs (`kptr`); `getmpb.rs`; `xbios/screen.rs` Setpalette; `floppy.rs` Protobt; `keytbl.rs` |
| 4 | `aes/mod.rs` `Aespb` (Block + `control` + `intin`/`addrin` im Aufrufer-Space, `copy_intout_back`, `set_addrout`, `global_kptr`, `ubuf`), `dispatch_in`, `thread.rs` (Space im `abi_raw`-Wort); `addr_in`-Ziele: `appl_find/write/read`, `evnt_mesag/multi`, `form_alert`, Dateiauswahl, `graf_mouse`, `menu_register/popup/attach/settings`, `rsrc_load`, `scrp_*`, `shel_*`, Tracer; `vdi/mod.rs` `Vdipb` (Block + `CONTRL[0..7]` geprüft, Elementzugriffe über `uaccess`, nach Fehler inert), `dispatch_in`; `vdi/raster.rs` `Mfdb::read`/`bind`/`bind_rows` (vrt/vro_cpyfm, vr_trnfm, `fd_stand`) |
| 5 | `object.rs` `ObjTree::wbuf` für `te_ptext`; `menu.rs` `menu_text`; AESGLOBAL in `appl_init`, `rsrc_load`/`rsrc_free`/`resolve` über `global_kptr` |
| 6 | `menu.rs` Tracker, `rehilite`, `open/close_box`, Submenü über `mntree_accessor`/`ObjTree::with_base`; `object.rs` `child_at_t`, `draw_obj_clip_t` |
| 7 | `vdi/mouse.rs` Kommentar, `docs/api/vdi/input/_mouse.md` |
| Nibble | `tos-types/src/memory.rs` Kommentar, `prot_from_prgflags`, `prot_from_mxalloc`, `ProtMode::from_prot`; `docs/MEMORY.md` ADR-023 Korrekturvermerk |
| Doku | `docs/api/bios/_exception-crash.md` (3a-Abdeckung), `desk/dosbind.rs`-Modulkommentar |
| Tests | `tos-api/tests/arch03_3a_caller_space.rs` (9), `arch03_3a_gem_space.rs` (4), uaccess (4), console (2), tos-types (4) |
| Sonde | `sdk/examples/uakid.c` (26 Fälle), `sdk/examples/uacc3a.c` (Treiber) |

**Gezählt (gegen die Inventur, ~44 umzustellende Stellen):** GEMDOS 21
(10 Pfadleser über `path_arg`, Fread, Fwrite, Fdatime, Konsole lesen/schreiben,
DTA, Dgetpath, Dfree, Cconws, Cconrs, Ssystem ×2 — Tracer zusätzlich), BIOS 4,
XBIOS 3, AES-Block 4 (`from_raw`, `copy_intout_back`, `set_addrout`,
AESGLOBAL), `addr_in`-Ziele 17 Aufrufe, F2 2, F3 1 Funktionsfamilie, VDI 3
(Block/CONTRL, Elementzugriffe, MFDB) — rund **55 Stellen**; mehr als die
Inventur, weil `menu_popup/attach/settings` und `shel_get/put/rdef/wdef/envrn`
ebenfalls Aufruferspeicher lesen oder schreiben. Unverändert (laut Inventur
schon tauglich): `ObjTree`-Bäume, `LiveStr`-Titel, WF_NEWDESK, Setscreen,
Getbpb/WF_SCREEN, Loader/Env/Seite 0, Upcalls, `rsrc_saddr` (schreibt in die
Kernel-Ansicht des Blobs).

## Verhalten bei schlechtem Zeiger

UACC3A (BAD = `0xDEAD0000`): alle 26 Fälle — GEMDOS c o n w g r W d f t k s,
BIOS m b, XBIOS P T K, AES A I G F E X, VDI V C M — enden mit
`Panic: Bus Error`, `addr=00000000dead0000`, PC im `uaccess`-Text, Taste,
`Pterm(-1)`, Eltern-`Pexec` = `0x0000FFFF`, der EL0-Elternprozess läuft
weiter, danach der Desktop. Kein dokumentierter Fehlercode statt Absturz:
EmuTOS kennt für diese Fälle keinen (Spec `_exception-crash.md` §5).
Zwei bewusste Ausnahmen ohne Absturz: Zeiger, die die AES **außerhalb** des
Eigentümer-Traps folgt (fremder Baum/Titel), lesen als nichts; Seite-0-Zellen
über den Seite-0-Alias (Setexc, conterm) können nicht fehlschlagen.

**Entscheidung, offen gelegt:** Für Parameterblöcke und `addr_in`-Puffer ist
die „Regionsprüfung“ aus Plan §3.6 als **Prüfung mit den Rechten des
Aufrufers** (unprivilegierte Probe über sein `TTBR0`) gebaut, nicht als Suche
in der Regionstabelle: Es sind Syscall-Argumente im eigenen Trap des
Aufrufers, die Probe ist mit Stufe 3 genau die richtige Antwort (sie verweigert
auch Schreiben auf `PROT_PR`-Seiten) und liefert einen echten Fehlerdatensatz
für den Absturzbericht. Zugriffe außerhalb des Eigentümer-Traps bleiben bei
`resolve_kva`.

## Prüfungen (Zahlen)

| | s12 | s37 | Basis |
|---|---|---|---|
| Hosttests | 1094/0/1 | **1104/0/1** | 1081/0/1 |
| Release frisch, tos-api/kernel, beide Boards | 64/94 | **64/94** | 64/94 |
| Clippy tos-api/kernel, beide | 80/125 | **76/125** (4 weniger, keine neuen) | 80/125 |
| Driftprobe | 31 | **31** | 31 |
| SDK-Hosttests | 13/0/0 | **13/0/0** | 13/0/0 |

QEMU je Prüfpunkt (Einzelheiten `RUNINFO.TXT`, `checks/s12-summary.txt`,
`checks/s37-summary.txt`): Desktop, TeraDesk, QED, APITEST-Bild **bytegleich**
mit Stufe 2; APITEST 106/0/5; Doom VERIFIED; S2ALL PASS; PORT10 42/44 (PROC02
wie vorher); CRASHTST PASS (raspi4b, virt); KBDVEC PASS + EmuCON bytegleich;
virt-Selbsttests wie Stufe 2 (sleepwake_switch dieselben 5 FAIL); virt-Desktop
bytegleich; UACC3A PASS (s12; s37b nach Sondenkorrektur Fall s). Desktop-Lauf
(`desk3a`, in-kernel Desktop = F1-Risiko): Ordner öffnen, zurück, Datei
markieren, *Show info*, Name editieren, Umbenennen, Laufwerk C: öffnen —
alles korrekt (`screendumps/s37-desk3a-*`).

**Sondenfehler unterwegs (kein Produktfehler):** Fall `s` fragte das Cookie
`_CPU` ab, das NewTOS nicht hat — `Ssystem` antwortet dann `ERROR` ohne zu
schreiben (spezifikationsgemäß). Korrigiert auf Tag 0 (Kapazität), Lauf s37b.
**Methodenfehler unterwegs:** die erste s12-Serie bootete alle Karten ohne
`#Z` (FAT16-Wurzel durch AppleDouble-Zwillinge kaputt, `seed-card.sh` brach
vor `NEWDESK.INF` ab); verworfen, Ursache behoben, neu gefahren.

## Für den Tester

- Karten: `method/seed-card.sh` (nie `tools/make-sd-image.sh`), Serie
  `method/runs-3a.sh <label> <cards>`, Sonde `C:\UACC3A.NAP` + `C:\UAKID.NAP`
  (Kernel mit Antwort-Injektor: `bin/kernel8-crash-s37.img`, virt:
  `bin/kernel-virt-crash-s37b.img`), Desktop-Lauf `method/desk3a.sh`.
- Erwartung UACC3A: „UACC3A: PASS“, jede Zeile `0x000000000000FFFF OK`, auf
  virt die 6 Plattenfälle „skipped“.
- Echte Pi: alle Programme unverändert außer den zwei neuen Sonden; nur
  `kernel8.img` + UACC3A/UAKID nötig. Pi-Lauf nur auf Ansage.

## Offen / zur Entscheidung

1. **F4 `vex_butv/motv/curv/timv`:** EmuTOS ruft alle vier aus `mouse_int`/
   `tick_int` (`vdi/vdi_asm.S:283,359,367`, `vdi/vdi_misc.c:83`); `user_mot`
   gibt die Koordinaten zurück. NewTOS speichert nur — die
   `vex_wheelv`-Upcall-Form (feuern und vergessen) trägt keinen Rückgabewert.
   Als dokumentierter No-op belassen; für Stufe 3 folgenlos (nichts wird über
   die Adressen ausgeführt). Ein Upcall mit Rückkanal wäre ein eigenes Design.
2. **F3** ist umgebaut, aber nicht mit zwei AES-Clients gefahren (Bar eines
   nicht laufenden Prozesses).
3. **Mxalloc-Nibble:** Dekoder vorhanden und getestet, `Mxalloc` maskiert
   weiter (`MX_MODEMASK`); das Verdrahten (Modus in der Region speichern,
   Plan Stufe 2/3) ist nicht gemacht.
4. Prüfpunkte gebündelt (s12, s37) statt je Teilschritt — siehe oben.

## Beobachtungen außerhalb des Auftrags (unverändert)

- `aes/shell.rs` `shel_envrn` schreibt eine **Kernel**-Adresse (`S.env`) in
  den Zeiger-Slot des Programms — für ein EL0-Programm nicht lesbar.
- `vdi/raster.rs` `vr_trnfm`: `(src.h as usize) * (src.wdwidth as usize)`
  mit negativem `fd_h` läuft im Dev-Profil in einen Überlauf-Panic.
- `find.rs`: ein EL1-Aufrufer, der bei laufendem Prozess `Fsetdta` mit einer
  Kernel-Adresse ruft, würde sie auf 32 Bit gekürzt in `p_xdta` legen.
- macOS-Sandbox versieht neue Dateien mit `com.apple.provenance`; auf
  FAT-Karten entstehen dadurch `._*`-Zwillinge (Methodenthema).
