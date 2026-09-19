# A58 — Quittung

**Status: DONE — durch A27 gegenstandslos, keine Codeaenderung** ·
2026-09-20 · `TASK/A58.md`s Kopfzeile bleibt unveraendert.

## Sachverhalt (gepinnte Quelle `1a7484ab`, `aes/gemoblib.c:1395-1422`)

GET-Arm von `ob_sysvar`: `*out2` wird nur fuer `LK3DIND` (`:1398`),
`LK3DACT` (`:1402`) und `AD3DVALUE` (`:1415`) geschrieben; `INDBUTCOL`,
`ACTBUTCOL`, `BACKGRCOL` schreiben nur `*out1` (`:1405`, `:1408`, `:1411`);
ein unbekanntes `which` schreibt nichts und liefert 0 (`:1417-1418`).
NewTOS (`tos-api/src/aes/object.rs::objc_sysvar`, GET-Zweig) schreibt
`int_out[1]` **und** `int_out[2]` bei jedem GET — fuer die drei Farbselektoren
`[2] = 0`, fuer ein unbekanntes `which` `[1] = [2] = 0`.

## Warum keine Abweichung mehr existiert

Seit A27 ist der Vertrag festgelegt (`docs/COMPATIBILITY.md`, „An AES handler
that writes fewer than `OUT_LEN` words delivers 0"): der Dispatcher baut
jedes Ergebnis in einem **genullten** lokalen `int_out[O_SIZE]`
(`tos-api/src/aes/mod.rs:401`, `let mut local_intout = [0i16; O_SIZE]`) und
kopiert genau `OUT_LEN` Woerter zurueck. Ein von NewTOS explizit mit 0
beschriebenes Wort und ein ungeschriebenes Wort sind fuer den Aufrufer damit
dasselbe Wort — 0 —, und ein Wort jenseits von `OUT_LEN` erreicht ihn in
keinem der beiden Faelle. `objc_sysvar` ist nur ueber Opcode 48 erreichbar
(`mod.rs:588`), es gibt keinen Direktaufrufer mit eigenem Puffer.

**Beleg** (`measurement-2026-09-20-a47-a52-a55-a58/host-tests/a58-evidence-*.txt`,
Scratch-Test `method/scratch_a58_evidence.rs.txt`, nach dem Lauf aus dem Baum
entfernt): alle sieben Selektoren (1–6 und 99) je mit `OUT_LEN 3` und
`OUT_LEN 2`, Puffer mit `0xDEAD` vergiftet —

    LK3DIND   OUT_LEN 3: [1, 0, 1, POISON]   OUT_LEN 2: [1, 0, POISON, POISON]
    LK3DACT   OUT_LEN 3: [1, 1, 0, POISON]   OUT_LEN 2: [1, 1, POISON, POISON]
    INDBUTCOL OUT_LEN 3: [1, 8, 0, POISON]   OUT_LEN 2: [1, 8, POISON, POISON]
    ACTBUTCOL OUT_LEN 3: [1, 8, 0, POISON]   OUT_LEN 2: [1, 8, POISON, POISON]
    BACKGRCOL OUT_LEN 3: [1, 8, 0, POISON]   OUT_LEN 2: [1, 8, POISON, POISON]
    AD3DVALUE OUT_LEN 3: [1, 2, 2, POISON]   OUT_LEN 2: [1, 2, POISON, POISON]
    which 99  OUT_LEN 3: [0, 0, 0, POISON]   OUT_LEN 2: [0, 0, POISON, POISON]

Gegenprobe: derselbe Scratch-Test mit einem GET-Arm, der **nur** EmuTOS'
Woerter schreibt (`method/scratch_a58_emutos_literal.diff.txt`, von Hand
eingesetzt, aus der Dateikopie zurueckgespielt, MD5 `584d2acd…` vorher wie
nachher) — Ausgabe **wortgleich** (`diff` leer). NewTOS schreibt nirgends einen
Wert ≠ 0 in ein Wort, das EmuTOS nicht schreibt.

Das im Ticket genannte Instrument (M09) wuerde auf dem TT den Stapel messen;
das Soll dort ist per A27 als Stapel benannt und per Festlegung auf NewTOS 0.
Es bleibt nichts zu messen.

## Was getan wurde

* Kein Produktcode. Kein Hosttest (Vorgabe: nur bei Codeaenderung).
* `docs/api/aes/object-form.md` § `objc_sysvar`: Absatz „Words a GET writes"
  mit den Zeilen je Selektor, dem A27-Vertrag und dem Hinweis, dass NewTOS'
  explizite 0 durch den Dispatcher ununterscheidbar ist.

## Offen

- Nichts fuer die Hardware. Nicht committet; `docs/STATUS.md` nicht
  angefasst.
