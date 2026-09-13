# A54 — STATUS: DONE

`wind_get(WF_SCREEN)` meldet keinen Erfolg mehr für einen Puffer, den es nicht
gibt. Bearbeitet 2026-09-12, Branch `desk-rebuild`, **nicht committet**.

## Der Fehler, beidseitig

| | Datei:Zeile |
|---|---|
| EmuTOS (Soll) | `references/emutos/aes/gemwmlib.c:1563-1565` → `gsx_mret`, `references/emutos/aes/gemgsxif.c:125-129`; Puffer einmalig von `gsx_malloc` `:108-114`, gerufen aus `sh_main` `references/emutos/aes/gemshlib.c:236` |
| NewTOS (vorher) | `tos-api/src/aes/window.rs:2672-2678` — `for i in 1..=4 { pb.set_intout(i, 0); } pb.set_ret(1);` |
| NewTOS (nachher) | `tos-api/src/aes/window.rs` (`wind_get`, WF_SCREEN-Zweig) — `pb.set_ret(0); return;`, kein geschriebenes Wort |

Gepinnte Quelle bestätigt: `git -C references/emutos log -1` →
`1a7484aba1bfba23e2c0899e243b6d8177fe41df`, *Update GitHub Actions to use Node
24*, 2026-04-14. `MENU_BUFFER_SIZE` und die Größenformel stehen in
`gemgsxif.c:44-72,98-103`, **nicht** im `TARGET_192`-Block von `include/config.h`
— die Falle des Auftrags greift hier nicht.

Gemessen auf dem 1.4-ROM (Referenz-TT über Hatari, `measurement-2026-09-12-aes-r4/`):
Adresse `0x01000218`, Länge `40960` = `640 · 8 · 16 · 4 / 8`, `ret = 1`.

## Welcher der zwei Ausgänge gilt — und woraus

**Ausgang 2: wir haben keinen Puffer.** Belegt aus dem Code, nicht aus der
Absicht:

1. **Kein `gsx_malloc`-Gegenstück.** Nichts im Baum belegt beim AES-Start einen
   Menü-/Alarmpuffer. Gesucht wurde nach `bb_save`, `menu_buffer`, `save_under`
   über `tos-api/src` und `kernel/src`.
2. **Der Alarm nimmt einen frischen Puffer je Alarm.**
   `tos-api/src/aes/form.rs`, `BbSave::capture` →
   `let mut buf = alloc::vec![0u32; words];`; der `BbSave`-Wert wird nach
   `restore` fallengelassen, die Belegung also freigegeben. Keine stabile
   Adresse, keine stabile Länge.
3. **Das Menü hält zwei Ablagen, keine davon fest.**
   `tos-api/src/aes/menu.rs`: `static MBACK` und `static SMBACK`, je ein
   `UnsafeCell<Vec<Pel>>`; `save_under` macht `store.clear(); store.reserve(need)`
   je geöffnetem Dropdown. Beim Start ist die Kapazität **0** (`Vec::new()`), die
   Adresse wandert beim Wachsen, und das Elementformat (`Pel`, 32 Bit) ist nicht
   einmal das `bb_save`-Wireformat.
4. **Und es gibt keine Adresse, die wir herausgeben *könnten*.** Diese Puffer
   liegen im Kernel-Heap, dessen Abbildungen `AP_RW_EL1` tragen — „R/W at EL1,
   **no EL0 access**“ (`kernel/src/arch/aarch64/mmu.rs:82`, gesetzt in
   `:178-179,283-284`). Eine EL0-Anwendung, die durch einen WF_SCREEN-Zeiger
   schreibt — und genau dafür ist das Feld da —, würde beim ersten Store
   abstürzen. Ein echter persistenter Kratzpuffer ist damit eine
   **Speicherpool- und Abbildungsfrage**, keine `wind_get`-Frage, und liegt
   ausserhalb dieses Tickets.

Deshalb: **FALSE, und nichts geschrieben** — `wm_get`s eigene Redensart für ein
Feld, das diese AES nicht beantwortet (`default: return FALSE`,
`aes/gemwmlib.c:1590-1591`), dieselbe Reparatur, die AUD-P5-014 für `WF_KIND`
bekommen hat. Ein konformer Aufrufer prüft den Rückgabewert, bevor er die
Adresse benutzt; FALSE ist die einzige Antwort, die nicht als „hier ist ein
Puffer, er liegt bei 0“ gelesen werden kann.

