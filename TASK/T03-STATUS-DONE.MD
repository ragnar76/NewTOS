# T03 — Quittung

**Status: ERLEDIGT · beide Loecher nachgestellt vorher und nachher ·
2026-09-16** · `newtos-arch/` wird beobachtet, die Bibliotheken sind
untereinander verkettet, der irrefuehrende Kopfkommentar ist ersetzt. Eine
**Zeitstempelkette reicht — aber ueber die QUELLEN, nicht ueber die Archive.**
Die naheliegende Archivkette wurde zuerst gebaut, dann gemessen und dabei als
**falsch** erwiesen; die Messung steht unten. Kein Erzeugnis aendert sich.

Belege: `measurements/measurement-2026-09-16-toolchain/`, Nachstellungen in `repro/`.

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

## Meine Entscheidung zur Verkettung — und was ich dabei widerlegt habe

**Eine Zeitstempelkette reicht. Aber sie muss ueber die QUELLEN laufen, nicht
ueber die Archive — und das ist nicht dasselbe.** Ich habe zuerst die
naheliegende Form gebaut (`cflib.a` gegen `gemlib.a` und `libc.a` pruefen),
sie gefahren und **sie faellt bei genau dem Loch durch, das sie schliessen
soll**:

```
gemlib.a out of date [gemlib/overlay] -- running build-gemlib.sh…
  archived -> …/gemlib.a (745634 bytes, unchanged -- timestamp kept)
cflib.a nachher: 079bf3066ea3feb7  261632 Bytes   <-- UNVERAENDERT
```

`OBJECT` zu verbreitern aendert `gemlib.a` **nicht** — gemlibs Bindungen
packen Parameterbloecke, sie indizieren keine `OBJECT`-Reihen; das Archiv
bleibt bei 745634 Bytes bytegleich. Ein Archiv ist also **kein Stellvertreter
fuer „eine Kopfdatei stromaufwaerts hat sich geaendert"**. Solange `gemlib.a`
bei jedem Lauf blind neu geschrieben wird, verdeckt der frische Zeitstempel
das; sobald man das abstellt — und man muss es abstellen, siehe unten — faellt
die Archivkette auf.

Die endgueltige Form: **jede Bibliothek wird gegen die transitive Vereinigung
der Quellen geprueft.** `cflib.a` gegen `cflib/overlay` **und**
`gemlib/overlay` **und** `libc/overlay` + `libc/newtos-arch` + `include/` +
die drei Bauskripte. Damit:

* Loch (a) schliesst: `cflib.a out of date [gemlib/overlay]`, unabhaengig
  davon, ob `gemlib.a` sich geaendert hat.
* **Der befuerchtete Dauer-Neubau entsteht gar nicht.** `build-libc.sh`
  schreibt `libc.a` bedingungslos neu — `build-apitest.sh` ruft es bei jedem
  Lauf. Waere `libc.a` der Ausloeser, zoege jeder solche Lauf gemlib und cflib
  durch einen Neubau, der dieselben Bytes erzeugt. Der Ausloeser sind die
  Quellen, und die haben sich nicht bewegt: **Neubauzeilen 0**
  (`repro/t03a-after.log`, `chain-convergence.log`).
* Sie konvergiert und kann nicht kreisen: `libc → gemlib → cflib` ist eine
  Kette, `lib_ensure` laeuft sie in einem Prozess in Bauordnung ab, danach ist
  jedes Archiv neuer als jede beobachtete Quelle. Gemessen: nach der
  ausgeloesten Kaskade bauen Lauf 2, 3 und 4 nichts mehr.

Die Sorge des Auftrags — „ein Kollege hat alle Archive neu gebaut, ihre
Zeitstempel sind jetzt neuer als die `.NAP`-Dateien" — trifft die Kette
ohnehin nicht: **die `.NAP`-Dateien nehmen am Test gar nicht teil.**
`build-qed.sh` und `build-teradesk.sh` loeschen ihre Objekte und binden bei
jedem Lauf neu; es gibt keinen Test „ist die .NAP aelter als eine Bibliothek".

