# `vdi-r2`: die unerklärte Verbesserung, zurückverfolgt (2026-09-12)

Auditor-Nachgang zu `docs/analysis/full-regression-2026-09-12.md`, Abschnitt
„`vdi-r2` — die Verbesserung, im Detail" (ab Zeile 84). Auftrag: die 22 Felder,
die sich zwischen dem 31.-August-Vorstand und dem heutigen Lauf geändert haben,
auf ihre Ursache zurückführen — und prüfen, ob sich zwei Fehler aufgehoben
hatten und einer noch steht.

**Kurzantwort.** Der Vorstand ist vergleichbar. Die Verbesserung stammt aus
drei *committeten* Reparaturen vom 4. und 6. September, nicht aus einer
unbeabsichtigten Nebenwirkung. Es war kein sich aufhebendes Fehlerpaar,
sondern ein einzelner Stub — die Trap-Rückgabe war eine fest verdrahtete `0`.
Aber: **derselbe Vertrag ist bis heute nur zur Hälfte repariert.** Der
Unbekannt-Selektor-Arm derselben `match`-Anweisung gibt weiterhin `0` zurück,
wo EmuTOS `d0` unverändert lässt — `vq_gdos()` meldet auf NewTOS „GDOS
vorhanden". Das ist in QEMU nachgefahren und bestätigt.

---

## 1. Ist der Vorstand vergleichbar? — Ja, in jeder entscheidenden Hinsicht

Alle vier Dumps selbst geparst (Kopfsatz + alle 120 Datensätze, eigener
Python-Leser, nicht `tosdiff`):

| | Vorstand 31. Aug | EmuTOS-Referenz | heute (640) | heute (Full HD) |
|---|---|---|---|---|
| Datei | `tosdump/reference/NEWTOS-VDI-r2.BIN` | `EMUTOS14-VDI-r2.BIN` | `…/vdi-r2/VDI.BIN` | `…/vdi-r2-fhd/VDI.BIN` |
| Format / Satzgröße / Sätze | v2 / 1592 / 120 | v2 / 1592 / 120 | v2 / 1592 / 120 | v2 / 1592 / 120 |
| Tabellen-CRC | `A0706A6D` | `A0706A6D` | `A0706A6D` | `A0706A6D` |
| Dumperbau | `3fed` | `1cd6` | `70ca` | `70ca` |
| DEV_TAB[0,1] | 1919, 1079 | 639, 479 | 639, 479 | **1919, 1079** |
| SIZ_TAB | `5,4,7,13,1,0,40,0,15,11,120,88` | identisch | identisch | identisch |
| Sitzung | `appl_init id 1, handle 2` | `id 0, handle 2` | `id 1, handle 2` | `id 1, handle 2` |

Der profilgleiche Vergleich ist also Vorstand gegen **`vdi-r2-fhd`**; genau den
hat der Tester benutzt. Damit fallen Geräteparameter aus dem Vergleich heraus.

**Der Dumperbau unterscheidet sich (`3fed` → `70ca`) — der VDI-Pfad darin
nicht.** Geprüft, nicht angenommen:
`git diff 3fedabd..HEAD -- tosdump/core tosdump/abi` listet `runner.c` und
`abi/newtos_svc.c` **gar nicht** (unverändert), und die einzige Änderung an
`record.c` im VDI-Teil ist die neu hinzugekommene `compare`-Spalte der
Felddeskriptoren, die für **jedes** VDI-Feld auf `1` steht
(`tosdump/core/record.c:374-383`). Aufnahmeweg (`td_abi_vdi` → `td_run_vdi` →
`td_vdi_record_write`), Vergiftungsmuster und Satzaufbau sind identisch. Die
Baukennung ist der Git-Kurz-Hash des Arbeitsbaums zur Bauzeit, nicht eine
Formatversion.

