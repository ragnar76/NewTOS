# A67 — STATUS: DONE (Hostbeleg + QEMU) / HW-Sichtabnahme offen

3D-Kastentypen (`FL3DIND`/`FL3DACT` auf `G_BOX`/`G_IBOX`/`G_BOXCHAR`/
`G_BOXTEXT`/`G_FBOXTEXT`) behalten jetzt ihren 1-Pixel-Rand **und** die Fase,
wie EmuTOS (`gemoblib.c:663-667` zeichnet den Rand bei jedem `th != 0`
unabhaengig von 3D, `add_3d_effect` legt die Fase danach obendrauf). Die
Fasenfunktion `add_3d_effect` selbst ist dabei komplett neu gefasst worden
(1-Punkt-Polylinie statt 2-Punkt-Fuellrechtecke, EmuTOS-genaue
WHITE/BLACK+LBLACK/LWHITE-Logik mit `icol`-Fallback). Unabhaengig verifiziert
2026-09-18, Tester-Messung (nicht Entwickler-Selbstreport), Branch
`desk-rebuild`, Basis HEAD `4c475e0`. Messlauf:
[`measurement-2026-09-18-a67/`](../measurement-2026-09-18-a67/RUNINFO.TXT).

Diese Quittung dokumentiert die **Pruefung**, nicht die Implementierung (die
lag beim Start der Messung bereits uncommitted im Baum). Auftrag: A67
unabhaengig verifizieren, dem Developer-Selbstreport
(`$CLAUDE_JOB_DIR/tmp/a67/`) nicht trauen.

## Der Fehler, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll), Rand | `references/emutos/aes/gemoblib.c:661-667` (`just_draw`, Rand bei jedem `th != 0`) |
| EmuTOS (Soll), Fase | `references/emutos/aes/gemoblib.c:220-283` (`add_3d_effect`), `:950-951` (Aufrufstelle, nach dem State-Block) |
| NewTOS (vorher) | `tos-api/src/aes/object.rs`: `th = 0` fuer jede `FL3DIND`/`FL3DACT`-**Box** (Rand durch Fase ersetzt statt ergaenzt); `add_3d_effect` 2px-Fuellrechtecke, raised/!raised statt selected, ohne `icol`-Fallback |
| NewTOS (nachher) | Rand = Spec-Byte-Staerke fuer **jeden** 3D-Kind (Box wie Text); `add_3d_effect` mit `(selected, th, icol)`-Signatur, 1px-Polylinien, schrumpft bei `th > 0` um den Rand |

## Test: rot, dann gruen — beides selbst nachgefahren

`tos-api/tests/aes_object_3d_box_border.rs`, vier Faelle (FSELECTR-Laufwerk A
unselektiert/selektiert, Innen-Rand, `G_BOXTEXT`), 640x480 Falcon-Truecolor,
gegen `fselectr.png`-Pixel bei den FSELECTR-Koordinaten.

Grün, in-tree:
```
running 4 tests
test fl3dind_boxchar_with_inside_border_draws_the_bevel_inside_the_border ... ok
test fl3dind_boxchar_with_outside_border_matches_the_reference_drive_box ... ok
test fl3dind_boxtext_with_outside_border_keeps_it_under_the_bevel ... ok
test selected_fl3dind_boxchar_keeps_the_border_and_inverts_the_bevel ... ok
test result: ok. 4 passed; 0 failed
```

Rot-Gegenprobe **selbst erzeugt**, nicht aus dem Entwicklerbericht
uebernommen: `git worktree add --detach <scratch> 4c475e0` (HEAD, Stand vor
dem Fix — kein `git stash`, kein Antasten des geteilten Baums), Test dort
hineinkopiert, gebaut, gelaufen:

```
test result: FAILED. 0 passed; 4 failed; 0 ignored
```

Rohdaten: `measurement-2026-09-18-a67/build/newtest-current-tree-GREEN.log` /
`newtest-worktree-HEAD-RED.log`.

## Prüfungen — alle selbst gefahren, keine übernommen

