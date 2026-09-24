# PORT02 — Welche Anwendungen koennen wir bis zum Termin portieren?

Status: **OFFEN** · Prioritaet: **P2** · Typ: Bewertung / Portierbarkeit — Anwenderwunsch 2026-09-25

Abhaengigkeiten: [PORT01](PORT01.md) (Kommandozeile erreicht die Anwendung)

## Warum

Ohne Anwendungen laesst sich am 9./10. Oktober wenig zeigen. Heute laeuft
**QED**; **TeraDesk** ist portiert. Was sonst schon im Baum liegt, ist
ungeportet und unbewertet.

## Was da ist (Bestandsaufnahme, 2026-09-25)

| Ort | Was |
|---|---|
| `Applications/z-tools/zview` | Bildbetrachter |
| `Applications/z-tools/ztask` | Task-/Programmleiste |
| `Applications/z-tools/zweather` | Wetteranzeige |
| `Applications/z-tools/zcodeclib` | Codec-Bibliothek (Unterbau von zView) |
| `Applications/thing-neo-codeSVN-r62` | Thing — alternativer Desktop |
| `sdk/apps/highwire` | HighWire — Browser |

## Lead-Entscheidung 2026-09-25: **zView zuerst**

Der Anwender hat gewaehlt. Ein Bildbetrachter wirkt auf einer Vorfuehrung
sofort und braucht kein Netzwerk. Die uebrigen Kandidaten bleiben in der
Liste, werden aber **nach** zView bewertet.

**Was `Applications/z-tools/prerequisites.txt` selbst verlangt** (nicht
geschaetzt, dort nachgelesen):

```
ldg (latest SVN, mandatory)     windom 1.21.3       freetype 2.4.6
gemlib (CVS)                    mintlib (CVS)       libxml 2.9.0
libshare 0.1.1                  libmem              pml 2.03
xpdf 3.03                       zlib 1.2.8          transprt.h (STiK/STinG)
```

**Die Frage, die zuerst beantwortet sein muss:** `ldg` ist als *mandatory*
gefuehrt — zView laedt seine Bilddecoder als LDG-Module nach, und
nachladbare Module sind bei uns [MOD01](MOD01.md), ungebaut. Lassen sich die
Decoder **statisch** dazubinden? Davon haengt ab, ob zView ein Kandidat fuer
Oktober ist oder ein Projekt danach.

Zwei Spuren aus der README des Projekts, die dabei zaehlen: ein
**`newldg`-Zweig**, der genau die LDG-Probleme angeht, und eine gepflegtere
Fassung unter `th-otto/zview`. Der Autor nennt ausserdem **ImgView** (Lonny
Pursell) als Alternative, die zView „in manchen Bereichen abloest" — pruefen,
ob die weniger verlangt.

**LDG-Quellen gibt es** (Anwender 2026-09-25) — sie liegen nur noch nicht im
Baum (`references/` hat heute `emutos`, `freemint`, `circle`). Damit ist LDG
kein fremder Klotz, sondern portierbar wie alles andere, und die Frage wird
feiner:

* **Jetzt machbar:** LDG' *Schnittstelle* portieren und die Decoder
  **statisch** dazubinden — der Lader loest die Module dann aus einer
  eingebauten Tabelle auf, statt Dateien nachzuladen. Damit laeuft zView,
  ohne dass MOD01 fertig sein muss.
* **Spaeter:** echtes Nachladen, sobald [MOD01](MOD01.md) steht — dann
  greift dieselbe Schnittstelle, und aus der Tabelle wird ein Dateizugriff.

Zu klaeren ist, ob zViews Decoder diese Trennung mitmachen oder ob sie
LDG-Eigenheiten benutzen, die nur beim echten Nachladen existieren. **Das
entscheidet, ob zView bis Oktober laeuft** — nicht die Frage, ob wir LDG
haben.