**Maschine:** Der Vorstand kam von **echter Pi-4-Hardware**
(Commit `229b286`, 2026-08-31, „first NewTOS hardware dump"), der heutige Lauf
aus QEMU `raspi4b`. Das ist der einzige unkontrollierte Unterschied — und er
wird durch die Messung selbst entkräftet: von 120 Datensätzen × ~800 Wörtern
weichen **genau die 22 erklärten Felder** ab, alles andere ist byte-identisch.
Echte Hardware und QEMU stimmen für diese Tabelle also überein, wo der Code
sich nicht geändert hat.

Eigene Nachrechnung des Diffs (voller Satzvergleich über alle sieben
Feldgruppen) reproduziert die Zahl des Testers exakt: **22 von 120 Fällen,
dieselben Fallnummern, dieselben Felder.**

---

## 2. Welche Änderung war es? — Drei Commits, alle mit EmuTOS-Beleg

### 2a) 16× `d0` und 5× die beiden vergifteten Felder → `675e311` (2026-09-04)

`fix(vdi): return d0 from the dispatcher, and two adjudicated defects`.

* **`d0` (16 Fälle, Opcodes 12, 19, 21, 107, 116, 117, 131).**
  Vor dem Commit schrieb der Trap-Handler eine fest verdrahtete Null:
  `675e311^:kernel/src/arch/aarch64/exceptions.rs:365` trug den Kommentar
  *„Neither returns a value in a register, so leave `x0 = 0`"*, `:375` rief
  `vdi::dispatch(...)` auf, das `()` zurückgab, und der Block endete auf ein
  literales `0`. Heute:
  `kernel/src/arch/aarch64/exceptions.rs:377`
  (`115 => i64::from(unsafe { tos_api::vdi::dispatch(args.regs[1], abi) })`)
  und `tos-api/src/vdi/mod.rs:418-421`.
  EmuTOS gibt dort `flip_y` zurück (`references/emutos/vdi/vdi_asm.S:124-125`,
  `moveq #0,d0; move.w _flip_y,d0`), gesetzt an neun Stellen
  (`vdi_control.c:379` `init_wk`, `:631` `vq_extnd`, `vdi_marker.c:49`
  `vsm_height`, `:178` `vqm_attributes`, `vdi_text.c:678`
  `setup_width_height`, `:1020` `vqt_attributes`, `:1058` `vqt_extent`,
  `:1095` `vqt_width`, `:1173` `vqt_fontinfo`), gelöscht bei Eintritt in
  `screen()` (`vdi_main.c:135`).
* **`intout[33]` bei `vqt_name` (4 Fälle).** `vdi_vqt_name` schreibt **34**
  Wörter, nicht 33: `*int_out++ = font_id`, dann die Kopierschleife, die den
  Namen **einschließlich seines NUL** ausgibt, dann `while (i++ <
  FONT_NAME_LEN)` mit `32 − L` Null-Wörtern — `1 + (L+1) + (32−L) = 34`
  (`references/emutos/vdi/vdi_text.c:1128-1133`). Nachgerechnet am Dump:
  `intout[0..15]` = Font-Id + „6x6 system font", `[16]` = NUL, `[17..33]` = 17
  Nullen, `[34]` = `0xDEAD`, `contrl[4] = 33` — auf beiden Maschinen.
* **`intin[0]` bei `v_dspcur` (1 Fall).** `escfn18` schreibt `INTIN[0] = 0`
  in das **Eingabe**-Array des Aufrufers zurück
  (`references/emutos/vdi/vdi_esc.c:274-278`); `escfn19` (`v_rmcur`) tut das
  **nicht** (`:284-287`). Unsere Umsetzung: `tos-api/src/vdi/escape.rs:281-284`.

### 2b) 1× `vqin_mode` → `1eb4e68` (2026-09-06)

`fix(aes): set the VDI string device to SAMPLE on every dispatch`.
Fall 51 ist `vqin_mode(4)`. EmuTOS' `vqin_mode` gibt den rohen internen
Modus zurück (0 = REQUEST, 1 = SAMPLE, `vdi/vdi_input.c:154-176`), und der
AES-Dispatcher setzt das String-Gerät bei jedem Durchlauf auf SAMPLE
(`aes/gemdisp.c:157-177` → `aes/gemgsxif.c:459-463`). Unsere Entsprechung:
`tos-api/src/aes/mod.rs:337` (`chkkbd()` in `dsptch()`), `:386-388`.

### 2c) Bonus: 3× `vq_extnd(1)` → `f76a666` (2026-09-04), nur in `vdi-all` sichtbar

Derselbe Zeitraum, breitere Tabelle. Der 31.-August-Vorstand von `vdi-all`
(`tosdump/reference/NEWTOS-VDI-all.BIN`, ebenfalls Full HD, ebenfalls
`id 1/handle 2`) gegen den heutigen Full-HD-Lauf ergibt **45 von 182**
Abweichungen: 39 davon in `d0` (nachgezählt), 4 `vqt_name intout[33]`,
1 `v_dspcur intin[0]`, 1 `vqin_mode intout[0]` — und bei 3 der 39 (Opcode 102)
zusätzlich **`vq_extnd(1) intout[14]`: 512 → 1024**.
Das ist `MAX_VERTICES`, korrigiert in `f76a666`
(`tos-api/src/vdi/primitives.rs:577`). EmuTOS-Referenz: 1024. Auch das ist
kein Zufallsfund, sondern eine benannte Reparatur.

**Nichts anderes hat sich in diesem Zeitraum in 182 + 120 Datensätzen
bewegt.** Der VDI-Pfad war zwischen dem 31. August und heute abgesehen von
diesen drei Commits stabil.

### Was im Bericht des Testers korrigiert gehört

`docs/analysis/full-regression-2026-09-12.md:33-35` vermutet „vermutlich
Nebenwirkung einer der vielen anderen **uncommitteten** Änderungen im Baum".
Das ist falsch: alle drei Ursachen sind committet und liegen im `git log` des
VDI-Pfades zwischen Vorstand und Messung. Der Hinweis stand sogar schon in der
Commit-Nachricht, die den Vorstand aufgenommen hat (`229b286`: *„d0/flip_y is
never produced"*).

---

## 3. War vorher wirklich etwas kaputt? — Ja, ein lupenreiner Stub

Nicht „anders", sondern regelwidrig nach der No-Stub-Regel: eine Funktion, die
einen fest verdrahteten Wert zurückgibt. Der Fingerabdruck ist im Vorstand
messbar — **alle 120 Datensätze tragen `d0 = 0`**, kein einziger etwas
anderes (`ret` als voller Long ebenfalls überall 0). Ein solcher Wertebereich
entsteht nur durch eine Konstante; ein echter `flip_y`-Pfad hätte auch vorher
in 16 Fällen 1 liefern müssen. Die EmuTOS-Referenz liefert in genau diesen
16 Fällen 1.

Der Defekt war im Repo bereits benannt (`docs/api/vdi/_working-list.md:139`,
D-080) und in `docs/api/vdi/_dispatcher.md:33` seit der Spezifikations-
extraktion als Vertrag geführt (*„Return `d0` = `flip_y` (0/1)"*). Die
Spezifikation war hier also **richtig**, der Code falsch — kein Researcher-
Befund.

---

## 4. Steht noch ein Fehler? — Ja, in derselben `match`-Anweisung

### F-1 — GEM-Trap: unbekannter Selektor gibt `0` zurück statt `x0` unverändert zu lassen · **CONFIRMED · Bug**

* **EmuTOS-Quelle:** `references/emutos/vdi/vdi_asm.S:35-52`. `_vditrap`
  prüft `d0 == 0x73` (VDI) und `d0 == 0xFFFF` (Adressabfrage); für **alles
  andere** steht dort ein nacktes `rte` mit dem ausdrücklichen Kommentar
  *„Just like Atari TOS, we leave d0 unchanged, indicating the absence of
  GDOS if this is called with -2 in the low-order word of d0."*
* **Extrahierte Spezifikation:** `docs/api/vdi/_dispatcher.md:55-61` führt
  genau diese Tabelle (`-1` → Adresse von `_GSX_ENTRY`, `-2` → `d0`
  unverändert, sonst `rte`). Die Spezifikation ist korrekt und vollständig.
* **NewTOS-Code:** `kernel/src/arch/aarch64/exceptions.rs:399` —
  `_ => 0, // unknown selector → no-op (GEM trap is trusted)`.
* **Nachgefahren:** temporäre Kernel-Sonde, `svc #2` mit den Selektoren `-2`,
  `-1` und `999`, QEMU `raspi4b`, Kernel `42e5b8ee…`:

  ```
  [AUDIT] gem trap: sel -2 -> x0=0x0000000000000000,
                    sel -1 -> x0=0x0000000000000000,
                    sel 999 -> x0=0x0000000000000000
  ```

  (`measurements/measurement-2026-09-12-audit-flipy/GEMTRAP-SELECTOR-PROBE.txt`; die Sonde
  wurde danach von Hand entfernt, `kernel/src/lib.rs` ist byte-identisch zum
  Ausgangsstand.)
* **Fehlerszenario, mit Aufrufer:** `vq_gdos()` ist genau der `-2`-Sondierer:
  `sdk/gemlib/overlay/_gc_vq_gdos.c:30-39` gibt `(short)x0 != -2` zurück. Mit
  `x0 = 0` ist das **1 = „GDOS vorhanden"**. Auf einem System ohne GDOS ist
  das die falsche Antwort durch die dokumentierte API. `vq_vgdos()`
  (`_gc_vq_vgdos.c`) liefert entsprechend `0` statt `-2`.
  **Erreichbar, nicht theoretisch:** TeraDesk ruft es zweimal auf —
  `sdk/apps/teradesk/library/xdialog/xdialog.c:2355`
  (`if (load_fonts && vq_gdos()) *nfonts = vst_load_fonts(...)`) und
  `sdk/apps/teradesk/main.c:1982` (`if (vq_gdos() != 0)
  vst_unload_fonts(...)`); QEDs cflib zieht `vq_vgdos` über `appinit.o`
  (`sdk/apps/qed/build/why.txt:102`).
* **Heutige Auswirkung, ehrlich benannt:** begrenzt. Beide TeraDesk-Aufrufe
  landen bei `vst_load_fonts`/`vst_unload_fonts`, und unsere Umsetzung
  (`tos-api/src/vdi/attributes.rs`, `INTOUT[0] = 0`) verhält sich dort wie
  EmuTOS' `vdi_vst_load_fonts` — es kommen 0 Zusatzfonts zurück, nichts
  stürzt ab. Die **Antwort** bleibt trotzdem falsch, und der nächste
  konforme Aufrufer, der aus „GDOS vorhanden" eine echte GDOS-Funktion
  ableitet, bekommt sie nicht.
* **Keine HW-Abwesenheits-Ausnahme:** GDOS ist Software, kein Atari-Chip.
  Der Wert ist über die dokumentierte API beobachtbar und in
  `docs/COMPATIBILITY.md` **nicht** als zugelassene Abweichung geführt (dort
  kommt „GDOS" überhaupt nicht vor).
* **Einordnung zur Ausgangsfrage:** Das ist **kein** sich aufhebendes
  Fehlerpaar. Es ist eine *halb ausgeführte* Reparatur desselben Vertrags:
  `675e311` hat zwei der drei Arme derselben `match`-Anweisung richtig
  gemacht (VDI → `flip_y`, AES → hart 0, beides gegen EmuTOS belegt) und den
  dritten stehen lassen.

### F-2 — Selektor `-1` (Adressabfrage) und ein veralteter Kommentar · **CONFIRMED · Doku-Lücke, kein Bug**

Selektor `-1` gibt bei uns `0` statt einer „jsr-baren" Dispatcher-Adresse
zurück. Da NewTOS kein m68k-anspringbares `_GSX_ENTRY` hat, ist `0` die
ehrliche Antwort — aber sie ist nirgends als Abweichung festgehalten.
Getrennt davon behauptet `sdk/gemlib/overlay/_gc_vdi.c:19` weiterhin *„The GEM
trap returns nothing in a register (handler leaves x0 = 0)"*; das ist seit
`675e311` für den VDI-Selektor falsch. Beides gehört in
`docs/COMPATIBILITY.md` bzw. in den Kommentar korrigiert. Die
Trap-Primitiven selbst sind in Ordnung: alle vier Aufrufstellen
(`kernel/src/vdi_client.rs:76`, `kernel/src/selftest_vdi.rs:20`,
`sdk/gemlib/overlay/_gc_vdi.c:35`, `tosdump/abi/newtos_svc.c`) deklarieren
`x0` als `inout`/`"+r"` und verwerfen das Ergebnis — die neue Rückgabe
verdirbt keinem Aufrufer ein Register.

---

## 5. Was ich gefahren habe

### 5a) Gegenprüfung der fremden Zahlen (alles nachgerechnet, nichts übernommen)

* Alle vier `vdi-r2`-Dumps selbst geparst: Kopfsatz, CRC, Satzzahl, Profil.
* Voller Feldvergleich Vorstand ↔ heute (Full HD): **22 von 120**, dieselben
  Fälle und Felder wie im Testerbericht.
* `d0` über **alle vier** VDI-Falltabellen gegen die EmuTOS-Referenzen:
  `vdi-r1` (24), `vdi-r2` (120), `vdi-r3` (38), `vdi-all` (182) —
  **0 Abweichungen**, einschließlich der Opcodes 36, 38, 100, 102, die
  `vdi-r2` gar nicht enthält.
* `d0` in der Rastertabelle (59 Fälle, Opcodes 109/110/121): EmuTOS 0,
  NewTOS 0, 0 Abweichungen.
* `d0` in allen vier AES-Tabellen (95 + 78 + 15 + 39 = 227 Fälle): EmuTOS
  überall 0, NewTOS überall 0 — die Behauptung des Commits, der AES gebe hart
  0 zurück (`aes/gemsuper.c:454-466`), hält der Messung stand.
* `vdi-all`-Vorstand vom 31. August gegen heute, um denselben Zeitraum auf der
  breitesten Tabelle zu prüfen: 45 von 182, restlos aus denselben drei
  Commits erklärbar.

### 5b) Eigene, gegnerische Messung auf beiden Maschinen

Neue Tabelle `measurements/measurement-2026-09-12-audit-flipy/vdi-audit-flipy.tsv`
(36 Fälle, CRC `F8B45495`), gebaut, um genau die Arme zu treffen, die die
eingefrorenen Tabellen **nicht** erreichen: jeden Fehler- und Rückfallpfad der
Handler, die `flip_y` setzen, plus die drei von `675e311`/`1eb4e68`
geänderten Felder.

* EmuTOS 1.4 auf Hatari (TT, ttmed, `DUMPVDI.PRG` build `19b7`) — 36/36 gelaufen.
* NewTOS auf QEMU `raspi4b` (Kernel `42e5b8ee…`, `DUMPVDI.NAP` build `70ca`,
  reines Python-FAT16-Abbild, `#Z`-Autostart) — 36/36 gelaufen.

Ergebnis: **`d0` stimmt in allen 36 Fällen überein.** Im vollen Satzvergleich
weichen **2 von 36** ab, beide in der bereits dokumentierten
`vq_extnd`-Konstantenfamilie (Farbzahl 16↔256, Planes 4↔16). Getrieben und
byte-gleich bestätigt wurden unter anderem:

| Pfad | Beobachtung (EmuTOS = NewTOS) |
|---|---|
| `vqt_width` mit Zeichen 300 / −1 (Fehlerarm) | `INTOUT[0] = −1`, `PTSOUT[0]` unberührt (`0xDEAD`), `PTSOUT[2]/[4] = 0`, `d0 = 1` |
| `vst_font` mit Face −1 / 0 (Systemfont-Rückfall) | `d0 = 1` |
| `vst_font` nach `vst_point` (der `pts_mode`-Arm) | `d0 = 1` |
| `vst_height` 0 / −1, `vst_point` 0 / −3, `vsm_height` −5 / 32767 | `d0 = 1` |
| `vqt_width` auf skaliertem Font (`act_siz`-Arm) | Breite 8, `d0 = 1` |
| `vqt_name` Element 9 / −1 / 0 (Rückfall auf `fon6x6`) | 34 Wörter, `intout[33] = 0`, `intout[34] = 0xDEAD`, `contrl[4] = 33`, `d0 = 0` |
| `vqin_mode` Gerät 0 / 5 (außerhalb 1..4) | `INTOUT[0]` **unberührt** (`0xDEAD`), `contrl[4] = 1` |
| `vqin_mode(4)` → `vsin_mode(4,1)` → `vqin_mode(4)` → `vsin_mode(4,2)` → `vqin_mode(4)` | 1 → 0 → 1, identischer Zustandsautomat |
| `vsin_mode` Gerät 9 (außerhalb) | `INTOUT[0] = 2` echot, speichert nichts |
| `v_dspcur` / `v_rmcur` | `INTIN[0] = 0` nur bei sub 18, bei sub 19 unberührt |
| Escape sub 20 (jenseits `ldri_escape`) | nichts geschrieben, `contrl[2] = contrl[4] = 0` |

### 5c) Nachgefahrene Sonde

Der GEM-Trap mit den Selektoren `-2`, `-1`, `999` (siehe F-1).

---

## 6. Was unentschieden bleibt

* **`flip_y` bei Opcode 1 (`v_opnwk`)** und auf dem `v_opnvwk`-Pfad **ohne
  freien Slot**: nicht messbar. Opcode 1/100 zerstören die Werkstatt, durch
  die der Lauf misst, und der Slot-Erschöpfungsfall bräuchte einen Dumper,
  der die Handle-Tabelle vollmacht. Aus der Quelle gelesen — EmuTOS'
  `vdi_v_opnvwk` verlässt sich bei erschöpfter Tabelle über
  `CONTRL[6] = 0; return;` **vor** `init_wk` (`vdi_control.c:425-427`), also
  `flip_y = 0`; unser `tos-api/src/vdi/mod.rs:287-293` gibt in genau diesem
  Fall `FLIP_Y_COORDINATES` zurück. Gelesen, nicht bewiesen.
* **Die EmuTOS-Seite von F-1** (`d0` bleibt bei `-2` unverändert) ist aus der
  gepinnten Quelle und der extrahierten Spezifikation belegt, **nicht**
  gemessen: dafür bräuchte es ein m68k-Sondierprogramm auf dem TT-Abbild, das
  hier nicht gebaut wurde. Die NewTOS-Seite ist gemessen.
* **Echte Hardware:** Alles Heutige ist QEMU. Der 31.-August-Vorstand kam von
  echter Pi-4-Hardware und stimmt außerhalb der erklärten Felder byte-genau mit
  QEMU überein — das stützt die Übertragbarkeit, ersetzt aber keinen Lauf auf
  dem Gerät. F-1 sollte auf dem Pi gegengeprüft werden, wenn TeraDesk das
  nächste Mal dort läuft.
* **Gegenprobe durch Rückbau:** Ein Kernel vom Stand `675e311^` wurde nicht
  gebaut. Für die `d0`-Klasse ist das entbehrlich (eine literale `0` im
  Quelltext plus 120 Datensätze mit ausschließlich `d0 = 0`), für Fall 51
  (`vqin_mode`) beruht die Zuordnung auf Commit-Inhalt plus dem heute
  gemessenen Zustandsautomaten, nicht auf einem Rückbau-Lauf.

---

## 7. Außerhalb des Auftrags beobachtet (nicht verfolgt)

* `docs/api/vdi/_working-list.md:139` führt D-080 weiterhin als „open question
  logged for whether NewTOS's selector-based ABI needs an equivalent signal at
  all" — seit `675e311` beantwortet; veraltete Protokollzeile.
* Die vom Dumper erzeugten Dateien `VDI.BIN`/`VDI.LOG` erscheinen im
  FAT16-Verzeichnis mit Attributbyte `0x00` statt `0x20` (Archiv). Nicht
  geprüft, was TOS hier tut.

---

## 8. Belege

* `measurements/measurement-2026-09-12-audit-flipy/` — neue Falltabelle, beide Rohdumps,
  Diff, Boot-Log, Sondenausgabe, `RUNINFO.TXT`.
* Unverändert geblieben: `tosdump/cases/`, `tosdump/reference/`, alle
  bestehenden `measurement-*`-Verzeichnisse, sämtlicher Produktcode.
  `kernel/src/lib.rs` ist nach Entfernen der Sonde byte-identisch
  (SHA-256 `be478726…`), `kernel8.img` aus der bereinigten Quelle neu gebaut
  und wieder bitgleich `42e5b8ee…`.
