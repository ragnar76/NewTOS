# M07 — STATUS: DONE (2026-09-13)

Der Riegel entscheidet je **Parameter**. Beide Richtungen sind belegt: die zwei
zu Unrecht gesperrten Arme laufen auf beiden Maschinen und stimmen dort in
jedem gemessenen Wort überein; die vier gefährlichen Aufrufe werden ohne
Pflanzung namentlich abgewiesen und laufen mit Pflanzung sauber. Keine
bestehende Tabelle liest sich anders, keine CRC hat sich bewegt, kein
bestehender Dump hat ein Byte verloren.

## Wo der Riegel jetzt entscheidet — und warum dort

**Im Lader**, restlos. M06s Naht hat getragen, und der M06-Bericht hatte recht:
die beiden harmlosen Arme brauchen die Maschinenphase nicht. Nachgeprüft statt
übernommen — jede Größe, die der neue Riegel liest, ist eine **Spalte der
Zeile**: die Flags, der Timerzähler, die Fehlernummer, das Zeichen, der
Objektindex, die Pflanzungen. Keine davon ist eine gemessene Zahl, also braucht
keine eine Maschine, und eine Tabelle bleibt auf dem Host prüfbar, bevor sie
irgendwohin getragen wird.

Umgesetzt als zweite, zeilenweise Frage neben dem Runde-2-Fensterriegel, in
derselben Funktion (`aes_line_parameter_refusal`, `tosdump/core/cases.c`), für
Preludes wie für Fälle.

| Opcode | was der Parameter entscheidet |
|---|---|
| 25 `evnt_multi` | `MU_TIMER` mit `MT_LOCOUNT`/`MT_HICOUNT` = 0 ist der einzige Arm, der vor `mwait` zurückkehrt (`gemevlib.c:216-220` gegen `:232-262`). `MU_MESAG` verlangt zusätzlich einen bepflanzten `MME_PBUFF`, weil der Quick-Check `ev_mesag` sechzehn Byte durch `addr_in[0]` schreibt (`:221-229`). Maske 0 ist der **schärfste** Fall der Abweisung, nicht die Ausnahme: `mwait(0)` prüft `!(0 & p_evflg)`, setzt `WAITIN` und ruft `dsptch()` (`gemasync.c:117-125`). |
| 53 `form_error` | `n > 63` ist der einzige Arm, der vor `fm_show` zurückkehrt (`gemfmlib.c:440-441`). |
| 55 `form_keybd` | Der Baum muss bepflanzt sein **und** `FM_ICHAR` darf keine Navigationstaste sein. Der Arm wird vom **Zeichen** gewählt: `direction` bleibt −1 für alles außer RETURN/ENTER/ARROW_UP/TAB/ARROW_DOWN, und dann wird der Baum gar nicht angefasst (`:167-190`). Mit Navigationstaste läuft `find_obj` als OBJECT-Feld über den Puffer, begrenzt nur von `LASTOB` (`:135-148`) — ein bepflanzter Puffer ist eine Buchstabenkette, **kein Baum**, also macht die Pflanzung diesen Arm nicht sicher. |
| 41 `objc_delete` | `ROOT` ist das einzige Objekt, das vor dem ersten Baumzugriff zurückkehrt (`gemoblib.c:1154-1155`). |
| 80 `scrp_read` | `strcpy(pscrap, D.g_scrap)` **schreibt** bis zu `LEN_ZPATH` = 114 Byte durch `addr_in[0]` (`gemsclib.c:40-45`) → Pflanzung Pflicht. |
| 81 `scrp_write` | `strcpy(D.g_scrap, pscrap)` **liest** eine Zeichenkette durch `addr_in[0]` (`:57-62`) → Pflanzung Pflicht. |