**Eine ausdrueckliche Abhaengigkeitsangabe braucht es trotzdem** — nur nicht
als Stempel, sondern als Deklaration. Das ist die eigentliche Ursache beider
Loecher: die Liste stand an **sechs** Aufrufstellen wortgleich, also haette
„`newtos-arch/` ergaenzen" sechs Skripte betroffen. Sie steht jetzt einmal in
`sdk/lib-stale.sh`, in `lib_chain()` und `lib_inputs()`, und die Aufrufer
sagen `lib_ensure cflib`.

### Zwei Zwischenfassungen, die ich verworfen habe

Beide sind gebaut, gefahren und wieder entfernt worden; sie stehen hier, weil
sie die Begruendung oben tragen:

1. **Archivkette, Archiv bedingungslos geschrieben.** Schliesst beide Loecher,
   konvergiert — kostet aber nach jedem `build-libc.sh` von Hand einen
   ueberfluessigen gemlib+cflib-Bau.
2. **Archivkette + „schreibe das Archiv nur bei Inhaltsaenderung"** (und dann
   zusaetzlich ein Bau-Stempel je Bibliothek, weil ein eingefrorener
   Archivzeitstempel seinen eigenen Ausloeser nie mehr ueberholt und die
   Bibliothek sonst **auf ewig** neu baut — auch das gemessen). Diese Fassung
   lief und konvergierte, **liess aber Loch (a) wieder offen**, siehe oben.
   Die Quellenkette braucht weder Inhaltstor noch Stempel.

## Was geaendert ist

`sdk/lib-stale.sh` (27 → 248 Zeilen):

* `lib_stale <archiv> <pfad…>` bleibt unveraendert das Primitiv, setzt
  zusaetzlich `LIB_STALE_REASON` — damit die Ausgabe **benennt**, was den
  Neubau ausgeloest hat (`libc.a out of date [libc/newtos-arch/do_stat.c]`).
* `lib_chain <name>` → `libc`, `libc gemlib`, `libc gemlib cflib`.
* `lib_inputs <name>` → die Quellen, die **wir** bearbeiten und die in dieses
  eine Archiv eingehen:
  - libc: `libc/overlay`, **`libc/newtos-arch`**, **`include/`**,
    `libc/build-libc.sh`
  - gemlib: `gemlib/overlay`, `gemlib/build-gemlib.sh`
  - cflib: `cflib/overlay`, `cflib/build-cflib.sh`
* `lib_watch <name>` → die Vereinigung ueber `lib_chain`, also fuer cflib alle
  neun Eintraege. Gegen **diese** Menge wird das Archiv verglichen.
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
cflib.a vorher: 079bf3066ea3feb7  261632 Bytes
gemlib.a out of date [gemlib/overlay] -- running build-gemlib.sh…
  archived -> …/gemlib.a (745634 bytes)          <-- inhaltlich UNVERAENDERT
cflib.a out of date [gemlib/overlay] -- running build-cflib.sh…
  archived -> …/cflib.a (262184 bytes)
cflib.a nachher: e81214014090c613  262184 Bytes  <-- auf dem neuen Layout
Lauf 2 Neubauzeilen: 0
Lauf 3 Neubauzeilen: 0
Lauf 4 nach libc-Bau von Hand, Neubauzeilen: 0
```

Loch (b), `repro/t03b-after.log`:

```
libc.a out of date [libc/newtos-arch/do_stat.c] -- running build-libc.sh…
gemlib.a out of date [libc/newtos-arch/do_stat.c] -- running build-gemlib.sh…
cflib.a out of date [libc/newtos-arch/do_stat.c] -- running build-cflib.sh…
build-qed EXIT=0
__newtos_staleness_probe in libc.a : 1
Lauf 2 Neubauzeilen: 0
```

## Belege

* Erzeugnisse bytegleich: `roundA-sha256.txt` (voller Bau mit den
  HEAD-Skripten) == `roundB-sha256.txt` (voller Bau mit den geaenderten) ==
  `final-sha256.txt` (nach der Umstellung auf die Quellenkette neu aufgenommen).
* `chain-convergence.log`: Lauf 1 baut alle drei, Lauf 2 und 3 nichts, ein
  `build-libc.sh` von Hand danach zieht nichts mit.
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
