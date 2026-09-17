# A66 — STATUS: DONE (Hostbeleg) / HW-Abnahme offen

`bb_save`/`bb_restore` nehmen den Mauszeiger jetzt selbst vom Schirm, solange
sie kopieren (EmuTOS' `bb_set`-Ebene); `form.rs`s eigene, zu spaet sitzende
Aussenklammer um `bb_restore` ist entfernt. Unabhaengig verifiziert
2026-09-18, Tester-Messung (nicht Entwickler-Selbstreport), Branch
`desk-rebuild`, Basis HEAD `a7896cf`. Messlauf:
[`measurement-2026-09-18-a66/`](../measurement-2026-09-18-a66/RUNINFO.TXT).

Diese Quittung dokumentiert die **Pruefung**, nicht die Implementierung
(die lag beim Start der Messung bereits uncommitted im Baum, ohne eigenes
Statusticket). Auftrag: A66 unabhaengig verifizieren, dem
Developer-Selbstreport nicht trauen.

## Der Fehler, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll), `bb_set` | `references/emutos/aes/gemgsxif.c:367` `gsx_moff()` … `:378` `gsx_mon()`, gezaehlt ueber `gl_moff` (`:440-455`) |
| EmuTOS (Soll), `fm_alert` | `references/emutos/aes/gemfmalt.c:363` (`bb_save`) … `:390` (`bb_restore`), **ohne eigene Klammer** — sie steckt in `bb_set` |
| NewTOS (vorher) | `tos-api/src/vdi/raster.rs` `bb_save`/`bb_restore` — keine Klammer; `tos-api/src/aes/form.rs:301-337` klammerte stattdessen aussen um `capture`/`restore`, aber **erst nach** `capture` schon lief (Geisterpfeil: der Zeiger stand beim Sichern noch auf dem Schirm) |
| NewTOS (nachher) | `tos-api/src/vdi/raster.rs` `bb_save`/`bb_restore` rufen `super::mouse::hide`/`show` selbst, symmetrisch um die Kopie; `form.rs` hat keine eigene Klammer mehr um `restore()` |

Gepinnte EmuTOS-Quelle (aus dem Ticket, nicht in dieser Messung neu gepinnt):
`references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`.

## Wo die Klammer sitzt — die im Ticket offene Entscheidung

A66 stellte zwei Optionen zur Wahl: die Klammer in `bb_save`/`bb_restore`
(EmuTOS' Ort, `bb_set`) oder in `form.rs` (nur diesen einen Aufrufer). Der
vorgefundene Fix waehlte **Option 1** — deckt jeden kuenftigen `bb_save`/
`bb_restore`-Aufrufer ab, genau wie EmuTOS' `bb_set`. `menu.rs`s eigene
Klammer (`:476-480`) nestet darauf, wie das Ticket vorhersah — der Zaehler
(`HIDE_COUNT`, A65-Mechanismus) traegt das, ohne dass `menu.rs` angefasst
wurde.

## Der Test: rot, dann gruen — beides selbst nachgefahren

`tos-api/tests/aes_form_alert_cursor_bracket.rs`, drei Faelle, 640×480
Falcon-Truecolor (dasselbe Testprofil wie A65). Rot-Gegenprobe **nicht** aus
dem Entwicklerbericht uebernommen, sondern selbst erzeugt: `git worktree add
--detach <scratch> a7896cf` (HEAD, der Stand vor dem Fix — kein `git stash`,
kein Antasten des geteilten Baums), den neuen Test dort hineinkopiert,
gebaut, gelaufen:

```
3 von 3 FAILED, jeweils:
  "... the box IS the ghost arrow, pixel for pixel"
```

Zweimal unabhaengig reproduziert (zwei separate Worktree-Kopien), identisch.
Danach gruen auf dem Fix-Stand: `3 passed; 0 failed`. Rohdaten:
`measurement-2026-09-18-a66/build/newtest-RED.txt` /
`newtest-GREEN.txt`.

## Pruefungen — alle selbst gefahren, keine uebernommen

| | Ergebnis |
|---|---|
| `aes_form_alert_cursor_bracket` (A66) | **3/3 gruen**; Rot-Gegenprobe auf HEAD **3/3 rot**, Geisterpfeil-Signatur |
| `aes_object_cursor_bracket` (A65-Regression) | **3/3 gruen**, unveraendert |
| `cargo run -p xtask -- test` | **563 bestanden / 0 gefallen / 1 uebersprungen** — Sollstand exakt |
| rustc-Warnungen, beide Boards | tos-api **64**, kernel **94** — Sollstand, Delta 0 |
| clippy-Warnungen, beide Boards | tos-api **81**, kernel **125** — Sollstand, Delta 0 |
| QEMU raspi4b, TeraDesk per `#Z`-Autorun | Screendump **0 von 307200** Bildpunkten verschieden gegen `measurement-2026-09-17-h03/verify/screendumps/verify-run1-teradesk-startup.png` |
| QEMU raspi4b, unser Desktop, kein Autorun | Screendump **24511 von 307200** Bildpunkten verschieden gegen `measurement-2026-09-17-h02/verify/screendumps/runB-desktopup-final.png` — **Icon- vs. Text-View des C:\-Fensters**, keine Geisterpfeil-Signatur, keine A66-bezogene Abweichung; die Referenz ist der Endzustand einer mehrstufigen H02-Interaktionssequenz, kein einfacher Boot-Abzug. Details und Einordnung: `measurement-2026-09-18-a66/RUNINFO.TXT` Abschnitt 4b |
| `git status --short` | fuenf Pfade: die drei aus dem Auftrag + `docs/api/aes/object-form.md` + `docs/api/vdi/raster/_raster.md` (Spec-Zeilen zum Fix, im Auftragsvorspann nicht genannt, aber inhaltlich zum Fix gehoerig — CLAUDE.md Regel 5) |

Verfahren, Rohdaten, Hashes: `measurement-2026-09-18-a66/RUNINFO.TXT`,
Werkzeuge aus `measurement-2026-09-17-a65/method/` unveraendert
wiederverwendet.

## Was offen bleibt — HW-Abnahme

QEMU `raspi4b` hat keinen Eingabeweg (`[pcie] RC not present ... USB
disabled`); der Zeiger bewegt sich dort nie, der eigentliche Fehler
(Geisterpfeil nach `form_alert`-Schliessen) kann headless nicht ausgeloest
werden. Die Ticket-Abnahme bleibt deshalb eine **Vorhersage**, auf der
echten Pi 4 zu pruefen:

1. `form_alert` ueber einem sichtbaren Zeiger oeffnen, schliessen: **vorher**
   blieb ein Pfeil an der Oeffnungsposition stehen (Ticketbefund); **nachher**
   darf dort kein Pfeil stehen bleiben.
2. Der Hosttest deckt exakt diese Signatur (16×16-Box ab dem Sprite-Ursprung,
   ARROW-Bitmuster) — die Vorhersage ist also durch den Test gestuetzt, aber
   nicht durch eine reale Messung ersetzt.

## Aus dem Skopus, nicht angefasst

- Die abweichende Icon/Text-Ansicht im Desktop-Vergleich (siehe Tabelle
  oben) — Ursache nicht ermittelt, ausserhalb A66.
- Die zwei nicht im Auftragsvorspann genannten `docs/api/`-Aenderungen —
  gemeldet, nicht bewertet.

## Berührte Dateien (durch diese Messung neu, kein Produktcode)

```
measurement-2026-09-18-a66/                  (Messlauf: RUNINFO.TXT,
                                              screendumps/, serial/, build/)
TASK/A66-STATUS-DONE.MD                      (diese Quittung)
```

Produktcode (`tos-api/src/aes/form.rs`, `tos-api/src/vdi/raster.rs`,
`tos-api/tests/aes_form_alert_cursor_bracket.rs`, die beiden `docs/api/`-
Dateien) lag bereits uncommitted im Baum und wurde durch diese Messung nicht
veraendert.