Die Formregel, die das total macht: wo der Riegel ein `int_in`-WORD liest, muss
der Fall es nennen **und** `IN_LEN` darüber hinaus deklarieren. `xif`s `int_in`
ist ein **lokales** Feld, in das nur `min(IN_LEN, I_SIZE)` Wörter kopiert werden
(`gemsuper.c:428-438`) — ein WORD außerhalb `IN_LEN` ist nicht „der vorige
Aufruf", sondern uninitialisierter Stapel.

**Vereinigung, nie Schnitt.** Wo die beiden AES uneins sind, ob ein Parameter
tödlich ist, beweist ein Versuch auf einer Maschine nichts über die andere. Der
Zeichentest von `form_keybd` deckt deshalb EmuTOS' Vergleich des ganzen
Tastenworts (`scancode.h:22-32`) **und** NewTOS' byteweisen ab.

## `objc_delete(-1)` — der gelebte Fall

Ja, jetzt gefangen, im Lader, mit Grund und Zeilennummer:

> `ob_delete` returns before touching the tree only for `OB_DELOB == ROOT`
> (gemoblib.c:1154-1155); any other object reads `tree[obj].ob_next` and writes
> through `get_par` (:1157-1188), and a negative one reads BELOW the tree base
> — which NewTOS answers 0 for and the TT does not, so testing on one machine
> proves nothing about the other.

`objc_delete` war nicht in der Dreierliste des Tickets, aber die Abnahme fragt
ausdrücklich danach; es ist der einzige Zusatz über die genannten fünf hinaus.
Die zwei `objc_delete(ROOT)`-Fälle von `aes-r4-subfunctions.tsv` (Zeilen 555,
556) sind genau der erlaubte Arm und laden unverändert.

## Die beiden Nachweisläufe

**Richtung 1 — die zu Unrecht Gesperrten.**
`tosdump/cases/aes-m07-parameter.tsv`, 20 Fälle (12 × `evnt_multi`, 8 ×
`form_error`), CRC `0F46665B`, einmalig geschrieben. Auf **beiden** Maschinen
gefahren (`measurements/measurement-2026-09-13-m07-parameter/`): je „all 20 cases ran",
`--validate` accepted, kein Hänger, keine Panik, kein Fault.

**`int_out[0]` stimmt in allen zwanzig Fällen überein.** Die Unterschiede sind
die sechs `AESGLOBAL`-Wörter, die in jedem Fall jeder Runde abweichen, plus die
Mausposition in den zwölf `evnt_multi`-Fällen (TT 319,239 gegen NewTOS 320,240 —
die Mitte eines 640×480-Schirms, um eins versetzt). Wortgleich sind unter
anderem: der Quick-Check-Timer allein (32), der Button-Quick-Check, der
mitfeuert, weil `downorup(0,0,0)` konstruktionsbedingt wahr ist (34), die
feuernde `MU_M1`-Rechteckprobe (36), die Maskierung von Bits außerhalb
`MU_TOSVALID` (32), `OUT_LEN 1` und `OUT_LEN 0` (das Gift bleibt stehen), und
`form_error` = 0 über `n` = 64, 65, 100, 1000, 32767.

**Richtung 2 — die Gefährlichen, Wegwerflauf** (`throwaway/`, nichts ist daran
gebunden). Dieselben vier Aufrufe zweimal geschrieben:

* **ohne Pflanzung** → `casecheck` exit 1, jede Zeile einzeln geprüft, jede mit
  benanntem Grund und ihrer physischen Zeilennummer (die vier Texte stehen in
  `RUNINFO.TXT`);
* **mit Pflanzung** → angenommen und auf **beiden** Maschinen gefahren, je
  „all 4 cases ran", `--validate` accepted, kein Hänger. Jedes aufgezeichnete
  Wort stimmt zwischen TT und NewTOS überein: `scrp_write` 1, `scrp_read` 1,
  `form_keybd` 1 mit `FM_OCHAR` = 16706 unverändert durchgereicht (das ist, was
  `direction == -1` bedeutet), `objc_delete(ROOT)` 0.

## Rückwärtskompatibilität, vierfach belegt

