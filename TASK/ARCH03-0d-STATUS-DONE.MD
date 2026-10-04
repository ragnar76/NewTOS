# ARCH03 Stufe 0d — Quittung

**Status: IMPLEMENTIERT · in QEMU auf beiden Boards belegt · Abnahme durch den
Tester offen · eine Entscheidung offen (E8b, siehe unten)** · 2026-10-05 ·
Zweig `arch03`, HEAD `abc5c6a`, Baum vorher sauber, nichts committet · Belege
[measurements/measurement-2026-10-05-arch03-0d/](../measurements/measurement-2026-10-05-arch03-0d/)

Jedes NewTOS-Programm ist jetzt ein positionsunabhängiges ELF (`ET_DYN`). Der
Kernel-Lader wendet beim Laden die `R_AARCH64_RELATIVE`-Relokationen an (Plan
E8) und lehnt ein `ET_EXEC` mit `EPLFMT` (−66) ab (E7). In Stufe 0 läuft jedes
Programm weiter an derselben Adresse wie bisher (TPA bei VA 0, Ladebasis 0);
Stufe 1 muss dem Lader nur eine andere Ladebasis geben.

**Abweichung vom Plan (auf Auftrag):** Der Plan nimmt `ET_EXEC` in 0d noch an
und schneidet erst nach Stufe 1. Der Auftrag verlangt den Schnitt schon jetzt
(„reject ET_EXEC per E7, after all in-tree programs are rebuilt“) — umgesetzt.
Folge: alter Kernel + neue Programme und neuer Kernel + alte Programme laufen
**beide nicht**; Kernel und Programme müssen zusammen auf die Karte.

## Quellen

- AAELF64 2025Q4 (gepinnt): `R_AARCH64_RELATIVE` = 1027, `Delta(S)+A`, Platz =
  8-Byte-ausgerichtetes 64-Bit-Wort (§5.7.15, S. 40/41); `e_flags` „shall contain
  zero“ (§5.2, S. 13).
- **Neu gepinnt** (`references/gabi/`, `references/README.md` mit sha256):
  System-V-gABI, ELF-Kapitel (Entwurf 2013-06-10, die `[SCO-ELF]`-Quelle aus
  AAELF64 §2.2) — `Elf64_Rela`, `ELF64_R_SYM/TYPE`, `Elf64_Dyn`,
  `DT_RELA/RELASZ/RELAENT` = 7/8/9 samt „must also have“, `DT_TEXTREL`,
  `DF_TEXTREL`, `PT_DYNAMIC`, `PT_INTERP`; und Arm SYSVABI64 2025Q4 (§9, S. 23:
  PIEs sind `ET_DYN`). `DT_RELACOUNT` steht in keiner gepinnten Quelle — der
  Lader benutzt es nicht, er läuft über `DT_RELASZ`.

## Toolchain

| | vorher | jetzt |
|---|---|---|
| Übersetzen | `-fno-pic` | `-fPIE` |
| Binden | `-no-pie` | `-pie --no-dynamic-linker -z text` (+ unverändert `-z max-page-size=0x1000 --build-id=none`) |
| `sdk/newtos.ld` | `.got` verworfen, kein `PT_DYNAMIC` | Basis bleibt `0x1000`; `.dynsym/.gnu.hash/.hash/.dynstr/.rela.dyn` ins `text`-Segment, `.dynamic/.got` ins `data`-Segment vor `.data`, eigener `PT_DYNAMIC`-Kopf |
| `crt0` (`sdk/crt0.S`, `sdk/libc/crt0.S`) | Basepage aus VA 0 | Basepage aus `x0` |

Werkzeuge: Apple clang 21.0.0, rust-lld 22.1.2. **Gemessene Relokationstypen**
(`reloc-census.txt`, alle 40 neu gebauten Programme): **nur
`R_AARCH64_RELATIVE`** (0 bis 1892 je Programm). Kein `ABS64`, `GLOB_DAT`,
`JUMP_SLOT`, `IRELATIVE`, TLS; dynamische Marken nur `RELA/RELASZ/RELAENT/RELACOUNT`.

Zwei Spike-Befunde, im Linkskript behoben: (1) Mit dem alten
`/DISCARD/ *(.got)` band lld eine `-fPIE`-GOT-Ladung für ein schwaches,
undefiniertes Symbol auf Adresse 0 — `.got` bleibt jetzt erhalten. (2) Als
Waise landete `.dynamic` hinter `.bss` und machte das BSS zu Dateinullen —
jetzt ausdrücklich platziert. Alle sechs Basepage-Segmentfelder bleiben
konsistent (APITEST T32 grün).

