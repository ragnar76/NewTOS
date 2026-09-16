# T03 — Quittung

**Status: ERLEDIGT · beide Loecher nachgestellt vorher und nachher ·
2026-09-16** · `newtos-arch/` wird beobachtet, die Bibliotheken sind
untereinander verkettet, der irrefuehrende Kopfkommentar ist ersetzt. Eine
**Zeitstempelkette reicht** — die Begruendung steht unten, mitsamt der
Gegenprobe gegen den befuerchteten Dauer-Neubau. Kein Erzeugnis aendert sich.

Belege: `measurement-2026-09-16-toolchain/`, Nachstellungen in `repro/`.

## Loch (a), selbst nachgestellt

Isolierte Kopie, Skripte des HEAD `282f255`, `OBJECT` in
`sdk/gemlib/overlay/mt_gem.h.in` um zwei Byte verbreitert
(`repro/t03a-before.log`):

```
cflib.a before: 6c3672966bd8a3b4  261720 bytes
gemlib.a missing or out of date -- running build-gemlib.sh first…
compiled OK=37  FAILED=0  (of 37)
LINK OK -> …/QED.NAP   undefined symbols: 0
build-qed EXIT=0
cflib.a after : 6c3672966bd8a3b4  261720 bytes  <-- UNCHANGED
--- erzwungener build-cflib.sh danach ---
cflib.a forced: bb8c07875842c381  262184 bytes
```

QEDs 37 Objekte auf dem neuen `OBJECT`, `cflib.a` bytegleich auf dem alten.
**Ein Programm, zwei Strukturlayouts** — genau der Vorfall vom 2026-09-02,
eine Ebene hoeher.

## Loch (b), selbst nachgestellt

Eine Funktion an `sdk/libc/newtos-arch/do_stat.c`
(`repro/t03b-before.log`):

```
build-qed EXIT=0
  (keine Zeile "out of date" in der gesamten Ausgabe)
libc.a after : fbb4296dd3f1ce81  <-- UNCHANGED
__newtos_staleness_probe in libc.a : 0
```

## Meine Entscheidung zur Verkettung, und warum

**Eine Zeitstempelkette genuegt. Eine ausdrueckliche Abhaengigkeitsangabe je
Bibliothek braucht es auch — aber nicht als Stempel, sondern als
Deklaration.** Das sind zwei verschiedene Dinge, und das Ticket wirft sie
zusammen:

1. **Der Vergleich** kann eine Zeitstempelkette sein. `libc → gemlib → cflib`
   ist eine Kette, kein Kreis; nichts flussabwaerts speist zurueck.
   `lib_ensure` laeuft sie **innerhalb eines Prozesses** in dieser Reihenfolge
   ab, also stehen die Archive nach jedem Neubau in aufsteigender Zeit, und
   der naechste Lauf findet nichts. Gegenprobe gefahren
   (`repro/t03a-after.log`): nach dem ausgeloesten Neubau bauen **Lauf 2 und
   Lauf 3 nichts mehr** (`run 2 rebuild lines: 0`, `run 3: 0`).
   Die Sorge des Auftrags — „ein Kollege hat alle Archive neu gebaut, ihre
   Zeitstempel sind jetzt neuer als die `.NAP`-Dateien" — trifft die Kette
   nicht: **die `.NAP`-Dateien nehmen am Test ueberhaupt nicht teil.**
   `build-qed.sh` und `build-teradesk.sh` loeschen ihre Objekte und binden bei
   **jedem** Lauf neu; es gibt keinen Test „ist die .NAP aelter als eine
   Bibliothek". Und die drei Archive lagen bereits in der richtigen Ordnung
   (libc 23:56:21 < gemlib 23:56:43 < cflib 23:56:50), der Erstlauf mit der
   Kette hat null zusaetzliche Bauten ausgeloest.
   Ein Inhaltsstempel (Hash statt mtime) waere praeziser, aber er wuerde einen
   Zustand einfuehren, den jemand pflegen muss, und die eine Klasse Fehler,
   die er faengt — Neubau ohne Inhaltsaenderung — kostet hier genau einen
   ueberfluessigen Bibliotheksbau und konvergiert danach.

2. **Die Angabe, WAS zu einer Bibliothek gehoert, muss ausdruecklich sein —
   und an EINER Stelle.** Das ist die eigentliche Ursache beider Loecher: die
   Liste stand an **sechs** Aufrufstellen wortgleich, also haette „`newtos-arch/`
   ergaenzen" sechs Skripte betroffen, und die Verkettung haette jede
   Aufrufstelle einzeln wissen muessen. Sie steht jetzt in
   `sdk/lib-stale.sh`, in `lib_chain()` und `lib_watch()`, und die Aufrufer
   sagen `lib_ensure cflib`.

## Was geaendert ist

`sdk/lib-stale.sh` (27 → 248 Zeilen):

