# Volle Gegenmessung 2026-09-12 — Regressionssuche nach B1, B2, A22, A34, A33, A37

Scope: alle neun Falltabellen unter `tosdump/cases/` frisch gegen den
aktuellen Arbeitsbaum (`70caee2`, Branch `desk-rebuild`, inkl. der
ILP32-Ladesperre) gemessen — je einmal gegen die EmuTOS-Referenz (absoluter
Stand) und einmal gegen den letzten aufgezeichneten NewTOS-Vorstand
(Regressionsfrage). Kein Produktcode angefasst, keine Diagnose gegen die
EmuTOS-Quelle — nur Zahlen gegen Zahlen.

Rohdumps, Boot-Logs und Diffs: `measurement-2026-09-12-full-regression/`.
Metadaten (Commit, SHA-256, Maschinenparameter): dortiges `RUNINFO.TXT`.

## Kurzantwort auf die Hauptfrage

**Kein Fall ist neu aufgebrochen.** In keiner der neun Tabellen ist ein Fall
schlechter geworden. Zwei Tabellen (`aes-r2b-rlist`, `aes-r3-wind-state`) haben
einen zum aktuellen Kernel passenden 640×480-QEMU-Vorstand und sind
byte-genau nachgemessen: `aes-r3-wind-state` reproduziert den
Nach-A37-Vorstand von heute Morgen **exakt** (0 von 39 Fällen anders),
`aes-r2b-rlist` weicht vom Vor-den-sechs-Reparaturen-Vorstand nur in den zwei
dokumentierten Rauschfeldern ab (0 echte Nutzlast-Abweichungen).

Für fünf Tabellen existiert nur ein Full-HD-Vorstand von echter Pi-4-Hardware
(9. September); dafür wurde zusätzlich ein 1920×1080-QEMU-Lauf gefahren, um
den Vergleich profilgleich zu halten. Zwei Tabellen (`vdi-r1`, `vdi-r3`) haben
**nie zuvor** einen NewTOS-Vorstand bekommen — dort ist die Regressionsfrage
mangels Vergleichsbasis nicht beantwortbar, nur der heutige Absolutstand.

**Unerwartet besser geworden:** `vdi-r2` — `d0`/`flip_y` wird jetzt für 16
Opcodes korrekt erzeugt (vorher immer 0) und zwei vergiftete Sentinel-Felder
(`intout[33]` bei `vqt_name`, `intin[0]` bei `v_dspcur`) sind jetzt korrekt
genullt; alle drei Punkte stimmen heute mit der EmuTOS-Referenz überein, taten
es im 31.-August-Vorstand nicht. Das ist keiner der sechs benannten Fixes —
vermutlich Nebenwirkung einer der vielen anderen uncommitteten Änderungen im
Baum; Ursachen-Zuordnung ist Auditor-Arbeit, nicht Teil dieses Berichts.

