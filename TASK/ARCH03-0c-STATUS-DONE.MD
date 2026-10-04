# ARCH03 Stufe 0c — Quittung

**Status: IMPLEMENTIERT · in QEMU auf beiden Boards belegt · Abnahme durch den
Tester offen · eine Entscheidung offen (siehe unten, Punkt E11)** · 2026-10-05 ·
Zweig `arch03`, HEAD `b0365eb`, Baum vorher sauber, nichts committet · Belege
[measurements/measurement-2026-10-05-arch03-0c/](../measurements/measurement-2026-10-05-arch03-0c/)

Ein Programm, das eine synchrone Ausnahme nimmt (Daten-/Befehlsabbruch,
undefinierter Befehl, Ausrichtung, gesperrter Systembefehl), haelt die Maschine
nicht mehr an. Wie bei EmuTOS (`bios/vectors.S` `any_vec` → `bios/kprint.c:302`
`dopanic` → `bios/panicasm.S:96-99` `kill_program`) erscheint der Absturzbericht
auf dem Bildschirm und im seriellen Protokoll, nach einem Tastendruck wird das
Programm mit `Pterm(-1)` beendet (der `Pexec` des Elternprozesses liefert
`0x0000FFFF`), Elternprozess bzw. Desktop laufen weiter. EL1-Ausnahmen ausserhalb
des neuen `uaccess` bleiben fatal wie bisher.

## Spezifikation

`docs/api/bios/_exception-crash.md` (neu, aus EmuTOS @ `1a7484ab`, gegen
FreeMiNT @ `f906416` gegengelesen): Ablauf von `any_vec`/`dopanic`/`kill_program`,
Berichtsformat, Zuordnung `ESR_EL1.EC` → EmuTOS-Meldung, Anpassungen
(X0–X30 statt D0–A7, `esr=` statt 68030-Rahmenwoerter), offene Punkte.
**Bitte freigeben** — die Spezifikation ist in dieser Stufe entstanden, nicht davor.

**Schlechter Zeiger im Systemaufruf (abgeleitet, kein EFAULT):** TOS kennt kein
`EFAULT`. EmuTOS prueft keine Zeiger; ein GEMDOS-Aufruf auf einen unzugaenglichen
Zeiger nimmt einen Busfehler im Supervisor-Modus, und `dopanic` beendet mit
`run` = Aufrufer **das aufrufende Programm** (`kprint.c:587-594`). FreeMiNT
ebenso im Ergebnis (`SIGBUS` auf `curproc`, `sys/arch/sig_mach.c:493-502`;
sein `EFAULT` = −40 = TOS-`EIMBA` kommt nur aus expliziten Pruefungen in
`sys/dosmem.c`). Umgesetzt ist daher: die `uaccess`-Primitive liefern `Err`,
der Aufruf bricht ab, und der Systemaufruf-Schwanz beendet den EL0-Aufrufer mit
dem Bericht des Kernel-Fehlers. **Der Aufrufer bekommt keinen Fehlercode** —
das weicht von der Auftrags-Erwartung „returns an error“ und von Plan Stufe 0c
(„`Fread` mit ungueltigem Puffer → Fehlercode“) ab und folgt EmuTOS. Die Politik
steckt an einer Stelle (`uaccess_fault_tail`); eine Umstellung auf Fehlercode
waere klein, braeuchte aber einen TOS-Code, den es nicht gibt.

## Was getan wurde