Zahlen: `measurements/measurement-2026-09-13-m07-parameter/BACKCOMPAT.TXT`.

1. Alle **zwölf** Tabellen mit unveränderter CRC — und schärfer als die CRC:
   `casecheck`s **vollständige Ausgabe** (jede Fall-ID, jeder Opcode, jedes
   `control[1..4]`, jeder `int_in`-Wert, jedes Prelude, jede Pflanzung) ist
   byteweise identisch, Tabelle für Tabelle.
2. 432 Fälle in sieben AES-Tabellen liefern mit altem und neuem `core/cases.c`
   gegen dasselbe Testdouble **byteweise identische** Records. Der Alt-Bau ist
   als solcher nachgewiesen: er antwortet auf 25 und 53 noch mit dem alten
   Verweigerungstext, wortgleich.
3. Ein wiederholter `aes-r4`-Lauf ist auf **beiden** Maschinen **byteidentisch**
   zum M06-Wiederholungslauf, Header eingeschlossen. M07 verantwortet null
   Bytes.
4. `tosdiff --validate` fällt über alle **75** `*.BIN` im Baum dasselbe Urteil
   wie die Vor-M07-Fassung (71 akzeptiert, 4 `TERADESK-*TRACE` sind gar keine
   TDMP-Dumps).

**Die Frage, die nur ein gelockerter Riegel aufwirft**, ist geprüft statt
behauptet: die Opcodes 25 und 53 kommen in **keiner** der zwölf Tabellen vor
(vollständige Opcode-Vereinigung in `BACKCOMPAT.TXT` §1), und von den vier
verschärften erscheint nur `objc_delete`, zweimal, beide als `ROOT`.

## Abnahme

* `cargo run -p xtask -- test` → **492 passed / 0 failed / 1 ignored**,
  unverändert.
* `cargo run -p xtask -- build --board rpi4` und `--board qemu-virt`, je frisch
  → beide sauber, je 94 `kernel`- + 64 `tos-api`-Warnungen, exakt der
  Vorbestand (M07 fasst keine Rust-Zeile an; `kernel8.img` hat dieselbe SHA-256
  wie in M06).
* `make -C tosdump all` → exit 0: alle sechs Dumper übersetzen
  (DUMPVDI/DUMPAES/DUMPRAST, je `.PRG` und `.NAP`), Host-Suite exit 0.
* Kein QEMU- und kein Hatari-Prozess blieb laufen (`pgrep` nach jedem Lauf
  leer).

## Was ich nicht angefasst habe

* **Die anderen Objektmanager-Aufrufe** — `objc_add` (40), `objc_order` (45),
  `objc_edit` (46), dazu `menu_register` (35), `rsrc_gaddr`/`saddr` (112, 113),
  `appl_tplay` (14), `shel_get`/`put` (122, 123). Sie erreichen denselben
  vergifteten `addr_in[0]` und werden heute in `aes-r4-subfunctions.tsv`
  gefahren, auf genau derselben Grundlage wie `objc_delete(ROOT)`: ein
  genannter Parameter nimmt einen Wächterarm, der vorher zurückkehrt. Jeder
  braucht seine eigene Arm-für-Arm-Lesung seines eigenen Handlers; die aus der
  Form des Opcodes zu erraten ist, wie ein falscher Riegel entsteht.
* **`WF_COLOR`/`WF_DCOLOR` und die `graf_mouse`-Modi** (die zwei M06-Kandidaten
  für die Maschinenphase) — ausdrücklich nicht mein Auftrag. Meine Arbeit macht
  sie **nicht** leichter, und das ist der Befund: „hat dieser Bau
  `CONF_WITH_WINDOW_COLOURS`" ist weiter eine **gemessene** Eigenschaft der
  Maschine, also weder Lader- noch Parameterriegel-Frage. Wer sie messen will,
  braucht eine **dritte** Form: eine Verweigerung je Fall und je Fähigkeit auf
  der Maschinenseite. Steht so in `cases.h` und `docs/TOS-REFERENCE-DUMPER.md`.