* `lib_stale <archiv> <pfad…>` bleibt unveraendert das Primitiv, setzt
  zusaetzlich `LIB_STALE_REASON` — damit die Ausgabe **benennt**, was den
  Neubau ausgeloest hat (`libc.a out of date [libc/newtos-arch/do_stat.c]`).
* `lib_chain <name>` → `libc`, `libc gemlib`, `libc gemlib cflib`.
* `lib_watch <name>` → die Archive, gegen die gebaut wird, **plus** was wir
  selbst bearbeiten:
  - libc: `libc/overlay`, **`libc/newtos-arch`**, **`include/`**,
    `libc/build-libc.sh`
  - gemlib: `gemlib/overlay`, `gemlib/build-gemlib.sh` + `libc.a`
  - cflib: `cflib/overlay`, `cflib/build-cflib.sh` + `gemlib.a`, `libc.a`
* `lib_ensure <name>` — der Einstieg fuer alle Aufrufer.

`sdk/include/` ist **ueber den Auftrag hinaus** aufgenommen: `newtos.h` geht
ueber `-I"$SDK/include"` in sechs der `newtos-arch`-Uebersetzungen und damit
in `libc.a` ein. Dieselbe Sorte Luecke wie `newtos-arch/`, nur eine Datei.

Nicht beobachtet, mit Begruendung im Kopf des Skripts: die **entpackten**
Quellbaeume (werden bei jedem Lauf aus Tarball+Overlay neu erzeugt — wer sie
beobachtet, hat dauerhaft alles veraltet) und die **Tarballs** selbst (ein
Austausch wirkt ohne `*_REFRESH=1` gar nicht, ein davon ausgeloester Neubau
waere also ein Neubau ohne Wirkung).

Der Kopfkommentar, der den Fehler gedeckt hat — „was beobachtet wird, ist was
wir tatsaechlich bearbeiten: das overlay-Verzeichnis und das Bauskript selbst"
— ist durch die Beschreibung beider Loecher, der tatsaechlichen Beobachtung,
der ausdruecklichen Nicht-Beobachtung und der Konvergenzbegruendung ersetzt.

## Dieselben Nachstellungen, nach der Aenderung

Loch (a), `repro/t03a-after.log`:

```
cflib.a before: cf508d2a3e008e60  261720 bytes
gemlib.a out of date [gemlib/overlay] -- running build-gemlib.sh…
cflib.a out of date [gemlib/gemlib.a] -- running build-cflib.sh…
  archived -> …/cflib.a (262184 bytes)
cflib.a after : e81214014090c613  262184 bytes  <-- auf dem neuen Layout
run 2 rebuild lines: 0
run 3 rebuild lines: 0
```

Loch (b), `repro/t03b-after.log`:

```
libc.a out of date [libc/newtos-arch/do_stat.c] -- running build-libc.sh…
gemlib.a out of date [libc/libc.a] -- running build-gemlib.sh…
cflib.a out of date [libc/libc.a] -- running build-cflib.sh…
build-qed EXIT=0
__newtos_staleness_probe in libc.a : 1
```

## Belege

* Erzeugnisse bytegleich: `roundA-sha256.txt` (voller Bau mit den
  HEAD-Skripten) == `roundB-sha256.txt` (voller Bau mit den geaenderten) ==
  `final-sha256.txt`.
* `cargo run -p xtask -- test` → **548 / 0 / 1**.
* Beide Boards `--release`: Exit 0, rustc `tos-api` 64 / `kernel` 94,
  clippy `tos-api` 81 / `kernel` 125. Delta 0.

## Ein vorgefundener Befund, nicht von dieser Arbeit

Die `DESKTOP.NAP`, die im Baum lag (`900b9ab0…`, 2026-09-14 20:13), ist aus
den heutigen Quellen **nicht reproduzierbar**: ein Neubau mit den
**unveraenderten** HEAD-Skripten liefert `b717825a…`, zweimal hintereinander
identisch. Sie entstand vor `e503efd` (21:46, `sdk/libc/overlay`), und niemand
hat sie danach neu gebaut. Diese Sorte Alterung faengt `lib-stale.sh` nicht —
eine `.NAP` wird nur dann neu, wenn jemand ihr Skript startet. Wenn das
gewollt geschlossen werden soll, ist es ein eigenes Ticket.

## Was ich NICHT angefasst habe

* `sdk/apps/{chaintest,gemtest}/build-*.sh` pruefen weiter nur auf
  **Abwesenheit** der Archive (`[ ! -f libc.a ]`) — die Form von vor dem
  2026-09-02, dieselbe Fehlerklasse. In keinem der beiden Tickets genannt.
* `sdk/gemlib/build-gemlib-m68k.sh` (eigener Baum, kein `lib_stale`).
* Keine der elf verzeichneten libc-Fehlschlagdateien (siehe T02).

## Offen

Abnahme durch den Tester. Alle fuenf Erzeugnisse sind bytegleich, ein
Verhaltensunterschied auf der Maschine ist damit ausgeschlossen.