| Datei | was |
|---|---|
| `tos-api/src/uaccess.rs` (neu) | Blattroutinen mit `ldtrb/ldtrh/ldtr/sttr*` (`global_asm`), Fixup-Tabelle `(Befehls-PC, Fixup-PC)` im `.rodata`, `fixup_for(pc)`/`lookup_fixup`, Fehlerprotokoll `note_fault`/`take_pending_fault` (erster Fehler gewinnt), `get_user_u{8,16,32,64}`, `put_user_u{8,16,32,64}`, `copy_from_user`, `copy_to_user`, `read_user_cstr`. Auf dem Host gewoehnliche Zugriffe, leere Tabelle. |
| `tos-api/src/bios/crash.rs` (neu) | `classify(esr)` → `CrashKind` (Bus Error / Address Error / Illegal Instruction / Privilege Violation / `Exception class 0x..`), `far_valid`, `fault_status`, `write_report` (EmuTOS-Zeilenbild) |
| `kernel/src/arch/aarch64/crash.rs` (neu) | Bericht auf Konsole (Bildschirm + UART, CR vor LF wie `cprintf_outc`), anstehende Tasten verwerfen, Aufforderung, auf Taste warten; Bytes eines `kbdvec`-Hakens des sterbenden Prozesses gehen an den Standard-Decoder, sonst haengt die Aufforderung (Doom-Fall) |
| `kernel/src/arch/aarch64/exceptions.rs` | `rust_sync_handler`: EC 0x25 an einer Fixup-PC → Protokoll + `ELR` = Fixup; EL0-Ausnahme → `kill_crashed_program`; vor jedem SVC verwaister Fehler laut verworfen. `complete_call_tail`: anstehender `uaccess`-Fehler → `uaccess_fault_tail` (EL0: Bericht + `Pterm(-1)`; EL1-Aufrufer: Halt wie bisher). `kill_program` = `handle_pterm(frame, 0x4C)` mit −1 (also `etv_term`, Freigabe, Elternrueckkehr); verweigerte Terminierung → Halt statt Endlosschleife. `handle_pexec`: Pfad/Kommandozeile/Umgebung ueber `uaccess`, Fehler → `pexec_user_fault`. |
| `tos-api/src/gemdos/console.rs` | `Cconws` liest seinen String ueber `uaccess::get_user_u8` |
| `tos-api/src/i18n.rs` | `Sys::PressAnyKey` = `*** Press any key to continue ***` (`kprint.c:583`) |
| `sdk/examples/crchild.c`, `crashtst.c` (neu) | Testprogramme (Fall per Kommandozeile; Elterntreiber) |

## uaccess — Pilotstellen und Rest

**Umgestellt (Pilot):** `Cconws`; `Pexec` Pfad, Kommandozeile, Umgebung
(`handle_pexec`; EL1-Aufrufer lesen weiter mit gewoehnlichen Ladebefehlen).

**Nicht umgestellt** (Stufe 1; die vollstaendige Inventur ist laut Plan
Auditor-Auftrag vor Stufe 3):
- schon `ldtr/sttr`, aber ohne Fixup: `gemdos/console.rs` `read_user_byte`/`write_user_byte` (`Cconrs`; braucht Fehlerweg durch `LineEditIo`), `gemdos/ssystem.rs` `write_user_long`, `bios/vectors.rs` `read_user_long`/`write_user_long` (`Setexc` Sysvar-Bereich), `bios/chardev.rs` `read_user_byte` (`conterm`);
- privilegierte Direktzugriffe auf Anwenderzeiger: `bdos/fs/mod.rs` `cstr` (Fopen, Fcreate, Fdelete, Fattrib, Frename, Dcreate, Ddelete, Dsetpath, Fsfirst) — **braucht die Aufrufer-EL**, weil EL1-Aufrufer (Desktop, AES-Faser) Kernelzeiger uebergeben; `TrapArgs` traegt sie nicht; Fread/Fwrite-Puffer und `console_read/console_write` (`file.rs`), `Fdatime`, `Dgetpath`, DTA-Schreiber (`find.rs` `makbuf`/`put_state`), `Getmpb`, `Protobt`, `gemdos/trace.rs` `copy_cstr` (Feature `gemdos-trace`);
- AES/VDI-Parameterbloecke und Baeume → laut Plan `kalias`, nicht `uaccess`.

## 0b-Befund (nur festgehalten, nicht geaendert)

`exceptions.rs` `finish_terminate`: der Kommentar nennt eine UART-Zeile zwischen
`restore_boot_ttbr0` und `install_ttbr0` „LOAD-BEARING“ (QUIRK-001); die Zeile
fehlt seit `a3c4329`. Unveraendert gelassen. Neu belegt: Laeufe 1 und 3 fuhren
genau diesen Weg (Kind → EL0-Elternprozess, `parent_l0_pa != 0`) je Board
siebenmal auf QEMU 11.0.2 ohne die Zeile — kein Haenger. Pi unbestaetigt.

## Pruefung, die gelaufen ist

