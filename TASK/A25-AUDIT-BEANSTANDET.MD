# A25 — Unabhängige Gegenprüfung des `appl_init`-Fix

**Zustand: BEANSTANDET** · 2026-09-10 · Prüfer: unabhängiger Audit-Strang (nicht der Autor)

Gepinnte EmuTOS-Quelle: `references/emutos` @ **1a7484aba1bfba23e2c0899e243b6d8177fe41df**
(2026-04-14, „Update GitHub Actions to use Node 24").
NewTOS-Basis: Arbeitsbaum auf `desk-rebuild`, letzter Commit **70caee2**, der Fix
liegt uncommitted darüber.

Kurzfassung: Die Richtung des Fix ist **richtig** — `ap_init` ORt in EmuTOS
tatsächlich nur `AP_OPEN`. Der Fix ist aber **unvollständig**: er entfernt einen
Reset, der bisher — unbeabsichtigt — die einzige Zustandshygiene an der
Startgrenze Desktop → Kindprogramm war. EmuTOS hat an dieser Grenze zwei eigene
Rücksetzungen (`appl_exit()` in `deskmain.c:2166` und `rlr->p_flags = 0` in
`gemshlib.c:636`); NewTOS hat keine von beiden. Ergebnis: ein frisch gestartetes
GEM-Programm erbt jetzt die noch offenen `WM_*`-Nachrichten des Desktops.
Reproduziert, mit A/B gegen den Stand vor dem Fix.

---

## Auftrag

Genau ein Gegenstand: der bereits implementierte A25-Fix an
`tos-api/src/aes/pd.rs::open`, sein Test- und Spec-Beiwerk, sowie die sechs im
Prüfauftrag gestellten Fragen. Kein Produktivcode geändert, keine fremde Datei
angefasst, keine Hardwareaktion, kein Commit.

## Was ich geprüft habe

- `aes/gemaplib.c` (`ap_init`, `ap_exit`, `ap_rdwr`), `aes/gemsuper.c`
  (`APPL_INIT`/`APPL_EXIT`/`EVNT_*`-Arme), `aes/gempd.c` (`getpd`, `pstart`,
  `p_nameit`, `fpdnm`), `aes/geminit.c` (Boot-Initialisierung aller AESPDs,
  `wait_for_accs`), `aes/gemshlib.c` (`sh_ldapp`), `aes/struct.h` (`p_flags`-Bits),
  `desk/deskmain.c` (Desktop-Epilog) — in der gepinnten Fassung, selbst gelesen.
- Den vollständigen Diff im Arbeitsbaum (`git diff`), nicht die Beschreibung.
- Alle NewTOS-Stellen, die `slot.flags` / `slot.queue` / `slot.name` schreiben
  oder lesen (Kompensationsprüfung, Liste unten).
- `cargo run -p xtask -- test` und den AArch64-Build selbst ausgeführt.
- Vier eigene, temporäre Testaufsätze, jeweils **A/B** gegen den Stand *vor* dem
  Fix (Export von `HEAD` via `git archive` in ein Scratch-Verzeichnis; der
  Arbeitsbaum wurde dafür nicht verändert). Alle vier sind gelöscht.

---

## Belege

### EmuTOS (Pin 1a7484a)

| Ort | Inhalt |
|---|---|
| `aes/gemaplib.c:54-63` | `ap_init`: `pid = rlr->p_pid; rlr->p_flags \|= AP_OPEN; return pid;` — sonst nichts |
| `aes/gemsuper.c:90-100` | `APPL_INIT`-Arm: sechs AESGLOBAL-Felder, dann `ret = ap_init()` |
| `aes/gemaplib.c:251-261` | `ap_exit`: **leert die Warteschlange** (`if (rlr->p_qindex) ap_rdwr(MU_MESAG, …)`), löscht dann `AP_OPEN`. Name und `AP_MESAG` bleiben |
| `aes/geminit.c:851-873` | Boot-Initialisierung **jedes** AESPD: `p_qindex = 0`, `p_name = ' '×8`, `p_appdir[0] = 0`, `p_pid = i`, `p_stat = 0`; `p_flags` bleibt 0 (BSS bzw. `bzero` der ACC-Blöcke, `:816`) |
| `aes/gempd.c:59-68` | `getpd`: setzt nur `p_pid = curpid++`, löscht **nichts** |
| `aes/gempd.c:105-127` | `pstart`: `getpd` + `p_nameit` + `p_setappdir` + `psetup` |
| `aes/gemshlib.c:633-636` | Wiederbelegung der AESPD 0 für ein neues Programm: `p_nameit`, `p_setappdir`, **`rlr->p_flags = 0;`** |
| `aes/gemshlib.c:650-657` | Nach `dos_exec`: nur falls `AP_OPEN` → `mn_cleanup`, Queue leeren, `AP_OPEN` löschen |
| `desk/deskmain.c:2159-2166` | Desktop-Epilog vor jedem Programmstart: `menu_bar(NULL,0)`, `wind_set(DESKWH, WF_NEWDESK, NULL,…)`, **`appl_exit();`** |
| `aes/struct.h:113-116` | `AP_OPEN 0x01`, `AP_MESAG 0x02`, `AP_ACCLOSE 0x04` |

### NewTOS (Arbeitsbaum)

| Ort | Inhalt |
|---|---|
| `tos-api/src/aes/pd.rs:295-311` | `open`: neuer Slot → `*slot = Aespd::EMPTY`; danach **`slot.flags \|= AP_OPEN;`** (Zeile 309), kein Queue-Reset |
| `tos-api/src/aes/pd.rs:254-260` | `Aespd::EMPTY` = `in_use false, pid 0, flags 0, name ' '×8, queue leer` |
| `tos-api/src/aes/pd.rs:313-345` | neues `init_tests`-Modul |
| `tos-api/src/aes/pd.rs:526-544` | `register`: bei vorhandener PID **nur** der Name; `flags`/`queue` bleiben |
| `tos-api/src/aes/pd.rs:465-475` | `close_if_open`: löscht den Slot nur, wenn `AP_OPEN` gesetzt ist |
| `tos-api/src/aes/appl.rs:243-268` | `appl_init`, einziger Aufrufer von `pd::open` (Zeile 246) |
| `tos-api/src/aes/appl.rs:191-199` | `current_pid()` = 1 im Kernel-Bootkontext (Desktop) |
| `tos-api/src/aes/mod.rs:504-506` | `pid_of_slot(slot) = slot + 1` |
| `tos-api/src/bdos/process.rs:787` | `allocate_slot()` nimmt den **ersten freien** Slot, also 0 |
| `kernel/src/desk/main.rs:520-548` | `run_gem_app` / `desktop_relinquish`: `menu_bar`, `save_windows`, `wind_set_newdesk(0,0)`, `sh_draw_clear` — **kein `appl_exit()`** |
| `kernel/src/desk/mod.rs:634-638` | `appl_exit()` nur beim Herunterfahren des Desktops, nicht vor einem Start |
| `kernel/src/launch.rs:102,141` | `register_process(child.slot, path)` vor dem Start, `close_client_if_open` danach |
| `kernel/src/shell_loop.rs:341-357` | schriftlich festgehalten, dass `rlr->p_flags = 0` (gemshlib.c:636) **nicht** umgesetzt ist |

---

## Ergebnis zu den sechs Punkten

### 1. Ist der Fix richtig? — JA, für `ap_init` selbst

Selbst nachgelesen: `aes/gemaplib.c:54-63` besteht aus drei Anweisungen; die
einzige Zustandsänderung ist `rlr->p_flags |= AP_OPEN`. Weder Name noch
`p_queue`/`p_qindex` noch die übrigen Flags werden angefasst. Der Dispatcher-Arm
`gemsuper.c:90-100` schreibt davor die sechs AESGLOBAL-Felder und ruft `ap_init`
danach — er setzt ebenfalls nichts zurück. Zwischen zwei `appl_init`-Aufrufen
desselben Programms läuft in EmuTOS kein weiterer Code. `slot.flags = AP_OPEN` +
`slot.queue.clear()` war also nachweislich falsch, `slot.flags |= AP_OPEN` ist
richtig. **BESTÄTIGT.**

Zwei Nebenbefunde am `appl_init`-Pfad, die der Fix nicht berührt und die ich
nicht weiterverfolgt habe:
- `ap_init` kann in EmuTOS **nicht scheitern** (`rlr` existiert immer).
  NewTOS' `appl_init` liefert `-1`, wenn die 8er-Tabelle voll und die PID neu ist
  (`appl.rs:246-250`). Beobachtbare Abweichung, unverändert durch den Fix.
- `AP_MESAG` setzt EmuTOS im Dispatcher **vor** dem Warten (`gemsuper.c:137,148`),
  NewTOS erst **nach** erfolgreichem Pop (`event.rs:552,654`, `appl.rs:393`). In
  NewTOS wird `AP_MESAG` nirgends gelesen, also derzeit nicht beobachtbar.
  `AP_ACCLOSE` (0x04) fehlt ganz — ohne Accessories ohne Wirkung.

### 2. Ist der Fix vollständig? — NEIN

**Erhält EmuTOS beim Wiederaufruf Queue, Name und übrige Flags?** Ja, innerhalb
eines Programms uneingeschränkt. Der Autor hat das richtig gesehen.

**Setzt EmuTOS an anderer Stelle doch etwas zurück?** Ja, an zwei Stellen, und
beide liegen genau an der Grenze, die NewTOS mit demselben Deskriptor überquert:

1. `desk/deskmain.c:2166` — der ROM-Desktop ruft **`appl_exit()`**, bevor ein
   Programm läuft; `ap_exit` (`gemaplib.c:251-261`) **leert dabei die
   Warteschlange** und löscht `AP_OPEN`.
2. `aes/gemshlib.c:636` — `sh_ldapp` setzt danach **`rlr->p_flags = 0;`**, bevor
   `dos_exec` das neue Programm startet.

**Wo initialisiert EmuTOS einen NEUEN Slot?** Drei Stellen, und unsere getrennte
Initialisierung entspricht nur zweien davon:

| EmuTOS | Inhalt | NewTOS-Entsprechung |
|---|---|---|
| `geminit.c:851-873` | Boot: Queue leer, Name Leerzeichen, `p_flags` 0 | `Aespd::EMPTY` (`pd.rs:254-260`) — **stimmt überein** |
| `gempd.c:59-68` `getpd` | nur `p_pid`; der Slot ist bereits genullt | `open`/`register` mit `*slot = Aespd::EMPTY` — **stimmt überein** |
| `gemshlib.c:633-636` | **Wiederbelegung** desselben Deskriptors durch das nächste Programm: Name neu, `p_flags = 0` | `pd::register` setzt **nur** den Namen (`pd.rs:526-544`); der Flags-Reset fehlt, laut `shell_loop.rs:341-357` bewusst zurückgestellt |

Die dritte Zeile ist in einem Einzeltask-AES *die* eigentliche
Neu-Initialisierung. Sie fehlt — und bis zu diesem Fix hat `slot.queue.clear()`
in `open` sie (unsauber, aber wirksam) vertreten.

### 3. Kompensationsprüfung

Vollständige Liste der Stellen, die `slot.flags`, `slot.queue` oder `slot.name`
schreiben oder lesen:

**`flags` schreiben:** `pd.rs:302` (`*slot = EMPTY`, neuer Slot in `open`),
`pd.rs:309` (`|= AP_OPEN`, der Fix), `pd.rs:438` (`close`), `pd.rs:473`
(`close_if_open`), `pd.rs:533` (`register`, frischer Slot), `appl.rs:393`,
`event.rs:552`, `event.rs:654` (jeweils `|= AP_MESAG`).
**`flags` lesen:** ausschließlich `pd.rs:471` (`close_if_open`).
**`queue` schreiben:** `pd.rs:359` (`post_msg`), `pd.rs:385` (`post_words`),
`appl.rs:382` (`pop_into`), `event.rs:542`, `event.rs:644` (`pop`),
`window.rs:3303` (`test_clear_msg`, Testhaken), die `EMPTY`-Zuweisungen oben —
und **entfallen** der bisherige `clear()` in `open`.
**`queue` lesen:** `window.rs:3286` (`test_pending_msg`), `window.rs:3294`
(`test_msg_count`) — beides Testhaken.
**`name` schreiben:** `pd.rs:543` (`register` → `p_nameit`), die
`EMPTY`-Zuweisungen. **`name` lesen:** `pd.rs:425` (`find_by_name`/`appl_find`).

Bewertung: `pd::open` hat nur einen Aufrufer (`appl.rs:246`) — die Behauptung des
Autors stimmt. Am Namen ändert der Fix nichts: `open` hat den Namen auch vorher
nicht angefasst (der Unit-Test-Teil „Name bleibt erhalten" wäre schon vor dem
Fix grün gewesen). Kein Aufrufer *liest* die alte Rücksetzung. **Aber:** eine
Stelle hat sich stillschweigend auf sie **verlassen** — die Startgrenze
Desktop → Kind, die in NewTOS gar keinen eigenen Reset hat.

#### B1 — REGRESSION, CONFIRMED, durch diesen Fix verursacht

Der Desktop läuft im Kernel-Bootkontext und bekommt `current_pid() = 1`
(`appl.rs:191-199`). Das erste vom Desktop gestartete Programm bekommt
`allocate_slot() = 0` (`process.rs:787`) und damit `pid_of_slot(0) = 1`
(`mod.rs:504-506`) — **dieselbe AES-PID und denselben Deskriptor**. `register`
setzt nur den Namen um, `open` ORt nur noch `AP_OPEN` — die Warteschlange des
Desktops wandert unverändert in das Kindprogramm.

Nachgestellt über den echten LP64-Dispatcher, nur öffentliche Einstiegspunkte,
in genau der Reihenfolge von `kernel/src/desk/main.rs` und `kernel/src/launch.rs`:

```
appl_init()                      -> 1              (Desktop, desk/mod.rs:183)
wind_create/wind_open x2, Queue leergelesen        (Desktop öffnet 2 Fenster)
wind_set(w1, WF_TOP)                               (erster Klick eines Doppelklicks,
                                                    hndl_button, desk/main.rs:59)
  -> Desktop-FIFO: 1 Nachricht  [20, 1, 0, 1, 11, 53, 276, 154]   (WM_REDRAW, Fenster 1)
register_process(0, "C:\QED.NAP")                  (launch.rs:102)
appl_init()                      -> 1              (das Kind)
  Arbeitsbaum (mit Fix):  Kind startet mit 1 geerbten Nachricht
  HEAD 70caee2 (ohne Fix): Kind startet mit 0 Nachrichten
```

Zweite, unabhängige Nachstellung über `appl_write`/`appl_read`:
`appl_read` des Kindes liefert mit Fix **1** und den Rumpf
`[20, 1, 0, 3, 10, 20, 30, 40]`, ohne Fix **0**.

Konkretes Fehlerbild: das Kindprogramm erhält als allererste Nachricht ein
`WM_REDRAW` für ein **Fensterhandle, das ihm nicht gehört** — es gehört dem
Desktop und ist von `save_windows()` (`window.rs:805-822`) bereits aus der
lebenden Fenstertabelle entfernt worden. Ein regelkonformes GEM-Programm
beantwortet das mit `wind_get(handle, WF_FIRSTXYWH)` und zeichnet in ein
Rechteck, das ihm nicht zusteht. Auch `msg[1]` (Absender-PID) ist 1, also das
Kind selbst — es kann den Fremdursprung nicht erkennen.

EmuTOS hat das Problem nicht, obwohl es denselben AESPD 0 teilt: der Desktop
ruft vorher `appl_exit()` (`deskmain.c:2166`), das die Queue leert
(`gemaplib.c:256-257`), und `sh_ldapp` setzt anschließend `p_flags = 0`
(`gemshlib.c:636`). NewTOS' `desktop_relinquish` (`desk/main.rs:529-548`) zitiert
im eigenen Kommentar genau den Bereich `deskmain.c:2085-2171`, lässt den
`appl_exit()`-Schritt daraus aber weg.

Einordnung: **Fehler, keine hinnehmbare Abweichung.** Beobachtbar über die
dokumentierte API (`evnt_mesag`/`evnt_multi`/`appl_read` eines frisch
gestarteten Programms). Nicht Atari-Hardware-spezifisch.

Wichtig für die Bewertung: das ist **kein Argument für ein Zurückdrehen**. Der
alte Zustand war ebenfalls falsch (er verwarf echte eigene Nachrichten). Richtig
ist der Fix **plus** die fehlende Rücksetzung an der Startgrenze — also ein
`appl_exit()` in `desktop_relinquish` und/oder `rlr->p_flags = 0` plus
Queue-Leerung in `pd::register`, dort wo ein vorhandener Deskriptor an einen
neuen Belegten übergeht. Welche der beiden Formen gewählt wird, entscheidet der
Entwickler; die Prüfung sagt nur, dass ohne eine davon der Vertrag verletzt ist.

#### B2 — CONFIRMED, aber NICHT durch diesen Fix verursacht (vorbestehend)

Dieselbe geteilte PID 1 hat eine zweite Folge: wenn das Kind endet, löscht
`close_client_if_open(pid_of_slot(0)) = close_if_open(1)` (`launch.rs:141`,
`pd.rs:465-475`) den Deskriptor, weil `AP_OPEN` gesetzt ist — und trifft damit
den **Desktop**. `desktop_resume()` (`desk/main.rs:551-565`) ruft kein
`appl_init` nach. Danach ist `pd::by_pid(1)` `None`, jedes
`post_msg(1, WM_REDRAW)` fällt still weg — exakt der Fehler, den
`kernel/src/desk/mod.rs:172-182` als Grund für den `appl_init`-Aufruf des
Desktops beschreibt.

Nachgestellt: nach `register_process(0,…)` → Kind-`appl_init` →
`close_client_if_open(1)` liefert ein `appl_write` an PID 1 **0** statt 1, weil
der Deskriptor weg ist. Das Ergebnis ist auf **beiden** Bäumen gleich (mit und
ohne Fix) — also vorbestehend. Ich melde es, verfolge es nicht weiter und lege
keinen Task an.

### 4. Wirkung auf die eingefrorene Messbasis — KEINE

`tosdump/cases/aes-r1.tsv`, `aes-r2-wind.tsv` und `aes-r2b-rlist.tsv` enthalten
zusammen die Opcodes **0, 24, 77, 79, 100–109** (95 / 205 / 62 Datenzeilen).
**Kein einziger Fall mit Opcode 10, 11, 12 oder 19.** `appl_read` ist in
`tosdump/core/cases.c:454-455` sogar ausdrücklich als `BLOCKS` markiert und damit
von jeder Tabelle ausgeschlossen; `appl_exit` als `FATAL` (`:462`).

Der Dumper selbst ruft `appl_init` genau einmal beim Start
(`tosdump/core/gem.c:90`) und protokolliert die sechs AESGLOBAL-Felder
(`dumpaes.c:113-120`). Diese sechs Felder rührt der Fix nicht an — er ändert nur
`flags`/`queue` des Deskriptors. Auch der Rückgabewert von `appl_init` ändert
sich nicht (dieselbe Slot-Suche).

**Ergebnis: run1-pi4, run2-pi4-r2wind und run3-pi4-rlist bleiben gültig; die
geplante Pi-Gegenmessung braucht für die bestehenden Fälle keine neuen
Tabellen.** (Der Dumper ist selbst ein `.NAP` und liefe unter B1 mit geerbter
Queue — aber er liest keine Nachrichten, also bleibt das für die Dumps folgenlos.)

### 5. Nachgeprüfte Testergebnisse — die Angaben des Autors stimmen

Selbst ausgeführt, nicht übernommen:

- `cargo run -p xtask -- test`: **Exit 0**, aufsummiert über alle 17
  Testbinaries **429 bestanden, 0 fehlgeschlagen, 1 ignoriert** (der Doc-Test
  `tos-api/src/bdos/mem/stats.rs`). Deckt sich exakt mit dem Ticket.
- `cargo build -p tos-api --target aarch64-unknown-none-softfloat`: **Exit 0**,
  0 Fehler (64 vorbestehende Warnungen).
- „Vorher rot": selbst gegengeprüft, indem ich `HEAD` (70caee2) per `git archive`
  in ein Scratch-Verzeichnis exportiert und die beiden neuen Tests dort laufen
  ließ:
  - `tos-api/tests/aes_appl_init.rs` scheitert dort in Zeile 212 mit
    `left: 0, right: 1` („appl_init discarded a pending message") — genau die im
    Ticket behauptete „Rückgabe 0 statt 1".
  - `pd.rs::init_tests` scheitert dort in Zeile 336 mit `left: 1, right: 3`
    (Flags `AP_OPEN` statt `AP_OPEN|AP_MESAG`).
  Beide neuen Tests sind also echte Regressionswächter, keine Tautologien.

Was die Tests **nicht** abdecken: den Übergang zwischen zwei Belegten desselben
Deskriptors. Genau deshalb sind sie grün, obwohl B1 offen ist.

### 6. Hardwareabnahme — für diesen Übergang existiert kein Fall

Der Autor sagt, `UNFINISHED` heiße „Hardwareabnahme fehlt". Für den
Wiederaufruffall gibt es in **keiner** eingefrorenen Tabelle einen Fall (siehe
Punkt 4). Die Gegenmessung ist damit ohne **neue** Tabelle nicht möglich; nach
den gemeinsamen Arbeitsregeln (TASK/README.md, „Neue Fälle in neue Tabellen und
neue Läufe in eigene Verzeichnisse") heißt das: neue Falltabelle mit eigener CRC,
neues Laufverzeichnis, **und** ein passender TT-Referenzlauf, sonst gibt es
nichts zu vergleichen. Zusätzlich ist `appl_read` auf EmuTOS blockierend
(`cases.c:454-455`), der Fall darf also nur nach nachgewiesen erfolgreichem
`appl_write` gelesen werden — das steht in A25 bereits richtig.

Meine Bewertung: `UNFINISHED` war für den Stand von gestern eine faire
Beschreibung. Nach dieser Prüfung ist der Stand aber nicht „fertig, nur ohne
Hardwarenachweis", sondern **unvollständig**, weil B1 eine über die API
beobachtbare Verschlechterung ist, die auf dem Host reproduzierbar ist und für
die kein Hardwarelauf nötig war.

---

## Was ich getrieben habe / was offen bleibt

**Getrieben (reproduziert):**
- Wiederaufruf `appl_init` → `appl_init` mit wartender Nachricht (Fix wirkt,
  A/B gegen 70caee2).
- Übergang Desktop → Kind über `register_process` + `appl_init` (B1, A/B).
- Realistische Desktopaktion davor: zwei Fenster, `wind_set(WF_TOP)` — die
  Warteschlange ist zum Startzeitpunkt nachweislich nicht leer (B1-Erreichbarkeit).
- Programmende → `close_client_if_open` löscht den Desktop-Deskriptor (B2,
  auf beiden Bäumen gleich).
- Voller Host-Testlauf und AArch64-Build.
- Beide neuen Tests gegen den Stand vor dem Fix (rot bestätigt).
- Opcode-Inventar aller drei eingefrorenen Tabellen.

**Nicht verifiziert (offen):**
- **Realer Pi-4-Lauf.** Alles oben ist Host-Reproduktion über den echten
  Dispatcher. Der Doppelklick-Pfad selbst (`hndl_button` → `run_gem_app`) ist auf
  echter Hardware nicht von mir gefahren worden; er braucht Eingabe und C:-Medium
  gleichzeitig. Die Erreichbarkeit von B1 habe ich über die AES-Aufrufe
  nachgestellt, die dieser Pfad ausführt — nicht über einen echten Klick.
- Ob eine bestimmte reale App (QED) auf das geerbte `WM_REDRAW` sichtbar falsch
  reagiert, habe ich nicht gemessen; ich behaupte nur, dass sie es zugestellt
  bekommt.
- Ob `menu_bar(tree,0)` oder `wind_set_newdesk(0,0)` in `desktop_relinquish`
  ihrerseits Nachrichten einreihen, habe ich nicht einzeln geprüft — für B1 nicht
  nötig, da die Erreichbarkeit schon über `hndl_button` gezeigt ist.

## Nebenbeobachtungen (nicht verfolgt, kein Task angelegt)

- Deskriptor-Leck: ein Programm, das nie `appl_init` ruft (z. B. `.NCP`), bekommt
  über `register_process` einen Slot, den `close_if_open` mangels `AP_OPEN` nie
  freigibt (`pd.rs:465-475`); nach 8 verschiedenen GEMDOS-Slots ist die
  AES-Tabelle voll und das nächste `appl_init` liefert `-1`.
- `appl_exit` löscht in NewTOS den ganzen Slot (`pd.rs:434-439`), EmuTOS behält
  Name und `AP_MESAG` (`gemaplib.c:260`) — danach findet `appl_find(name)` das
  Programm in EmuTOS noch, in NewTOS nicht mehr.
- `AP_MESAG` wird in NewTOS gesetzt, aber nirgends gelesen; `AP_ACCLOSE` fehlt.
- Der Spec-Zusatz in `docs/api/aes/application.md` ist sachlich richtig, erwähnt
  aber die Gegenstücke `gemshlib.c:636` und `gemaplib.c:256-257` nicht; die
  stehen korrekt in `docs/api/aes/shell-loop.md:309,468,478-483`. Die Korrektur
  „nicht `log2(num_colors)`" ist richtig — der Code benutzte schon `planes()`
  (`appl.rs:214-218`), die alte Spec-Zeile war falsch.

## Aufräumen

Vier temporäre Dateien angelegt, alle wieder **per `rm` gelöscht**:
`tos-api/tests/zz_audit_a25_tmp.rs`, `tos-api/tests/zz_audit_a25_reach_tmp.rs`,
`tos-api/tests/zz_audit_a25_wipe_tmp.rs` sowie die eingefügte Testkopie im
Scratch-Export. Der Scratch-Export von `HEAD` liegt außerhalb des Repos.
`git status` nach dem Aufräumen ist identisch mit dem Stand bei Auditbeginn;
kein Produktivcode, keine fremde Datei, kein eingefrorenes Artefakt verändert,
kein `git checkout`/`clean`/`stash`/`restore` benutzt, kein Commit erzeugt.