Die Abwesenheit ist benannt in **`docs/COMPATIBILITY.md`** („Sanctioned
deviations, one by one“, neuer Eintrag mit Ausstiegskriterium) und in
**`docs/STATUS.md`** (datierter *Recent-Decisions*-Eintrag, im selben
Arbeitsgang eingefügt, **ohne** `git stash`).

## Was mit AUD-P5-012 geschehen ist

Beides wurde richtiggestellt, nicht keines von beidem.

* **Der Code-Kommentar** in `tos-api/src/aes/window.rs` ist ersetzt. Die Zeile
  „Assessed deliberate deviation (AUD-P5-012)“ mit der Begründung „report a
  null/zero-length buffer truthfully (ret=1)“ ist weg; der neue Kommentar
  benennt EmuTOS' Soll, die vier Code-Belege für das Fehlen des Puffers, die
  EL0-Abbildung als harte Schranke und verweist auf den
  `docs/COMPATIBILITY.md`-Eintrag.
* **Die Bewertung** in `docs/analysis/aes-audit/p5-window-graf.md` (AUD-P5-012)
  trägt jetzt einen Absatz *„BEWERTUNG KORRIGIERT, TEILWEISE ERLEDIGT
  2026-09-12 (A54)“*: die Einstufung als bewusste Abweichung ist
  **zurückgenommen** (eine Begründung ist keine Unbeobachtbarkeit, und der Wert
  wurde gemessen), erledigt ist die Hälfte ohne Architekturentscheidung, offen
  bleibt der Puffer selbst — mit Angabe, woran er hängt.

## Bestehende Tests, die angefasst wurden

**Genau einer**, und er wird nicht gelöscht, nicht `#[ignore]`d und nicht
umgeschrieben:

* `tos-api/tests/aes_reference/cases_r1.rs` Fall **166**
  — `wind_get(0, WF_SCREEN)`, `OUT_LEN 1`, also nur `int_out[0]`. Der TT hat
  dort `1` aufgezeichnet. Der Fall war grün,
  **weil NewTOS aus dem falschen Grund dieselbe 1 lieferte**; mit der ehrlichen
  Absage wird er rot. Die generierte Datei ist unverändert geblieben („do not
  edit them by hand“).

Gehalten wird er stattdessen über eine **neue, namentlich geführte Liste**
`SANCTIONED` in `tos-api/tests/aes_reference/harness.rs`, getrennt von `DELTAS`
(deren Doku ausdrücklich „exactly one subject: `WF_OWNER`“ sagt und weiterhin
stimmt). Sie unterliegt denselben drei Regeln, damit sie kein stiller Weg wird,
Rot grün zu machen:

* der aufgezeichnete Wert wird **geprüft** — ein Eintrag kann keine Messung
  beschreiben, die der Dump nicht enthält;
* die Antwort wird **ersetzt**, nicht nachträglich gefiltert — ein Rückfall auf
  EmuTOS' `1` färbt den Fall wieder rot (**gegengeprüft**: mit einem
  Kratzversuch `set_ret(1)` fiel Fall 166 mit
  `int_out[0] NewTOS = 0x0001 (1)   expected = 0x0000 (0)` durch; die Datei
  wurde von Hand zurückgeändert, kein `git`);
* ein nie konsultierter Eintrag ist ein Fehler (`Oracle::finish`), und ein
  Eintrag, dessen „Abweichung“ zufällig dem Referenzwert gleicht, ebenfalls.

Die Fehlermeldung nennt so einen Wort als `expected`, nie als `reference`, und
das Modul-Doku-Bekenntnis („nothing is skipped silently, no case is
`#[ignore]`d“) bleibt wörtlich wahr.

**Das ist die eine Ermessensentscheidung in diesem Änderungssatz** und liegt in
einer Datei, die sich in einem Stück zurückdrehen lässt. Siehe „Offen“.

## Neue Tests

`tos-api/tests/aes_window_screen_buffer.rs` (neue Datei, vorher nicht
vorhanden), drei Tests:

* `wf_screen_refuses_on_every_handle_instead_of_reporting_a_zero_buffer` —
  Handle 0, Handle −1 und ein wirklich offenes Fenster, also genau die drei
  Handles der Fälle 331/332/439;
* `wf_screen_writes_no_address_and_no_length` — `int_out[1..4]` bleiben das
  genullte Dispatcher-Lokal, und hinter `OUT_LEN` wird nichts kopiert;
* `the_other_two_handle_free_fields_still_answer` — `WF_TOP` und `WF_DCOLOR`
  antworten auf einem ungültigen Handle weiter mit TRUE, die Absage ist also
  nicht versehentlich in die Handle-Vorprüfung gerutscht.

## Gegenmessung (ohne Hardware)

Alles in `measurement-2026-09-12-a54/` (neues Verzeichnis, `RUNINFO.TXT` mit
allen Prüfsummen). QEMU `raspi4b`, headless, 640×480, FAT16-Karte aus
`tools/make-sd-image.sh`, `#Z`-Autostart, kein Klick. Dumper
`DUMPAES.NAP` build `70ca`, SHA `5889e9bb…` — **byte-identisch** zu dem der
r4-Messung, der Kernel ist also die einzige Variable. Kernel `kernel8.img`
SHA `5f4355b5…`. Die `AES.TSV` auf der Karte wurde nach dem Kopieren gegen
`tosdump/cases/aes-r4-subfunctions.tsv` geprüft (SHA `e88fa2d3…`, CRC
`227422B2` im Lauf-Log bestätigt).

**r4 gegen `EMUTOS14-AES-r4.BIN`** — Vergleich der Mengen abweichender Felder je
Fall, vorher (`measurement-2026-09-12-aes-r4/newtos-qemu-raspi4b/NEWTOS-AES-r4.BIN`)
gegen nachher:

```
Fälle, deren Menge abweichender Felder sich geändert hat : 3
  Fall 331  op 104   neu abweichend: int_out[0]
  Fall 332  op 104   neu abweichend: int_out[0]
  Fall 439  op 104   neu abweichend: int_out[0]
Fälle unverändert                                        : 178
Fälle mit Abweichung ausserhalb global[]  : 64 vorher, 64 nachher, gleiche IDs
Fälle nur mit global[]-Abweichung         : 117 vorher, 117 nachher, gleiche IDs
```

Die drei Fälle nachher, je identisch:

```
EmuTOS 1.4 (TT) int_out[0..4] = 1, 0x0100, 0x0218, 0x0000, 0xA000
NewTOS (A54)    int_out[0..4] = 0, 0,      0,      0,      0
```

Also **umgeschlagen auf die ehrliche Absage**, wie es der Auftrag als zweiten
zulässigen Ausgang vorsieht. Zusammenfallen war nie erreichbar: die Tabelle
selbst erklärt `WF_SCREEN` in ihrem Kopf (Zeile 250) als **Geräteparameter**
(„RECORDED, not compared away“); worüber die Unterfunktionsfrage entscheidet,
ist `RET_CODE 1 gegen 0`, und genau dort steht jetzt eine wahre Aussage statt
einer falschen. Die vier Wörter dahinter sind das genullte Dispatcher-Lokal
(`aes/gemsuper.c:432`), keine Antwort — der Zweig schreibt nichts mehr.

**r3-Regressionsprobe** gegen `tosdump/reference/EMUTOS14-AES-r3.BIN`
(`aes-r3-wind-state.tsv`, 39 Fälle, CRC `979FAC60`):

```
Fälle mit int_out-Abweichung : 261, 280, 400, 481, 494, 574, 588
Fälle mit addr_out-Abweichung: keine
```

Unverändert der Endzustand, den A33/A34 hinterlassen haben — die sieben
Dispatcher-Stapelreste (`aes/gemsuper.c:430-432,444-445`), für die es kein Soll
gibt. Keine Rechtecklistengruppe ist aufgebrochen.

Nach jedem Lauf `pgrep -f 'qemu-system-aarch64|Hatari'` → nichts.

## Bau und Prüfung