| | Ergebnis | Basislinie |
|---|---|---|
| Hosttests | **1020 / 0 / 1** (+5 `uaccess`, +5 `bios::crash`) | 1010 / 0 / 1 |
| Release-Bau frisch je Board | `tos-api` **64**, `kernel` **94** (beide) | 64 / 94 |
| Clippy beide Feature-Saetze | **80 / 125**, kein Treffer in beruehrten/neuen Dateien | 80 / 125 |
| `sh tools/desk-drift-probe.sh` | **31** | 31 |
| Fixup-Tabelle im Release-ELF | 11 Eintraege, jeder = Adresse eines `ldtr*`/`sttr*` | — |

QEMU (Einzelheiten `RUNINFO.TXT`):

| Lauf | Board | Ergebnis |
|---|---|---|
| rpi4-crashtst | raspi4b, #Z CRASHTST (EL0-Eltern) | 6 Berichte, alle Kinder `0xFFFF`, sauberes Kind danach `0`, **PASS**, Desktop, Alt-C oeffnet C:\ |
| rpi4-crchild | raspi4b, #Z CRCHILD (Eltern = Shell) | Bericht ueber dem Desktop, Taste, Desktop, Alt-C oeffnet C:\ |
| virt-run1 | virt, Kernelstart aus dem Bootpfad | 7 Berichte, **PASS**, danach Desktop |
| rpi4-crchild-product | raspi4b, **Produktkern** | Bericht + Aufforderung erreicht (keine Tastenquelle → dort beendet) |
| virt-product-boot | virt, **Produktkern** | Desktop kommt |
| rpi4-doom-product | raspi4b, **Produktkern**, Doom | `VERIFIED via EdDI+scale+vro_cpyfm`, Demo laeuft |

Je Fall: `w` Bus Error (DABT, Translation L1), `u` Illegal Instruction (EC 00),
`x` Bus Error (IABT, Permission L3 — Sprung in Physbase), `a` Address Error
(EC 22), `c`/`p` Bus Error (EC 25, Kernel-`ldtrb` in `Cconws` bzw. `Pexec`).

## Offen / Hinweise fuer den Lead

- **E11:** Plan empfiehlt den FreeMiNT-Alerttext „MEMORY VIOLATION“; der
  Auftrag verlangte den EmuTOS-Bericht. Umgesetzt ist der EmuTOS-Bericht; kein
  `form_alert`. Entscheidung offen.
- **Kein Fehlercode bei schlechtem Zeiger** (oben) — Abweichung von der
  Auftrags-/Planformulierung, EmuTOS-treu.
- Echte Pi: nicht gefahren (Auftrag). Tastenwarten dort ueber USB-HID-Polling
  wie bei `Cconin`.
- Tester: die Laeufe brauchen die Scratch-Instrumentierung nur als
  Tastenquelle (raspi4b) bzw. Startweg (virt, keine Karte);
  `method/apply-instrumentation.py` + `run-*.sh` + `drive.py` sind wiederholbar.

## Beobachtungen ausserhalb des Auftrags (unveraendert)

- SIMD-Register v0–v31 ueberleben einen `Pexec`/`Pterm`-Umlauf nicht
  (`finish_terminate` stellt nur GPRs her); clang hielt Vektorkonstanten ueber
  den `svc` — `crashtst.c` umgeht es (`noinline`/`volatile`).
- Der Kernel-Startweg (#Z/AUTO/Boot) betritt EL0 mit Kernelwerten in x1…x30
  (im Bericht sichtbar); der `Pexec`-Weg nullt sie.
- Schreiben auf Anwenderadresse 0 faellt nicht auf: Seite 0 ist `UserRwExec`.
- `SCTLR_EL1` = `0x30D0_0800`: `nTWI/nTWE/UCT/UCI/DZE` = 0 → `WFI`, `CTR_EL0`,
  `DC ZVA`, Cache-Pflege aus EL0 fangen (jetzt „Privilege Violation“ statt Halt).
- `tools/make-sd-image.sh` schreibt ueber den festen Pfad `/Volumes/NEWTOS`;
  laege eine echte Karte mit diesem Namen an, landete der Inhalt dort. Fuer
  diese Laeufe nicht benutzt.
- Ein erneuter Absturz im eigenen `etv_term`-Handler endet ueber die
  Upcall-Schachteltiefe mit `Pterm(-998)` (EmuTOS liefe in eine Schleife).