| | Ergebnis |
|---|---|
| `aes_object_3d_box_border` (A67) | **4/4 grün**; Rot-Gegenprobe auf HEAD **4/4 FAILED** |
| `cargo run -p xtask -- test` | **567 bestanden / 0 gefallen / 1 übersprungen** — Sollstand exakt |
| rustc-Warnungen, beide Boards (frisch, eigener `CARGO_TARGET_DIR`) | tos-api **64**, kernel **94** — Sollstand, Delta 0 |
| clippy-Warnungen, beide Boards (`--release`) | tos-api **81**, kernel **125** — Sollstand, Delta 0 |
| QEMU raspi4b, Dateiauswähler (`fsel::preview()`-Hook, Scratch-Workspace) | **349 von 356** wurzelrelative Zeilen identisch, **7** verschieden — exakt Zeile 146, 147, 148, 161, 167, 174, 198 (Schiebereinzug + Sanduhr, beide nicht A67-Ursache) |
| QEMU raspi4b, `form_alert` (`draw_alert`, G_BUTTON+FL3DACT), vorher/nachher | **420 von 307200** Bildpunkten verschieden, ausschließlich im Rechteck der OK/Cancel-Knöpfe (x 230–409, y 256–281) — Ursache: die neue `add_3d_effect` zeichnet 1px statt 2px Fase; Sichtprüfung (4x-Zoom) zeigt eine saubere 1-Pixel-Fase weiß oben/links, dunkel unten/rechts, kein Bildfehler |
| QEMU raspi4b, unser Desktop, kein Autorun, vorher/nachher | **0 von 307200** Bildpunkten verschieden — keine FL3DIND/FL3DACT-Boxen an dieser Stelle |
| QEMU raspi4b, TeraDesk per `#Z`-Autorun | **0 von 307200** Bildpunkten verschieden gegen `measurement-2026-09-17-h03/verify/screendumps/verify-run1-teradesk-startup.png` (vor dem Fix aufgenommen) — TeraDesks Start-Icons sind keine FL3DIND/FL3DACT-Boxen |
| `git status --short` | nur die zwei Fix-Dateien (`tos-api/src/aes/object.rs`, `tos-api/tests/aes_object_3d_box_border.rs`) |
| `kernel/src/lib.rs` im Projektbaum | MD5 `5434f7b25c5641c7fab1fce0b1c98b85` vorher = nachher — unangetastet; alle temporären `fsel::preview()`/`draw_alert()`-Hooks liefen ausschließlich in Scratch-`git worktree`-Kopien, nie im Projektbaum |

Verfahren, Rohdaten, Hashes: `measurement-2026-09-18-a67/RUNINFO.TXT`.
Werkzeuge aus `measurement-2026-09-17-fsel/method/` und
`measurement-2026-09-17-h03/method/` unverändert wiederverwendet.

## Ein Befund, der über das Ticket hinausgeht

A67.md beschreibt nur den Rand der Kastentypen. Der tatsächliche Diff geht
weiter: `add_3d_effect` selbst wurde neu gefasst und wirkt auf **jedes**
`FL3DOBJ` — auch `G_BUTTON` (z.B. jeder `form_alert`-Knopf), dessen
Randstärke unverändert blieb, dessen **Fase** aber jetzt 1px statt 2px breit
ist und EmuTOS' `icol`-Fallback-Logik trägt. Das ist im gemessenen Bild
korrekt und sauber gezeichnet (siehe Tabelle), aber es ist eine Reichweite,
die im Ticket nicht benannt ist und die die Quittung deshalb hier
nachträgt.

## Was offen bleibt — HW-Sichtabnahme

Die 1-Pixel-Fase (Farbe, Cache-Verhalten der Non-Cacheable-FB-Pfad) ist nur
in QEMU-TCG gemessen. Reale Pi-4-Sichtprüfung der Dialogknöpfe (1px-Fase
weiß oben/links, dunkel unten/rechts, Ecken oben-rechts/unten-links in
Flächenfarbe, schwarzer Ring außen bei `th=-1`) gehört auf
`docs/api/desktop/_hw-test-checklist.md` — nicht durch diese Messung
ersetzbar.

## Berührte Dateien (durch diese Messung neu, kein Produktcode)

```
measurement-2026-09-18-a67/                  (Messlauf: RUNINFO.TXT,
                                              screendumps/, serial/, compare/, build/)
TASK/A67-STATUS-DONE.MD                      (diese Quittung)
```

Produktcode (`tos-api/src/aes/object.rs`,
`tos-api/tests/aes_object_3d_box_border.rs`) lag bereits uncommitted im Baum
und wurde durch diese Messung nicht verändert.