```
cargo run -p xtask -- test        489 / 0 / 1   ->   492 / 0 / 1   (die drei neuen)
cargo run -p xtask -- build --board rpi4        Finished, objcopy -> kernel8.img
cargo run -p xtask -- build --board qemu-virt   Finished
```

Beide Boards je frisch gebaut (`touch tos-api/src/aes/window.rs` davor, wegen
des geteilten Zielverzeichnisses). Warnungen unverändert: `tos-api` 64,
`kernel` 94 auf beiden Boards.

Clippy-Delta **0**, gemessen mit einem Kratzversuch vor/nach demselben Baum:

```
cargo clippy -p kernel --target aarch64-unknown-none-softfloat \
             --no-default-features --features board-rpi4        208 -> 208
                                            --features board-qemu-virt     208
cargo clippy -p tos-api --tests                                  93 ->  93
```

Kein Treffer zeigt auf den geänderten Bereich, die neue Testdatei oder die
Harness-Ergänzung; die Treffer in `window.rs` sind die vorbestehende
`Aespb`-`private_interfaces`-Familie und `:1927` (`value assigned to top is
never read`, fremder Stand).

## Nicht angefasst

`tosdump/cases/`, `tosdump/reference/`, alle `measurement-*` ausser dem neuen
eigenen Verzeichnis. Kein `git checkout`/`clean`/`stash`/`restore`/`reset`, kein
Commit. Keine Hardware, nichts geflasht, nichts ausgeworfen. Die fremden
Stränge in `tos-api/src/aes/window.rs` (A22/A33/A34), `resource.rs`
(B1/A37/A38), `bdos/fs/file.rs` (Q07), `exceptions.rs` (V16), `tools/mkrsc.py`
(B2/A37), `kernel/src/dbg_qedtrace.rs` und die fremde Einzeiländerung in
`tos-api/tests/vdi_reference/harness.rs` sind unberührt.

## Offen

1. **Der Puffer selbst.** Ein echter persistenter AES-Kratzpuffer in
   EL0-erreichbarem Speicher, benutzt vom Alarm- *und* vom Menü-Save-Under,
   würde `wind_get(WF_SCREEN)` wieder zu einer TRUE-Antwort machen. Das hängt an
   zwei Entscheidungen, die nicht meine sind: **aus welchem Pool** und **wie in
   EL0 abgebildet**. Eine wörtliche Übernahme von EmuTOS' Formel wäre dabei
   keine gute Idee: `gl_mlen = MENU_BUFFER_SIZE · gl_wchar · gl_hchar ·
   gl_nplanes / 8` ergäbe bei 1920×1080 einen Puffer, in den unsere Dropdowns
   nicht passen, und damit EmuTOS' „adjust height to fit buffer: this will leave
   droppings“ (`aes/gemgsxif.c:336-343`) als sichtbaren Rückschritt gegenüber
   der heutigen ungedeckelten Ablage.
2. **Die `SANCTIONED`-Liste ist zur Durchsicht vorgelegt.** Sie ist die einzige
   Ermessensentscheidung hier: der Auftrag verlangt zugleich die ehrliche
   Absage *und* eine grüne Testreihe, und Fall 166 der r1-Tabelle ist der Punkt,
   an dem sich beides trifft. Die Alternative wäre, Fall 166 rot stehen zu
   lassen. In `DELTAS` gehört er nicht — das ist ausdrücklich die Liste der
   EmuTOS-**Versions**deltas.
3. **Nebenbefund, nicht angefasst:** `docs/STATUS.md` führt in der
   Phase-13-Zeile `#51 bb_save` weiterhin als zurückgestellt. `bb_save` /
   `bb_restore` gibt es längst (`tos-api/src/vdi/raster.rs:545-580`) und
   `form.rs` benutzt sie; fehlend ist allein die *Persistenz* des Puffers. Im
   Recent-Decisions-Eintrag benannt, die Zeile selbst nicht bearbeitet (fremde
   uncommittete Nachbararbeit).
4. **Gehört zu A53, nicht hierher:** `docs/api/aes/_full-function-inventory.md`
   Zeilen 443 und 878 beschreiben `WF_SCREEN` noch als „ja, aber 0/0“ /
   „bewusste Abweichung, gelesen“. Die Auswertung weist diese Korrektur
   ausdrücklich A53 zu; unberührt gelassen.
