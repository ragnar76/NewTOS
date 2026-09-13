# M05 — STATUS: DONE (2026-09-12)

Die Zeigermauer ist auf, an der Stelle, die der Auftrag benannt hat: eine
AES-Falltabelle kann einem Aufruf einen **Puffer per Nummer** reichen, und der
Dumper setzt auf jeder Maschine seine eigene Adresse ein. Keine bestehende
Tabelle liest sich anders, keine bestehende CRC hat sich bewegt.

## Was dazugekommen ist

* **Optionale achte Spalte `addr_in`**, per Tabelle mit `!columns 8` angemeldet.
  Ohne die Direktive bleibt alles beim Sieben-Spalten-Sinn — das ist die ganze
  Rückwärtskompatibilität in einem Satz.
* Ein Eintrag ist `aN=B` (Puffer B nach `addr_in[N]`) oder `iN=B` (Zeiger in
  Zielmaschinenbreite an `int_in`-Wort N, die klassische GEM-Bindung, **nur auf
  einem Prelude**, weil `int_in` aufgezeichnet wird und der Fußabdruck eines
  gepackten Zeigers auf den beiden Maschinen verschieden breit ist).
* `TD_AES_BUF_N` Puffer zu `TD_AES_BUF_BYTES`, vor **jedem** Aufruf neu gelegt,
  auf beiden Maschinen byteweise gleich: acht druckbare Zeichen, eine NUL, dann
  dasselbe Muster als Kanarie. Eine begrenzte Zeichenkette und nicht das
  Wortmuster des Raster-Dumpers, weil die Hälfte dieser Aufrufe einen C-String
  durch den Zeiger liest.
* Im Record steht der **Puffer-Token** `0xB0FF0000 + n`, nicht die Adresse.
* Der Fensterriegel öffnet sich an genau einer Stelle: `WF_NAME`/`WF_INFO` mit
  gepflanztem Puffer. `WF_COLOR`/`WF_DCOLOR` und `WF_NEWDESK` bleiben verweigert.

## Der Nachweis

`tosdump/cases/aes-m05-addrin.tsv` — 5 Fälle, 5 Preludes, CRC `215A9E48`,
einmalig geschrieben. Auf **beiden** Maschinen gefahren
(`measurement-2026-09-12-m05-addrin/`):

* EmuTOS 1.4 / Atari TT via Hatari: `all 5 cases ran`, `--validate` accepted.
* NewTOS / QEMU `raspi4b` 640x480: `all 5 cases ran`, `--validate` accepted.
* **A42 ist damit zum ersten Mal gemessen**: Fall 102, `wind_get(WF_NAME)` auf
  ein Fenster mit gesetztem Titel — `int_out[0]` EmuTOS **0**, NewTOS **1**.
  Der Gegenfall 109 (kein Titel) antwortet auf beiden 0.

## Die Rückwärtskompatibilität, dreifach belegt

Einzelheiten mit Zahlen: `measurement-2026-09-12-m05-addrin/BACKCOMPAT.TXT`.

1. Alle zehn Tabellen unter `tosdump/cases/` haben unveränderte CRCs und laden
   mit unveränderten Fall-, Prelude- und Spaltenzahlen.
2. Alle fünf AES-Tabellen (408 Fälle) liefern mit altem und neuem `core/` gegen
   dasselbe Testdouble **byteweise identische** Dumps.
3. Wiederholungslauf von `aes-r4` mit dem geänderten Dumper: auf dem TT 4 Bytes
   Unterschied (Offset 12..15 = Build-ID, alle 21720 Recordbytes gleich), auf
   NewTOS 3 Bytes, die alle drei zur A54-Änderung an `wind_get(WF_SCREEN)`
   gehören (Fälle 331/332/439) und nicht zu dieser Arbeit.

## Was offen bleibt

* Ein **gemessener** Fall kann einen Zeiger nur in `addr_in` tragen. Die
  klassische Bindung packt ihn in die `int_in`-WORDs, deren Breite die Maschine
  bestimmt — deshalb Prelude-only.
* Der 120-Byte-AES-Record hat keinen Platz für den **Inhalt** eines Puffers. Ein
  Aufruf, dessen einzige Wirkung im Puffer steht, ist weiter nur über seinen
  Rückgabewert messbar. Ein größerer Record entwertet jeden bereits genommenen
  Dump; das wäre eine eigene Area, wie es `vdi-raster` vorgemacht hat.
* `mkrusttests` **verweigert** eine Tabelle mit Puffern (benannter Grund), statt
  einen Rust-Test zu erzeugen, der ohne Puffer nachspielt. Wer die grosse
  Tabelle baut, braucht dafür ein Gegenstück auf der Rust-Seite.
* Die grosse Tabelle selbst ist ein eigener Auftrag, wie das Ticket sagt.
* Nicht angefasst: M06 (Riegel auf falscher Version), M07 (Riegelgranularität),
  `tosdump/deploy-tt/` und `tosdump/deploy-pi4/` (dort liegen ältere
  Dumper-Kopien; sie zu erneuern gehört zum nächsten Messlauf, nicht hierher).