**Sie liegen bereits da: `Applications/ldg-2` (LDG 2.35)** — und beim
Hineinsehen zeigt sich, dass es die **SDK-Fassung** ist, nicht der Quelltext
des Laders:

* **7 C-Dateien, null Assembler** — und alle sieben sind Demos bzw.
  Beispielbibliotheken (`demo/client/client.c`, `demo/libs/popup.c`,
  `contrib/fileutil/fileutil.c` …).
* Die Umsetzung kommt fertig: `lib/gcc/libldg.a` (m68k, fuer uns wertlos)
  und `68K/auto/ldg.prg` als Binaerdatei.
* **Was brauchbar ist, ist die Schnittstelle:** `include/ldg.h` mit
  `ldg_open`/`ldg_close`/`ldg_find`/`ldg_init`, den `LDG`-Strukturen,
  Fehlercodes und Flags — dazu eine Handbuchseite je Funktion unter
  `man/cat3/`.

**Das macht den statischen Weg kleiner als gedacht:** LDGs Lader muss gar
nicht portiert werden. Zu bauen waeren rund sechs dokumentierte Funktionen,
die statt aus Dateien aus einer eingebauten Tabelle aufloesen.

Zwei Dinge, die dabei auffallen und zu pruefen sind:

* `LDG_STDCALL` ist dokumentiert als *„a2/d2 are not scratch register"* — die
  Modulschnittstelle traegt **m68k-Registerkonventionen**. Fuer statisch
  gebundenen AArch64-Code gegenstandslos, aber es zeigt, dass die
  Schnittstelle Annahmen mitbringt; welche noch, ist Teil der Bewertung.
* Die Dateinamen im Paket enthalten **literale Backslashes**
  (`ldg-2.35\include\ldg.h` ist *ein* Dateiname) — das Archiv wurde ohne
  Pfadumsetzung entpackt. Jedes Bauskript stolpert darueber; entweder
  vorher umsetzen oder ausdruecklich behandeln.

`windom` ist die zweite Unbekannte: ein GEM-Rahmenwerk ueber gemlib, das wir
bisher nicht gebaut haben. Aufwand dafuer getrennt ausweisen.

## Auftrag

Je Kandidat **eine belegte Einschaetzung**, keine Vermutung:

1. **Was braucht er?** Welche Bibliotheken (gemlib, cflib, LDG, eigene), und
   welche davon hat `sdk/` schon. Fehlende benennen, nicht ueberspringen.
2. **Wie viel m68k-Assembler?** Zaehlen, nicht schaetzen. Das ist die
   einzige Stelle, an der die Projektregel „die portierte Anwendung bleibt
   unveraendert" eine Ausnahme kennt.
3. **Welche Systemfunktionen braucht er, die uns fehlen?** Netzwerk faellt
   auf, aber nicht nur — gegen `docs/api/` und die offenen Tickets pruefen.
4. **Lizenz** — was erlaubt sie, was verlangt sie.
5. **Vorfuehrwert** und geschaetzter Aufwand, beides in einem Satz, und
   **getrennt** voneinander: das Beeindruckendste ist nicht zwingend das
   Naechstliegende.

Am Ende eine **Reihenfolge** mit Begruendung — was zuerst, was danach, was
realistisch nicht bis Oktober.

## Wie bewertet wird

Der Massstab ist unsere eigene Erfahrung mit QED, nicht ein Gefuehl: dort hat
sich gezeigt, dass eine Portierung im Kern ein **Neuausrichten der duennen
Trap-/Arch-Grenze plus Uebersetzen-und-Beheben** ist, keine Neuschreibung.
Ein Kandidat, der das nicht zulaesst — weil er zu tief auf Hardware sitzt
oder zu viel Assembler traegt — ist ein schlechter Kandidat, egal wie schoen
er aussieht.

Keine Codeaenderung in diesem Ticket. Ergebnis ist ein Dokument.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