## Lader

| Datei | was |
|---|---|
| `tos-api/src/bdos/process/elf.rs` | nur `ET_DYN` (`ET_EXEC` → `NotPositionIndependent`), `e_flags == 0` (`BadFlags`), `PT_INTERP` → `HasInterpreter`, höchstens ein `PT_DYNAMIC` (wird als `DynamicSegment` festgehalten) |
| `tos-api/src/bdos/process/reloc.rs` (neu) | `find_rela`: `PT_DYNAMIC` muss in Dateibytes eines `PT_LOAD` liegen, 8-ausgerichtet, ganze `Elf64_Dyn`, mit `DT_NULL`; Ablehnung von `DT_NEEDED`, `DT_JMPREL`/`DT_PLTRELSZ≠0`, `DT_REL`/`DT_RELSZ≠0`, `DT_TEXTREL`, `DF_TEXTREL`; `DT_RELA/RELASZ/RELAENT` alle drei oder keiner, `RELAENT == 24`, `RELASZ % 24 == 0`, Tabelle in Dateibytes. `apply_rela`: nur Typ 1027, Symbol 0, Platz 8-ausgerichtet, ganz in einem beschreibbaren `PT_LOAD` (BSS eingeschlossen), nicht in Tabelle/Dynamik-Array, Wert = Ladebasis + Addend mit Überlaufprüfung |
| `tos-api/src/bdos/process/loader.rs` | Relokation direkt nach dem Streamen der Segmente, vor Systemvariablen/Basepage und vor `sync_code_cache` (I-Cache-Pflege sieht das Endbild, kein UCI); `LoadError::Reloc` → `EPLFMT` über den bestehenden Default-Arm in `pexec.rs`/`exceptions.rs`; Zusicherung `USER_VA_BASE == 0` markiert die Stelle, an der Stufe 1 „Offset im TPA“ von „Ausführungsadresse“ trennen muss |
| `kernel/src/arch/aarch64/user_entry.rs` | `enter_el0_with_resume_capture(entry, basepage_va, …)` setzt `x0` = Basepage — vorher setzte nur der `Pexec`-Weg `x0`, der Kernel-Startweg (Desktop/AUTO/#Z) nicht |
| `kernel/src/launch.rs`, neun Selbsttests + `selftest_ec4` | übergeben die Basepage |

## Neu gebaut

SDK-Bibliotheken `libc.a`, `gemlib.a`, `cflib.a`, `libm.a` (`-fPIE`) und alle
Programme: QED, TeraDesk, Doom, CoreMark (CORE/COREVAL), APITEST, TESTGEM,
CHAINTST, die libc-/gemlib-/cflib-/libm-Testclients, die fünf tosdump-`.NAP`,
alle `sdk/build.sh`-Beispiele (inkl. KBDVEC, P10/port10, CRASHTST/CRCHILD) und
das handgebaute `HELLO.NCP` (`tools/mk-hello-ncp.py`, jetzt `ET_DYN` mit
PC-relativem `adr`). Selbsttest-Blobs: neun auf `ET_DYN`; `selftest_pexec` und
`selftest_pexec_d4d5` von VA `0x100` auf `0x1000` (0b-Befund
`SegmentVaOutOfRange`), ihre absoluten `movz`-Adressen und der Handler in
`selftest_etv_term` jetzt PC-relativ (`adr`).

## Prüfung

| | Ergebnis | Basislinie |
|---|---|---|
| Hosttests | **1045 / 0 / 1** (+25 `tests/loader_pie_reloc.rs`: gültiges Bild, Basis 0 und ≠ 0, nur Plätze geändert, ET_EXEC/e_flags/PT_INTERP, fremde Typen ABS64/GLOB_DAT/IRELATIVE/…, Symbol≠0, schlechte Offsets, Tabelle außerhalb, RELASZ/RELAENT, verbotene Marken, kein DT_NULL, Überlauf) | 1020 / 0 / 1 |
| Release frisch je Board | `tos-api` **64**, `kernel` **94** | 64 / 94 |
| Clippy beide Feature-Sätze | **80 / 125**, kein Treffer in berührten Dateien | 80 / 125 |
| Driftprobe | **31** | 31 |
| `sh sdk/run-host-tests.sh` | 13 / 0 / 0 | grün |

QEMU 11.0.2 (Einzelheiten `RUNINFO.TXT`):

| Lauf | Ergebnis |
|---|---|
| raspi4b Produktkern, kein #Z | Desktop |
| raspi4b, #Z TeraDesk | TeraDesk-Desktop (646 Relokationen) |
| raspi4b, #Z QED | QED, eigene Menüleiste, Vollbild (559) |
| raspi4b, #Z Doom | `VERIFIED via EdDI+scale+vro_cpyfm`, Demo läuft (1892) |
| raspi4b, #Z APITEST | 106 PASS / 0 FAIL / 5 SKIP |
| raspi4b Scratch, #Z CRASHTST | **PASS** (6 Abstürze 0xFFFF, sauberes Kind 0), Desktop danach |
| raspi4b Scratch, #Z KBDVEC + EmuCON | **KBDVEC: PASS**; EmuCON über Ctrl-Z: `run hello2.ncp` läuft, `run oldexec.ncp` → „Cannot run program (error 66).“, `exit` → Desktop |
| raspi4b Produktkern, #Z OLDEXEC.NCP (absichtlich `ET_EXEC`) | `LoadFailed(Parse(NotPositionIndependent))`, `rc=-66`, Alert „TOS error #35.“ |
| raspi4b, Accessory SYSACC.NAC | über den Scheduler-Erststart geladen, erreicht seine Nachrichtenwarte |
| virt Produktkern | Desktop |
| virt Scratch (RAM-Registry) | CRCHILD + CRASHTST **PASS** |
| virt Scratch, 10 Selbsttests | 9 PASS; `sleepwake_switch` FAIL (5 Prüfungen) — **gleich auf HEAD vor 0d**, also vorbestehend; `pexec`/`pexec_env`/`d4d5` scheiterten vor 0d an `SegmentVaOutOfRange` und laufen jetzt |

Die Endkerne nach rein kommentarseitigen Nachbesserungen (`k-final-*`) sind
frisch gebaut und mit Doom/TeraDesk (raspi4b) und Desktop-Start (virt)
gegengeprüft.

## Dateien für die Pi-Karte (alle zusammen mit dem neuen `kernel8.img`)

Der neue Kernel lehnt jedes alte Programm ab, der alte Kernel jedes neue.
Ziel = der Ort der bisherigen Kopie auf der Karte; die Pfade unten sind die
üblichen (wie `make-sd-image.sh` sie für QEMU anlegt — die echte Karte kann
abweichen). Kein Ressourcen-/Datenfile hat sich geändert.

- Kernel: `kernel8.img` aus `cargo xtask build --board rpi4 --release` (Repo-Wurzel)
- `sdk/apps/teradesk/build/dist/DESKTOP.NAP` → `C:\Apps\TERADESK\DESKTOP.NAP`
- `sdk/apps/qed/build/QED.NAP` → `C:\Apps\QED\QED.NAP`
- `sdk/apps/doom/build/DOOM.NAP` → `…\DOOM.NAP` (neben `DOOM1.WAD`)
- `sdk/apps/coremark/build/CORE.NCP`, `COREVAL.NCP` → `C:\COREMARK\`
- `sdk/apps/apitest/APITEST.NCP`; `sdk/apps/gemtest/TESTGEM.NAP`; `sdk/apps/chaintest/CHAINTST.NAP`
- `sdk/libc/build/libtest.NCP` → `LIBTEST.NCP`; `argvtest.NAP` → `ARGVTEST.NAP`;
  `clocktest.NCP` → `CLOCKTST.NCP`; `setjmptst.NCP` → `SETJMPT.NCP`;
  `P10TEST.NCP`; `P10KID.NCP` (auf der Karte auch als `C:\P10\P10KID.NTP` und `ONLYNCP.NCP`)
- `sdk/gemlib/build/gemtest.NCP`; `sdk/cflib/build/cftest.NCP`; `sdk/libm/build/mathtst.NCP` → `MATHTST.NCP`
- `tosdump/build/DUMPVDI.NAP`, `DUMPAES.NAP`, `DUMPRAST.NAP`, `SCRNINFO.NAP`, `VDIDRAW.NAP`
  (`tosdump/deploy-pi4/` enthält noch die alten — nicht von dort kopieren)
- aus `measurements/measurement-2026-10-05-arch03-0d/bin/`: `HELLO.NCP`, `HELLO2.NCP`,
  `BIG.NCP`, `PHYSBASE.NCP`, `SCRFLIP.NCP`, `CURDIR.NCP` (→ `C:\SUB\`),
  `CMDLINE.NCP`, `CMDLINE.NTP`, `DTACHILD.NCP`, `SYSINFO.NCP`,
  `SYSACC.NAC` (→ `C:\ACC\SYSACC.NAC` und als `C:\SYSACC2.NAC`), `KBDVEC.NAP`,
  `CRASHTST.NAP`, `CRCHILD.NAP`, `A107PAR.NAP`, `A96ZOMB.NAP`, `A79COLOR.NAP`, `WFSCREEN.NAP`
- **nicht** kopieren: `OLDEXEC.NCP` (absichtliches `ET_EXEC`, nur Ablehnungstest)
- **außerhalb des Repos:** `BALL.NAP` (`/Volumes/Daten/Development/ball-demo`) — dessen
  `build-ball.sh` bindet noch `-fno-pic`/`-no-pie` und wird abgelehnt, bis dort
  dieselben zwei Flags umgestellt sind (nicht angefasst)

## E8b — nicht umgesetzt, was es ändern würde

Der Flag-Mechanismus ist **unverändert**: der Lader schreibt
`p_flags = PF_STANDARD` (`loader.rs` `populate_basepage`), das Linkskript
verwirft weiter `*(.note .note.*)`. E8b (PRGFLAGS als ELF-Notiz) würde ändern:
1. SDK: eine Notiz `.note.newtos.prgflags` (Name `"NewTOS"`, Typ 1, `u32` Flags,
   Standard `PF_STANDARD`) — z. B. aus `crt0.S` oder einem eigenen Objekt, je
   Programm überschreibbar; `newtos.ld` hält `.note.newtos*` statt sie zu
   verwerfen und bekommt einen `PT_NOTE`-Kopf.
2. `elf.rs`: `PT_NOTE` lesen und die Notiz prüfen (Namen, Typ, Länge, Ausrichtung).
3. `loader.rs`: `p_flags` aus der Notiz statt `PF_STANDARD`; fehlt sie → `PF_STANDARD`.
4. Später (Stufe 2/3): `Pexec(7)`, TPA-Wahl ST/TT und der Schutz-Standard des
   `memspace` lesen dieselben Flags; ein Werkzeug zum Ändern (wie `prgflags.ttp`).
Das Notizformat ist unsere Festlegung, keine Extraktion — daher Anwenderfreigabe.

## Offen / Hinweise für den Lead

- **`DT_RELR`** (gepackte relative Relokationen, lld `-z pack-relative-relocs`)
  steht in keiner gepinnten Quelle; der Lader kennt die Marke nicht und würde sie
  wie jede unbekannte Marke übergehen — ein so gebautes Programm liefe mit
  unrelozierten Zeigern. Wir erzeugen sie nicht. Frage: unbekannte Marken
  künftig ablehnen (Positivliste) oder `DT_RELR` nach Pinnen einer neueren gABI
  ausdrücklich ablehnen?
- Für Stufe 1 festgehalten (in 0d bewusst nicht geändert, Programme laufen bei
  Basis 0 richtig): der Scheduler-Erststart setzt `x0 = USER_VA_BASE` statt der
  Basepage des Slots (`exceptions.rs` `apply_switch`); `sdk/examples/cmdline.c`,
  `crchild.c` und APITEST (`T08`, `RD_U32(0x04)`) lesen die Basepage an VA 0;
  Kernel-seitige Handler-VAs der Selbsttests (`selftest_upcall` `HANDLER_VA`,
  `selftest_sleepwake_switch` `A_HANDLER_VA`) sind absolut; `loader.rs`
  vermengt Link- und Laufadresse (Zusicherung `USER_VA_BASE == 0`).
- Echte Pi nicht gefahren (Auftrag). Die I-Cache-Pflege nach der Relokation ist
  das bestehende `sync_code_cache` über `[TPA, image_top)` — auf QEMU unsichtbar.

## Beobachtungen außerhalb des Auftrags (unverändert)

- `selftest_sleepwake_switch` scheitert vor und nach 0d mit denselben 5 Prüfungen (unverdrahteter Selbsttest).
- `user_entry.rs` `boot_l0_phys` ist ungenutzt (vorbestehende Clippy-Meldung).
- `docs/ARCHITECTURE.md` und ältere Analyse-/TASK-Dokumente nennen noch `-no-pie`/`ET_EXEC` (historisch, nicht angefasst); das SDK-Handbuch `docs/sdk/` und `sdk/README.md` sind nachgezogen.
- `sdk/apps/gemtest/build-gemtest.sh` ist per `.gitignore` nicht versioniert; lokal ebenfalls umgestellt.
- APITEST T23/T24/T26 melden `rc=-33`, wenn `C:\qed.rsc` nicht im Wurzelverzeichnis liegt (Karteninhalt, nicht in der Zusammenfassung gezählt).
