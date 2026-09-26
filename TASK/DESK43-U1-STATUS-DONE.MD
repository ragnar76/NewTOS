# DESK43 / U1 — STATUS: DONE (2026-09-26) — das Instrument steht, Produktverhalten unveraendert

**Auftrag:** Einheit 1 von 10 des Desktop-Umzugs nach EL0
(`docs/analysis/desktop-userspace-plan-v2.md` §5 U1, Massnahme 4 aus §6.4).
Drei Dinge: das Tracer-Tor fuer den Desktop oeffnen, die falsche Begruendung
korrigieren (inklusive ihrer Kopien), und die Driftprobe als reproduzierbaren
Befehl einchecken. **Kein Produktverhalten.**

Basis: Branch `desk-rebuild`, HEAD `b3cec0c`. Die Aenderung liegt
**uncommitted** im gemeinsamen Baum. Nichts geflasht, nichts ausgeworfen, keine
Pi gefahren, nichts committet; kein `git checkout/restore/clean/stash/reset`.
`/Volumes/BOOT 1` wurde nicht angefasst, das SD-Abbild lag im Job-Scratch und
ist geloescht. `docs/STATUS.md`, `TASK/DESK43.md` und der Plan sind unberuehrt.

Messung:
[`measurement-2026-09-26-desk43-u1/`](../measurement-2026-09-26-desk43-u1/).
EmuTOS-Referenz: `references/emutos` @ `1a7484ab`.

---

## 1. Was geaendert wurde — drei Dateien, ein Skript

### 1.1 Das Tor (`kernel/src/arch/aarch64/exceptions.rs`, vorher `:344`/`:483`, jetzt `:352`/`:491`)

`let dbg_from_el0 = (frame.spsr & 0xF) == 0;` ist **weg**; beide Haken haengen
jetzt nur noch an `QED_SVC_TRACE`. Damit zeichnet der Tracer jeden `svc` auf,
gleich aus welchem Ausnahmeniveau. Die Variable ist geloescht, nicht auf `true`
gesetzt — sonst haette sie als unbenutzt gewarnt.

### 1.2 Die falsche Begruendung — drei Fundstellen, nicht zwei

`kernel/src/dbg_qedtrace.rs:14-17` behauptete: *„The desktop runs at EL1 and
calls the AES/VDI directly (never through the `svc` path)"*. Seit DESK10 falsch
(`kernel/src/desk/mod.rs:146-156` `gem_trap` = `svc #2`,
`kernel/src/desk/dosbind.rs:121-140` `dos_trap` = `svc #1`). Der Sweep mit
`/usr/bin/grep` fand **drei** Kopien:

| Datei | Stand vorher | jetzt |
|---|---|---|
| `kernel/src/dbg_qedtrace.rs:1-17` | Modulkopf, der Satz woertlich | umgeschrieben; der alte Satz steht **zitiert und widerlegt** in einem eigenen Abschnitt *„Why the hook is no longer EL0-only"* |
| `kernel/src/arch/aarch64/exceptions.rs:339-343` (vorher) | *„Traces only EL0 (user-app) traps — the desktop runs at EL1 and never reaches here"* | benennt DESK10, die beiden Trap-Stellen und den Zweck (Diff gegen `deskmain.c`) |
| `tos-api/src/gemdos/memory.rs:17-20` (vorher) | *„a kernel / desktop caller (which calls the pool directly, never through these trap readers)"* | dieselbe Widerlegung: der Desktop **kommt** durch diese Leser — `Mxalloc(-1L, MX_STRAM/MX_TTRAM)` per `svc #1` (`desk/dosbind.rs:379-393`), allerdings nur in der Sondierungsform. Die *Sache* (null `current()` ⇒ rohe physische Adressen) stimmt weiter und blieb stehen |