* Kein Rust, kein Kernel, kein `tos-api`. Keine fremde Arbeit im Baum.
  Nichts committet.

## Was offen bleibt

* **Der Record hat weiterhin keinen Platz für den INHALT eines Puffers** (M05).
  `scrp_read`/`scrp_write` sind damit nur über ihren Rückgabewert messbar — dass
  `sc_write` etwas hineinschrieb und `sc_read` es zurückholte, ist belegt, *was*
  es war, nicht. Eigene Frage, eigenes Instrument.
* **`form_keybd`s Navigationsarm bleibt unmessbar**, bis eine Falltabelle einen
  echten OBJECT-Baum ausdrücken kann. Das ist keine Riegelfrage mehr, sondern
  eine Formatfrage: ein Baum ist mehr als ein Puffer.
* **`evnt_multi` ist nur als Abfrage messbar.** Jeder Arm, der wirklich wartet,
  braucht einen Eingabegeber, den ein unbeaufsichtigter Lauf nicht hat.
* **`mkrusttests`** hat für `aes-m07-parameter.tsv` kein Gegenstück gebaut —
  der Emitter ist opcode-blind, aber eine generierte Rust-Suite würde hier die
  Mausposition und die Fensterwarteschlange des Host-Prozesses mitmessen. Wer
  diese Tabelle dauerhaft bewachen will, muss das zuerst klären (dieselbe Klasse
  Problem, die M06 für die 3D-Statics notiert hat).
* **`appl_yield` und die drei anderen PCGEM-Opcodes** bleiben unmessbar, bis
  eine Primärquelle entscheidet (unverändert aus M06).

---

## Nachtrag 2026-09-14 (V17) — eine Einordnung oben ist widerlegt

Der Abschnitt „Die beiden Nachweisläufe" nennt als Unterschied neben den sechs
`AESGLOBAL`-Wörtern „die Mausposition in den zwölf `evnt_multi`-Fällen (TT
319,239 gegen NewTOS 320,240 — die Mitte eines 640×480-Schirms, um eins
versetzt)". Der Nebensatz liest sich wie ein Geräteunterschied. **Er war
keiner.** Beide Zahlen sind dieselbe Parkposition, aus derselben Bildschirm-
grösse gerechnet — nur mit zwei verschiedenen Formeln:

* EmuTOS `GCURX = xres / 2` (`vdi/vdi_mouse.c:582-583`) mit
  `xres = DEV_TAB[0] = V_REZ_HZ − 1` (`vdi/vdi_defs.h:94`,
  `vdi/vdi_control.c:203`) → `639 / 2 = 319`;
* NewTOS `backend.width() / 2` (`tos-api/src/vdi/mouse.rs`, `ensure_init`)
  → `640 / 2 = 320`.

Also ein Produktdefekt, kein Maschinenwert. Repariert unter **V17**
(`park_position`, halbiert jetzt die letzte Koordinate). Die Gegenmessung
derselben Tabelle vom 2026-09-14 (`measurements/measurement-2026-09-14-v17/`) hat **keinen
einzigen `int_out`-Unterschied** mehr; es bleiben genau die sechs
`AESGLOBAL`-Wörter.

Nicht korrigierbar ist der Kopfkommentar von `tosdump/cases/aes-m07-parameter.tsv`
(Zeilen 39-44, „WHAT THESE CASES WILL DIFFER IN, by construction and not by
defect"): die Tabellen-CRC läuft über die **ganze Datei** einschliesslich der
Kommentare (`tosdump/core/cases.c:234`), und `tosdiff`/`mkrusttests` weisen jeden
Dump mit abweichender CRC ab. Eine Textänderung dort würde beide Messungen vom
2026-09-13 entwerten. Der Satz bleibt also stehen und ist hiermit widersprochen.
