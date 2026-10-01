# T02 — Quittung

**Status: ERLEDIGT · nachgestellt vorher und nachher · 2026-09-16**
· Die Fehlschlagliste von `build-libc.sh` wird gedruckt, die Anwendungsskripte
verschlucken sie nicht mehr, und ein **nicht verzeichneter** Fehlschlag haelt
den Bau an, bevor ein beschaedigtes Archiv entsteht. Kein Erzeugnis aendert
sich. Zusammen mit T03 in einem Wurf, weil beide dieselben Dateien beruehren.

Belege: `measurements/measurement-2026-09-16-toolchain/`, Nachstellungen in `repro/`.

## Der Befund, selbst nachgestellt

Die volle Ausgabe VOR der Aenderung, in einer isolierten Kopie mit den
Skripten des HEAD `282f255`, Einspielung `#error` an `mintlib/misc/error.c`
(`repro/t02-before.log`):

```
libc.a missing or out of date -- running build-libc.sh first…
clang : Apple clang version 21.0.0 (clang-2100.1.1.101)
…
QED COBJS manifest: 37 files
compiled OK=37  FAILED=0  (of 37)
LINK OK -> …/QED.NAP (569248 bytes)
undefined symbols in the final image (goal 0): 0
build-qed EXIT=0

failed.txt lines after : 12   (misc/error.c: 1)
error() in libc.a after : 0
```

`libc.a` hat `error()` und `error_at_line()` verloren, `failed.txt` waechst
von 11 auf 12 Zeilen, und **nirgends** steht ein Wort davon. Das ist der
Mechanismus, der `mintlib/setsysvar.c` und `unix/sysinfo.c` monatelang
getragen hat.

## Wieviele Fehlschlaege es heute wirklich sind

**Elf** — dieselben elf, die das Ticket nennt. Selbst nachgezaehlt in einem
frischen Lauf (`compiled OK=613  skipped=11  (of 624)`) und jede Diagnose
gegen `sdk/libc/build/obj/*.err` geprueft; die Urteile des Tickets treffen zu.
`e503efd` hat die Liste bereits von 13 auf 11 verkuerzt (setsysvar.c,
sysinfo.c) und `unix/ioctl.c` ist von „gescheitert" nach „ausgeschlossen"
umgezogen — die Zahl im Ticket war zum Zeitpunkt der Pruefung schon aktuell.

## Was geaendert ist

**1. `sdk/libc/build-libc.sh` druckt die Liste.** Die alte Zeile
`compiled OK=613  skipped=11  -> libc.a (N bytes)` ist in zwei zerlegt:
`compiled OK=… skipped=… (of …)` vor der Pruefung, `archived -> … (N bytes)`
danach. Dazwischen steht die Liste, wie bei `build-cflib.sh` und
`build-gemlib-m68k.sh`.

**2. Ein verzeichneter Soll-Satz, neu: `sdk/libc/expected-failures.txt`.**
Elf Zeilen `<Pfad>  <Grund>`, gruppiert nach Ursache. `build-libc.sh`
vergleicht den Ist- mit dem Soll-Satz (`lib_check_failures` in
`sdk/lib-stale.sh`):

* ein Fehlschlag, der **nicht** verzeichnet ist → Datei benannt, ihre erste
  Compilerdiagnose zitiert, **Abbruch mit 1**;
* ein verzeichneter Fehlschlag, der **jetzt uebersetzt** → wird gemeldet,
  damit der Satz kuerzer wird. Genau das hat bei `e503efd` niemand bemerkt.

Der Abbruch sitzt **vor** `rm -f libc.a` / `ar cr`. Das ist der Punkt: wuerde
er danach sitzen, bliebe ein still verkuerztes Archiv liegen, das beim
naechsten Lauf frisch genug aussieht und wieder benutzt wird.

**3. Die Anwendungsskripte verschlucken nichts mehr.** `>/dev/null 2>&1`
existiert in `build-qed.sh`, `build-teradesk.sh`, `build-cflib.sh`,
`build-gemlib.sh` und `build-coremark.sh` nicht mehr. `lib_ensure` faengt die
Ausgabe in ein Protokoll, druckt den `compiled OK=`-Block (Zahl, Liste,
Urteil, Archivgroesse) und bei Fehlschlag das **ganze** Protokoll auf stderr.