Dazu drei Folgesaetze im selben Modul, die dieselbe Annahme trugen und jetzt
falsch gewesen waeren: die Doc von `pre_dispatch`/`post_dispatch`/`record`
(*„Called from the EL0 `svc` handler"*) und **zwei SAFETY-Kommentare**, die
*„QED's TTBR0 is active and the block is a mapped user VA"* als Begruendung
fuehrten. Bei einem EL1-Aufrufer ist es eine Kernel-VA und `current_abi()`
liefert LP64 — beides jetzt benannt.

### 1.3 `tools/desk-drift-probe.sh` (neu, 178 Zeilen)

Die Zaehlung aus §2.2 als ein Befehl mit einer Zahl. Drei Gruppen:
`kernel/src/desk/` → `tos_api::` (A), → `crate::` ohne `crate::desk` (B),
und der Rest des Kernels → Modul `desk` (C). Alles ab dem ersten `//` einer
Zeile wird abgeschnitten, **Doc-Kommentare zaehlen nicht**. `-v` listet jede
gezaehlte Zeile, `-q` gibt nur die Zahl.

```
DRIFT-PROBE TOTAL: 47          (A 21 · B 21 · C 5)
```

Die Gruppenzahlen decken sich exakt mit §2.2 (A: `aes::object` 12,
`vdi::font` 8, `host::host` 1; B: die sieben Gruppen aus §2.2 D; C: `lib.rs:62`,
`lib.rs:475`, `accs.rs:48`, `launch.rs:266`, `shell_loop.rs:430`).

**Eine Korrektur zum Plan:** §2.2 schreibt *„36 Treffer, davon 19 reine
Doc-Kommentare"*. Gemessen sind es **21 Codezeilen und 15 Kommentarzeilen** —
21 ist auch die Summe der Gruppentabellen desselben Abschnitts (11+1 / 8 / 1),
die Zahl 19 passt zu keiner von beiden. Ab jetzt gilt die Probe.

---

## 2. Was es beweist — der Trace, gegen `deskmain.c` gestellt

### 2.1 Die Kontrolle zuerst

Derselbe Bau, nur mit dem **alten** EL0-Tor: ein Desktop-Boot schreibt
**ueberhaupt keine** `C:\QEDTRACE.LOG`
(`measurement-…/serial/boot-control-el0-gate.log`). Der Desktop ist die einzige
`evnt_multi`-Quelle in diesem Lauf — die beiden Accessories warten mit
`evnt_mesag` —, und ein EL1-`evnt_multi` loeste bisher keinen Flush aus. Das
Tor war also nicht „nur unvollstaendig", es hat den Desktop **komplett**
ausgeblendet.

### 2.2 Der Anlauf, wie er gemessen wurde

27 Zeilen, `trace/desktop-section-annotated.txt`:

```
appl_init -> 0
wind_get(desk, WF_WORKXYWH) -> (0,19,640,461)
objc_add [x8]
Ssystem(S_GETCOOKIE,_IDT) -> 4399
wind_calc  in=(0,19,640,461) out=(1,61,616,396)
Fopen "C:\NEWDESK.INF" / Fread / Fclose
Dgetdrv / Dsetdrv
objc_add [x3]
wind_set (WF_NEWDESK)
menu_bar
wind_update / evnt_dclick -> 2 / wind_update
menu_ienable [x31] / menu_icheck [x8]
graf_mouse / form_dial / menu_bar / graf_mouse [x2]
Dgetdrv / Dsetdrv
-> evnt_multi (hier wird die Datei geschrieben)
```

Zeile fuer Zeile gegen `deskmain.c:1950-2075`:
[`analysis/deskmain-comparison.md`](../measurement-2026-09-26-desk43-u1/analysis/deskmain-comparison.md).

### 2.3 Die Abweichungen — gefunden, **nicht behoben**

Zwoelf, vollstaendig in
[`analysis/differences.md`](../measurement-2026-09-26-desk43-u1/analysis/differences.md).
Die wichtigsten:

* **D2 — `graf_handle` wird nie gerufen** (`deskmain.c:1956`). Genau das Loch
  an der zweiten Stelle der Kette, die der Auftrag nennt. Das ist AES-Luecke
  **G4** und faellig in **U7**.
* **D5 — `app_start` fragt kein `shel_get`** (`deskapp.c:832`): kein Opcode 122
  im Trace, die Datei wird sofort geoeffnet. Das ist die Luecke, fuer die
  **U2** existiert — jetzt belegt statt vermutet.
* **D8/D9 — `menu_bar` laeuft zweimal**, und der Anlauf endet mit einem
  Vollbild-Redraw (`graf_mouse(M_OFF)`, `form_dial(FMD_FINISH)`, `menu_bar`,
  `graf_mouse(M_ON)`), den EmuTOS' `deskmain` nicht hat. Kein Ticket bisher.
* **D6 — Reihenfolge:** `obj_init` (8 × `objc_add`) und `wind_calc` laufen bei
  uns **vor** dem INF-Lesen, bei EmuTOS steht `win_start()` (`:2028`) hinter
  `app_start()` (`:2018`).
* **D3/D10 — die `wind_update(BEG/END)`-Klammern** um `desk_busy_on` (`:1962`)
  und um `menu_bar`+`desk_busy_off` (`:2072`) fehlen beide.

---

## 3. Bau, Clippy, Tests — beide Boards, je frisch gebaut

| | Soll (Auftrag) | gemessen |
|---|---|---|
| `cargo build --features board-qemu-virt` | tos-api 64 / kernel 94 | **64 / 94**, 0 Fehler |
| `cargo build --features board-rpi4` | tos-api 64 / kernel 94 | **64 / 94**, 0 Fehler |
| `cargo clippy --features board-qemu-virt` | 80 / 125 | **80 / 125** |
| `cargo clippy --features board-rpi4` | 80 / 125 | **80 / 125** |
| `cargo run -p xtask -- test` | 955 / 0 / 1 | **955 passed, 0 failed, 1 ignored** |

Delta: **null** in jeder Zelle. Die Aenderung ist Kommentar plus eine
geloeschte Bedingung; `QED_SVC_TRACE = false` faltet beide Haken weiter weg.
Gegenprobe gefahren: im Auslieferungsstand entsteht bei einem Boot **keine**
`QEDTRACE.LOG` (`serial/boot-final-tracer-off.log`).

---

## 4. Beruehrte Funktionen ohne EmuTOS-Zitat

Verbindlich nach §5 („(c) jede beruehrte `fn` ohne EmuTOS-Zitat"):

| `fn` | warum kein Zitat |
|---|---|
| `exceptions::rust_sync_handler` | AArch64-Trap-Einstieg; EmuTOS hat dafuer kein Gegenstueck (m68k `trap #1/#2/#13/#14`-Vektoren) |
| `dbg_qedtrace::{pre_dispatch, post_dispatch, record, trace_path, is_evnt_multi}` | reines Debug-Werkzeug, ausdruecklich nicht Produktgeschichte |
| `tools/desk-drift-probe.sh` | Host-Werkzeug |
| `gemdos::memory` (Modulkopf) | nur Doc; die Funktionen darunter blieben unberuehrt |

Keine dieser Stellen bildet TOS-Verhalten nach.

---

## 5. Was offen bleibt

1. **Der Tracer benennt drei Opcodes nicht**, die im Desktop-Anlauf zentral
   sind: 31 `menu_icheck`, 32 `menu_ienable`, 35 `menu_register` erscheinen als
   `?` (`dbg_qedtrace::aes_name`). Ich habe sie fuer die Auswertung von Hand
   aufgeloest und die Tabelle **nicht** angefasst — das waere Nachbararbeit.
   Eine Zeile Arbeit, wenn der Lead sie will.
2. **Der Tracer verschiebt Geometrie-Aufrufe.** `record()` leitet
   `wind_create`/`wind_open`/`wind_calc` und die Work-Abfragen in einen eigenen
   Puffer, der am Kopf der Datei landet — fuer QED gebaut, fuer einen
   Sequenzvergleich schaedlich. Fuer die Messung wurde das per Scratch
   abgeschaltet (`method/scratch-measurement.diff`), im Baum ist es unveraendert.
   **Wer U9 Punkt 4 (Trace-Diff vor/nach) fahren will, braucht hier eine
   Entscheidung.**
3. **`tos_api::aes::trace` (Feature `aes-trace`) war nie EL0-gattert** und sieht
   den Desktop schon lange (`slot = 0xFF`, Flag `KERNEL_CALLER`). Der Plan
   spricht in §5 U1 von *„dem AES-/GEMDOS-Tracer"* im Singular; tatsaechlich
   sind es zwei Instrumente, und nur das temporaere (`dbg_qedtrace`) war
   zu. Fuer U9 ist `aes-trace` das bessere Werkzeug (Ring wird nie geleert,
   Host-Dekoder `tools/aestrace.py`) — es braucht aber Eingabe, um zu spuelen,
   und ist damit headless auf `raspi4b` nicht ausloesbar.
4. **Zehn der zwoelf Abweichungen haben kein Ticket** (D1, D3, D4, D6–D11).
   Sie stehen in `analysis/differences.md` mit Beleg; U1 hat sie bewusst
   liegen lassen.