> **Erratum (A53, 2026-09-20).** Die Vermutung "uncommittete Änderung" ist
> falsch. Ursache sind drei **committete** Fixes im `git log` des VDI-Pfades
> zwischen Vorstand und Messung: `675e311` ("return d0 from the dispatcher,
> and two adjudicated defects" — behebt genau den `d0`/`flip_y`-Stub),
> `f76a666` ("MAX_VERTICES is 1024, and the last ten red cases adjudicated")
> und `1eb4e68` ("set the VDI string device to SAMPLE on every dispatch").
> Belegt in `docs/analysis/vdi-r2-improvement-traced-2026-09-12.md` §2, die
> den Zusammenhang schon zum Zeitpunkt dieser Messung nachgetragen hat.

## Tabelle über alle neun Falltabellen

| Tabelle | Läuft durch | Profil heute | Abw. ggü. EmuTOS heute | Regressions-Basis | Abw. ggü. Vorstand | Neu aufgebrochen |
|---|---|---|---|---|---|---|
| `aes-r1` | ✅ 95/95, CRC ok | 640×480 + 1920×1080 | 1 Fall (167, WF_OWNER-Versionsdelta) | `measurement-2026-09-09/run1-pi4` (Full-HD) | 0 echte (nur `global[2,3,11,12]`-Rauschen) | **keiner** |
| `aes-r2-wind` | ✅ 78/78, CRC ok | 640×480 + 1920×1080 | 2 Fälle (244/247, dieselbe Familie) | `measurement-2026-09-09/run2-pi4-r2wind` (Full-HD) | 0 echte (Rauschen + 2×`int_out[1]`, beides `ap_id`-Folge) | **keiner** |
| `aes-r2b-rlist` | ✅ 15/15, CRC ok | 640×480 | 4 Fälle (127/133/144/204, dokumentierte P1-Stapelreste) | `measurement-2026-09-10-qemu/run3-qemu-rlist-all6` (640, **vor** den sechs Reparaturen) | 0 echte (nur `global[11,12]`-Rauschen) | **keiner** |
| `aes-r3-wind-state` | ✅ 39/39, CRC ok | 640×480 | 7 Fälle (261/280/400/481/494/574/588, dokumentierte P1-Stapelreste) | `measurement-2026-09-12-qemu-r3-a37` (640, **nach** allen sechs Reparaturen, heute 08:51) | **0 von 39 — byte-identisch** | **keiner** |
| `vdi-r1` | ✅ 24/24, CRC ok | 640×480 | 4 Fälle (36/37/46/59, bekannte Familien) | **keiner existiert** | n/a | n/a — Erstmessung |
| `vdi-r2` | ✅ 120/120, CRC ok | 640×480 + 1920×1080 | 4 Fälle (46/89/140/141, bekannte Familien) | `reference/NEWTOS-VDI-r2.BIN` (Full-HD, 31. Aug) | 22 Fälle anders, **alle als Verbesserung erklärt** (`d0`/Sentinel, s. u.) | **keiner verschlechtert** |
| `vdi-r3` | ✅ 38/38, CRC ok | 640×480 | 5 Fälle (117/119/140/142/145, bekannte Familien) | **keiner existiert** | n/a | n/a — Erstmessung |
| `vdi-all` | ✅ 182/182, CRC ok | 640×480 + 1920×1080 | 13 Fälle (bekannte Familien, s. u.) | `measurement-2026-09-09/run1-pi4` (Full-HD) | 182 Fälle „anders“, **restlos als Handle-Artefakt der alten Mehrprogramm-Sitzung erklärt** | **keiner** |
| `vdi-raster` | ✅ 59/59, CRC ok | 640×480 + 1920×1080 | 1 Fall (`buf_b[511]`, ein Wort) | `measurement-2026-09-09/run1-pi4` (Full-HD) | 59 Fälle „anders“, **derselbe Handle-Artefakt** | **keiner** |

Jeder der 14 Läufe (9× 640×480, 5× zusätzlich 1920×1080) hat `all N cases ran`,
die erwartete Datensatz-/Byteanzahl, `tosdiff --validate` akzeptiert die
Tabellen-CRC, und der Bootlog ist frei von `[EXC]`/`PANIC` — der eingebaute
Desktop kam in jedem einzelnen Lauf danach hoch (`[desk] desktop up (1
drives)`).

## Einordnung der Regressionsvergleiche im Detail

### `aes-r3-wind-state` — die schärfste Probe, hält exakt

`tosdiff cases/aes-r3-wind-state.tsv measurement-2026-09-12-qemu-r3-a37/AES.BIN
measurement-2026-09-12-full-regression/aes-r3-wind-state/AES.BIN` meldet
**„0 of 39 cases differ“**. Der a37-Vorstand von heute 08:51 war bereits nach
allen sechs Reparaturen genommen; zwischen 08:51 und diesem Lauf hat sich am
AES-Fensterpfad nichts mehr geändert. Das ist die stärkste verfügbare
Bestätigung: kein Fall hat sich seit dem letzten Fix-Durchgang bewegt, weder
zum Schlechteren noch zum Besseren.

### `aes-r2b-rlist` — vor die sechs Reparaturen zurückverglichen

Vorstand ist `measurement-2026-09-10-qemu/run3-qemu-rlist-all6/AES.BIN`
(640×480, **vor** B1/B2/A22/A34/A33/A37). Alle 15 Fälle zeigen einen
Unterschied, aber ausschließlich in `global[11]` (kernel-adress-abhängiges
Wort, ändert sich mit jedem Build) und `global[12]` (`ap_3resv`,
uninitialisierter Stapelwert) — beides bereits in der Aufgabenstellung als
erwartetes Rauschen benannt. **Keine `int_out`-Zeile** in diesem Diff. Die vier
Fenster-Fixes (A22 Rechteck-Reihenfolge, A34/A33 `wind_create`/`wind_delete`)
wirken sich auf diese 15 älteren Fälle nicht sichtbar aus — plausibel, weil die
größere `aes-r3`-Tabelle gezielt für genau diese Szenarien gebaut wurde und die
kleinere `r2b-rlist`-Tabelle keine sich überlappenden Fenster bzw. keinen
Slot-Recycling-Fall enthält, an denen A22/A33/A34 etwas ändern. Kein Befund,
keine Regression.

### `vdi-r2` — die Verbesserung, im Detail

Vorstand `tosdump/reference/NEWTOS-VDI-r2.BIN` (31. Aug, ebenfalls ein
frischer Einzelprogramm-Boot: `appl_init id 1, v_opnvwk handle 2` — identisch
zu heute, also **kein** Sitzungs-Artefakt). 22 von 120 Fällen unterscheiden
sich:

- **16 Fälle** (Opcodes 12, 19, 21, 105, 107, 116, 117, 131 — `vst_height`,
  `vsm_height`, `vst_font`, `vqt_fontinfo`, `vst_point`, `vqt_extent`,
  `vqt_width`): `d0[0]` war `0`, ist jetzt `1`.
- **4 Fälle** (`vqt_name` Font-Index 0–3): `intout[33]` war `0xDEAD`
  (der Vergiftungs-Sentinel des Dumpers, also unberührter Speicher), ist
  jetzt `0x0000`.
- **1 Fall** (`v_dspcur`): `intin[0]` ebenso von `0xDEAD` auf `0x0000`.
- **1 Fall** (`vqin_mode string`): `intout[0]` von `0` auf `1`.

Gegen die EmuTOS-Referenz von heute (`DIFF-vs-emutos.txt`) tauchen **keine**
dieser 22 Fälle auf — der heutige Wert trifft die Referenz exakt, wo der
31.-August-Vorstand es nicht tat. Das erfüllt Frage 2 der Aufgabenstellung
(„etwas besser geworden, das nicht erwartet war"): ja, aber es ist keiner der
sechs benannten Fixes und wurde hier nicht weiter zurückverfolgt — das ist
Auditor-Arbeit.

### `vdi-all` / `vdi-raster` — der Handle-Unterschied ist Methodik, kein Befund

Beide Male ändert sich **ausschließlich** `contrl[6]`/`contrl_before[6]`
(bzw. der davon abgeleitete `INTOUT`-Farbzahl-/Handle-Bereich), und zwar
uniform über alle Fälle. Ursache gefunden, nicht vermutet: der
9.-September-Vorstand lief als **drittes** Programm in einer geteilten
Boot-Sitzung — `RASTER.LOG` dort zeigt `appl_init id 3, v_opnvwk handle 4`,
weil VDI- und AES-Dumper vorher im selben Boot liefen (`MESSRUN.TXT`,
„DURCHGANG 1 — drei Programme, ein Boot"). Der heutige Lauf **und** die
EmuTOS-Referenz sind beide frische Einzelprogramm-Boots (`id 0`/`id 1`,
`handle 2`) — und der heutige `contrl[6]`-Wert **trifft die EmuTOS-Referenz
exakt** (0 Treffer für dieses Feld im `DIFF-vs-emutos.txt`). Die Abweichung
zum 9.-September-Vorstand ist also ein Artefakt der damaligen
Mehrprogramm-Sitzung, keine Code-Änderung — bestätigt genau die im Auftrag
vorab genannte Kategorie „Workstation-Handle", nur in einer Tabelle, in der
sie zuvor nicht explizit vermerkt war.

### `vdi-r1` / `vdi-r3` — ehrliche Lücke

Für diese beiden Tabellen wurde **nie zuvor** ein NewTOS-Dump archiviert
(weder in `tosdump/reference/` noch in einem `measurement-*`-Verzeichnis).
Der heutige Lauf ist die Erstmessung. Die Regressionsfrage ist für diese
beiden Tabellen **nicht beantwortbar** — das wird hier so berichtet, nicht mit
einer erfundenen Zahl gefüllt. Der absolute Stand gegen EmuTOS zeigt in beiden
Fällen nur Abweichungen aus bereits an anderer Stelle dokumentierten Familien
(`vq_extnd(0)/(1)`-Konstanten, `v_get_pixel`-Farbmodus, Farbzahl-Klemmung bei
`vsl_color`/`vsm_color`/`vq_color`, Zusatz-Workstation-Farbzahl) — siehe
`tosdump/reference/FINDINGS-r1.md`, `FINDINGS-newtos-r2.md`.

### `aes-r1` / `aes-r2-wind` — Profilkorrektur nötig, dann sauber

Der einzige aufgezeichnete Vorstand für beide Tabellen ist der
9.-September-Lauf auf echter Pi-4-Hardware bei 1920×1080. Ein direkter
Vergleich mit einem 640×480-Lauf hätte Geräteparameter statt Code verglichen;
deshalb wurde zusätzlich ein 1920×1080-QEMU-Lauf gefahren. Danach: `aes-r1`
zeigt nur die vier dokumentierten `global[]`-Rauschfelder (`ap_id`,
`ap_private`, Kernel-Adresse, `ap_3resv`), keine `int_out`-Abweichung.
`aes-r2-wind` zeigt dieselben vier plus zwei `int_out[1]`-Zeilen bei den
Fällen 244/247 — das sind exakt die WF_OWNER-Fälle, deren Antwort die eigene
`ap_id` enthält, also eine direkte Folge desselben `ap_id`-Rauschens, keine
unabhängige Verhaltensänderung.

## Bekannte, erwartete Abweichungen (bestätigt, kein neuer Befund)

- `aes-r3-wind-state`: die sieben Fälle 261, 280, 400, 481, 494, 574, 588
  (EmuTOS-Dispatcher-Stapelrest) — **exakt diese sieben**, keiner mehr.
- `aes-r2b-rlist`: die vier Fälle 127, 133, 144, 204 (Stapelreste ohne Soll) —
  deckungsgleich mit `measurement-2026-09-11-pi4/run1-rlist-fixed/DIFF-gegen-TT-und-QEMU.txt`.
- `aes-r1`/`aes-r2-wind`: die WF_OWNER-Versionsdelta-Familie (Fälle 167 bzw.
  244/247) — EmuTOS 1.4 kennt `WF_OWNER` noch nicht (kam erst nach dem
  `VERSION_1_4`-Tag, `acb7f3b4`).
- Alle AES-Läufe: `global[]`-Wörter `ap_id`, `ap_private`, `ap_planes`
  (4 gegen 16), `ap_3resv` weichen in jedem Fall ab — bestätigt in jeder der
  vier AES-Tabellen.
- VDI/RASTER: Farbzahl-Klemmung (`vsl_color`/`vsm_color`/`vq_color` „out of
  range"/„pen 255"), `v_get_pixel`-Farbmodus, `vq_extnd`-Konstanten
  (`INQ_TAB`/`DEV_TAB`-Felder) — bestätigt in `vdi-r1`, `vdi-r2`, `vdi-r3`,
  `vdi-all` gegen dieselben Fallnamen wie in `FINDINGS-r1.md`/
  `FINDINGS-newtos-r2.md`.
- Workstation-Handle-Familie: jetzt auch für `vdi-all`/`vdi-raster` als reines
  Sitzungsartefakt identifiziert (s. o.), nicht als Geräteparameter im
  eigentlichen Sinn.

**Nichts davon liegt außerhalb der vorab genannten Kategorien** — in keiner
Tabelle ist eine Abweichung aufgetaucht, die nicht in eine der bekannten
Familien fällt oder als Sitzungsartefakt erklärt werden konnte.

## `cargo xtask test` und beide Boards

- `cargo run -p xtask -- test`: **472 passed / 0 failed / 1 ignored** — exakt
  der geforderte Sollstand.
- `cargo run -p xtask -- build --board rpi4`: sauber, 94 Compiler-Warnungen
  (unverändert gegenüber dem in `STATUS.md` für A34 festgehaltenen Stand).
- `cargo run -p xtask -- build --board qemu-virt`: sauber, ebenfalls 94
  Compiler-Warnungen im Kernel.
- `cargo build -p tos-api`: 64 Warnungen (unverändert).
- `cargo clippy -p tos-api`: 81 Warnungen (unverändert gegenüber dem
  A34-Stand von heute Morgen).
- `cargo clippy -p kernel` (beide Boards): 125 Warnungen, auf `board-rpi4`
  und `board-qemu-virt` **identisch** (keine Board-Asymmetrie). Für diese
  spezifische Zahl gibt es keinen näher als den 2026-09-05-Eintrag
  liegenden Vergleichswert (dort 124) — über fünf Wochen Weiterentwicklung
  ist das kein aussagekräftiger Regressionsvergleich für den heutigen
  Sechs-Fixes-Durchgang, deshalb nur als Messwert vermerkt, nicht als
  Befund gewertet.

Kein Delta gegenüber dem zuletzt aufgezeichneten Stand in irgendeiner dieser
Zahlen (außer dem nicht aussagekräftigen Kernel-Clippy-Altvergleich).

## Konnte nicht (vollständig) geprüft werden

- **`vdi-r1`, `vdi-r3` Regressionsfrage**: kein Vorstand existiert, weder in
  `tosdump/reference/` noch in einem `measurement-*`-Verzeichnis. Nur der
  Absolutstand gegen EmuTOS ist heute belegt.
- **Ursache der `vdi-r2`-Verbesserung**: nicht auf einen Commit
  zurückverfolgt — das ist Auditor-/Git-Archäologie-Arbeit, nicht Teil einer
  Testerabnahme.
- **Reale Hardware**: alles hier ist QEMU (`raspi4b`, headless). Die
  QUIRK-001-Klasse (TTBR0-Wedge) und Cache-Kohärenz-Effekte der
  Non-Cacheable-Framebuffer-Klasse sind mit QEMU-TCG grundsätzlich nicht
  prüfbar — für diesen rein zahlenbasierten Dumper-Vergleich ohne
  interaktive Eingabe ist das nicht einschlägig, wird aber der Vollständigkeit
  halber genannt.
- **Bildliche Prüfung (Screendump)**: nicht angefordert und hier nicht
  gemacht — der Auftrag war eine Zahlenvergleichsmessung, keine
  Render-Verifikation.

## Out-of-scope-Beobachtungen (nicht verfolgt)

- Der Arbeitsbaum trägt 109 geänderte/ungetrackte Pfade gegenüber `HEAD`,
  weit mehr als die sechs benannten Reparaturen (u. a. `kernel/src/drivers/
  usb/*`, `tools/xtask/src/main.rs`, `tools/mkrsc.py`, mehrere
  `tos-api/src/aes/*`-Dateien) — nicht angerührt, nur zur Kenntnis genommen.
- `tools/mkrsc.py` reserviert weiterhin nicht die vollen `te_txtlen` Bytes
  (in A37 als offen benannt) — nicht erneut geprüft, da außerhalb des
  heutigen Scopes.

## Aufräumen

Kein QEMU-Prozess blieb nach einem Lauf zurück (`pgrep` nach jedem der 14
Läufe geprüft, zuletzt: `none running`). Keine physische Karte
(`/Volumes/BOOT`, `/Volumes/TOOLS`, `/Volumes/NEWTOS1-3`) wurde berührt — alle
Boot-Abbilder lagen unter `$CLAUDE_JOB_DIR/tmp/tester-regress/` und wurden
nach jedem Lauf gelöscht. Keine Produktdatei wurde verändert; keine
bestehenden Mess-Artefakte (`tosdump/cases/`, `tosdump/reference/`, ältere
`measurement-*`) wurden angefasst.