**4. Dieselbe Buchfuehrung fuer gemlib und cflib — ueber den Auftrag hinaus,
und hier absichtlich streng.** Beide hatten dieselbe Luecke; `build-gemlib.sh`
druckte, anders als das Ticket sagt, ebenfalls **keine** Liste. Ihre
verzeichneten Saetze sind **leer** (`compiled OK=578 skipped=0`,
`OK=129 skipped=0`, selbst gemessen), also gilt dort die strenge Form, die
der Auftrag fuer libc ausdruecklich verbietet, solange die elf offen sind:
**jeder** Fehlschlag haelt an. Die Bedingung „nur wenn du zeigen kannst, dass
die Liste leer ist" ist fuer diese beiden erfuellt, fuer libc nicht.

## Dieselbe Nachstellung, nach der Aenderung

`repro/t02-after.log`, identische Einspielung, identischer Befehl:

```
libc.a out of date [libc/overlay/stdlib/labs.c] -- running build-libc.sh…
error: build-libc.sh FAILED -- its full output follows:
  compiled OK=612  skipped=12  (of 624)
  skipped (did not compile for AArch64/LP64):
      …
      misc/error.c
      …
  STOP: libc failure(s) NOT recorded in libc/expected-failures.txt:
    === misc/error.c ===
        …/mintlib/misc/error.c:180:2: error: NEWTOS T02 REPRO -- deliberately broken
          180 | #error NEWTOS T02 REPRO -- deliberately broken
        1 error generated.
    Fix the toolchain. Only if the file is genuinely out of reach on
    AArch64, record it with its reason in libc/expected-failures.txt.
build-qed EXIT=1

libc.a after : a25d6f27cc979cf2   <-- unangetastet, das gute Archiv ueberlebt
error() in libc.a after : 1
```

## Belege

* Erzeugnisse bytegleich, zwei vollstaendige Durchlaeufe im Arbeitsbaum, einmal
  mit den HEAD-Skripten, einmal mit den geaenderten:
  `roundA-sha256.txt` == `roundB-sha256.txt` == `final-sha256.txt`.
* `cargo run -p xtask -- test` → **548 bestanden / 0 gefallen / 1 uebersprungen**.
* Beide Boards `--release`, je nach `touch` beider Crate-Wurzeln: Exit 0,
  `tos-api` 64 / `kernel` 94 rustc-Warnungen; clippy `tos-api` 81 /
  `kernel` 125. **Delta 0** — es ist kein Rust angefasst.
* `build-coremark.sh` und `build-apitest.sh` mitgefahren (der zweite ruft
  `build-libc.sh` direkt auf): beide Exit 0, `libc.a` unveraendert.

## Was ich NICHT angefasst habe

* **Keine der elf Dateien.** Auftragsgemaess. Drei davon
  (`stdio/gets.c`, `mintlib/getwd.c`, `signal/siggetmask.c`) scheitern
  ausschliesslich an `<inline asm>:1:8: error: unsupported directive '.stabs'`
  — eine Warndirektive ohne Codewirkung. Das ist ein eigenes Ticket wert,
  keine Nebenwirkung dieser Aenderung.
* `sdk/apps/{chaintest,gemtest}/build-*.sh`: pruefen weiter nur auf
  ABWESENHEIT der Archive und rufen `build-libc.sh` weiter mit
  `>/dev/null 2>&1`. In keinem der beiden Tickets genannt.
* `sdk/gemlib/build-gemlib-m68k.sh`: druckt seine Liste seit jeher und hat
  keinen `lib_stale`-Aufruf.

## Offen

Abnahme durch den Tester. Da alle fuenf Erzeugnisse bytegleich sind, ist ein
Verhaltensunterschied auf der Maschine ausgeschlossen; was zu pruefen bliebe,
ist allein, dass ein Bau aus sauberem Baum durchlaeuft.
