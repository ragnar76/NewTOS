# DESK60 — STATUS: DONE (2026-10-16) — nur Logtext, Verhalten EmuTOS-gleich; nicht committet

**Auftrag:** Nach einem per `#Z` gestarteten Programm (TeraDesk) meldet die Shell „starting DESKTOP.NAP“, obwohl NEWDESK bzw. der
eingebaute Desktop hochkommt. Gegen EmuTOS `sh_main`/`sh_ldapp` pruefen, ob nur das Log oder auch Verhalten betroffen ist
(`TASK/DESK60.md`).

## Befund (EmuTOS `aes/gemshlib.c`)

| Schritt | EmuTOS | NewTOS |
|---|---|---|
| `#Z` | `sh_wdef(prog, dir)` (`geminit.c:455`) → `sh_desk`/`sh_cdir` | `shell::sh_wdef` |
| Durchlauf 1 | `sh_chdef` AUTORUN: `D.s_cmd = sh_cdir\sh_desk` (`:557-564`) | gleich |
| TeraDesk | `shel_write(SHW_NOEXEC)` nach jedem `Pexec` (`startprg.c:386`): `D.s_cmd = DEF_DESKTOP`, `sh_nextapp = DESKTOP_APP`, `sh_desk` bleibt (`:161-166`) | gleich |
| Ende | `set_default_desktop` nur wenn `sh_nextapp` noch AUTORUN (`:662-666`) → **uebersprungen** | gleich (live gelesen) |
| Durchlauf 2 | `sh_chdef` DESKTOP: `D.s_cmd = sh_desk` = `DESKTOP.NAP` (`:564`) | gleich |
| DESKTOP-Arm | startet `deskstart` unabhaengig von `D.s_cmd` (`:606`); `p_nameit(rlr, sh_name(D.s_cmd))` → `DESKTOP` (`:594,604`) | startet NEWDESK/eingebauten Desktop; Name aus `cmd` → `DESKTOP` |

Folge: `D.s_cmd` ist nicht „veraltet“, sondern aus dem unveraenderten `sh_desk` neu gebaut — in EmuTOS identisch. Auch Verhalten
gleich: Deskriptorname `DESKTOP` (`appl_find("NEWDESK")` faende den Desktop in diesem Zustand nicht, wie EmuTOS `appl_find("EMUDESK")`
— aus dem Code abgeleitet, nicht gemessen), `shel_read` liefert `DESKTOP.NAP`, aktuelles Verzeichnis `C:\Apps\TERADESK`. Nur unsere
Logzeile tat so, als werde `D.s_cmd` gestartet. Daher nur der Logtext korrigiert, kein Verhaltenseingriff.

## Aenderung

* `kernel/src/shell_loop.rs` `sh_ldapp`: DESKTOP-Arm loggt
  `[shell] sh_ldapp: starting desktop <NEWDESK|(built-in)|(none: no screen)> (D.s_cmd <s_cmd>), sh_nextapp=.., sh_isgem=..`
  und `desktop NEWDESK (D.s_cmd <s_cmd>) exited with rc=N`; neue Hilfsfunktion `desktop_label()`. Die anderen Arme behalten die alte
  Zeile; Praefix `[shell] sh_ldapp: starting ` fuer alle Arme unveraendert. Kommentar mit EmuTOS-Zitaten am Arm.
* `tos-api/tests/aes_shell_desk60_autorun_noexec.rs` (neu, 2 Tests): EmuTOS-Folge oben ueber den echten Dispatcher
  (`shel_write`/`shel_read`) und `sh_wdef`/`sh_main_init`/`sh_chdef`; Gegenfall ohne `shel_write` → `D.s_cmd = NEWDESK`.
  Gruen; pinnt die (bereits richtige) Logik, ist daher kein Rot-Gruen-Test fuer den Logtext.

## Beleg (QEMU raspi4b, `method/desk60.sh`, Kernel `kxtd`, Karte tdqed, mode on)

* EL0: `[shell] sh_ldapp: starting desktop NEWDESK (D.s_cmd DESKTOP.NAP), sh_nextapp=3, sh_isgem=1` → `[desk] desktop up`
* EL1 (`desk=kernel`): `[shell] sh_ldapp: starting desktop (built-in) (D.s_cmd DESKTOP.NAP), sh_nextapp=3, sh_isgem=1` → `[desk] desktop up`
* 8/8 Dumps byte-gleich zur DESK43-Abnahme und EL0 = EL1; 0 EXC/PANIC.
* Keine Regression: 45/45 Ansichten/Serien-Dumps (on/off/strict) byte-gleich zu DESK58; APITEST 106/0/5.

## Zahlen

| | Soll | gemessen |
|---|---|---|
| Hosttests tos-api/tos-types/xtask | 1370/0/1 + neu | 1372/0/1 |
| Warnungen rpi4 / qemu-virt | 64/94 | 64/94 beide |
| Clippy Kernel rpi4 / qemu-virt / +selftest-desk | 76/125 | 76/125 alle (0 Treffer in shell_loop.rs) |
| Programme (build + clippy) | 0 | 0 |
| Drift / unsafe | 4 / 20 | 4 / 20 |
| emutos-consts | gruen | 466/333/1/1/0/0/132 |
| SDK | 20/0/0 | 20/0/0 |

Beleg: `measurements/measurement-2026-10-16-t16-desk60/RUNINFO.TXT`.

## Offen

* Echte Pi 4 (keine HW-Wirkung: reiner Logtext). qemu-virt-Boot, on-asid/verify nicht gefahren.

## Ausserhalb des Auftrags (je eine Zeile, nicht angefasst)

* Nach `#Z`-TeraDesk + `SHW_NOEXEC` heisst der Desktop-Deskriptor `DESKTOP` und das Startverzeichnis ist `C:\Apps\TERADESK` — EmuTOS-gleich, bewusst belassen.
* `docs/api/aes/shell-loop.md` (Abschnitt DEF_DESKTOP) sagt, `D.s_cmd`/`sh_desk` wuerden nie von einer Anwendung gelesen; `shel_read` liest `D.s_cmd` (`gemshlib.c:104`).

## Selbstpruefung

Aufgabe in einem Satz: klaeren, ob „starting DESKTOP.NAP“ nur Log oder Verhalten ist, und nach EmuTOS beheben — Ergebnis: nur Log,
Logtext korrigiert, EmuTOS-Folge per Hosttest gepinnt. Geaendert: `kernel/src/shell_loop.rs`, `tos-api/tests/aes_shell_desk60_autorun_noexec.rs`;
diese Quittung, `TASK/DESK60.md`, das Messverzeichnis. Beide Boards bauen (64/94, Clippy 76/125). Nicht committet; `docs/STATUS.md` nicht angefasst.
