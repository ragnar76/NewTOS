# R3-Tabelle — Quittung

**Status: DONE** · 2026-09-11 · eine neue Falltabelle, die vier offene
Messlücken auf einmal schliesst

Kein Commit. Die Datei liegt uncommittet im gemeinsamen Arbeitsbaum. Keine
vorhandene Tabelle, kein eingefrorenes Artefakt und keine `.rs`-Datei wurde
angefasst.

Gepinnter EmuTOS-Stand für alle Belege im Tabellenkopf: `references/emutos` @
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (selbst nachgeprüft), also derselbe
Stand, gegen den A13–A16 und FIX-01…06 belegt sind.

## Auftrag

Eine **neue** Falltabelle bauen, die die vier offenen Messlücken A24
(verschachtelte FIRST/NEXT über zwei Fenster), A16 (die drei Rechtecke nach
`wind_close`), den P2-Pfad am Listenende (Gegenmessung zu FIX-05) und A22 (drei
und mehr überlappende Fenster) in einem Lauf beantwortet. Sie muss auf dem
Atari TT unter EmuTOS und auf der Raspberry Pi 4 unter NewTOS laufen, darf
nicht blockieren und darf keine vorhandene Tabelle verändern.

## Die Datei

`tosdump/cases/aes-r3-wind-state.tsv` — exklusiv angelegt (`open(..., "x")`;
die Datei existierte vorher nicht).

| | |
|---|---|
| Fälle | **39** |
| Vorläufe | **223** (von 256 möglichen, `TD_MAX_PRELUDES`) |
| **CRC32** | **`979FAC60`** |
| SHA-256 | `f739b4e17fb759117ecddcf985f6eba61bf4bd7f4021a98d140dd0b8e2125677` |
| Grösse | 30 935 Bytes, 603 Zeilen, reines LF, ASCII |
| Reset | `!reset wind_new` (Zeile 215) |
| Erwarteter Dump | **39 Datensätze, 4712 Bytes** (32 Byte Kopf + 39 × 120) |

**Zum Namen.** Vorgeschlagen war `aes-r3-mixed.tsv`. „mixed" sagt nur, dass es
mehrere Dinge sind. Alle vier Fragen betreffen dasselbe: Zustand, den der
Fenstermanager **zwischen** Aufrufen mitführt — den Suchzeiger pro Fenster, die
drei Rechtecke über ein `wind_close` hinweg, das Listenende und die Zerlegung
unter drei Fenstern. Daher `aes-r3-wind-state.tsv`; `r3` behält die
Rundennummerierung der vorhandenen Tabellen, `wind-state` sagt, wovon sie
handelt. Auf der Karte heisst sie im 8.3-Format `AES-R3.TSV`.

## casecheck — tatsächlich gesehen

`tosdump/host/build/casecheck` war vorhanden, wurde aber vor dem Lauf gegen den
aktuellen `core/cases.c` **neu gebaut** (`touch ../core/cases.c && make
build/casecheck`), damit der Opcode-Riegel der gebauten Fassung entspricht.
Gegenprobe an der eingefrorenen Tabelle zuerst:

    $ ./host/build/casecheck --aes cases/aes-r2b-rlist.tsv | head -1
    cases/aes-r2b-rlist.tsv: 15 cases, 46 preludes, CRC B4CC4A5A

— die in `RUNINFO.TXT` archivierten Kennzahlen, das Werkzeug ist also in Ordnung.
Dann die neue Tabelle:

    $ ./host/build/casecheck --aes cases/aes-r3-wind-state.tsv
    cases/aes-r3-wind-state.tsv: 39 cases, 223 preludes, CRC 979FAC60
    ... (39 Fälle mit ihren Vorläufen aufgelöst)
    exit 0

**Angenommen, keine Beanstandung.** Jeder Fall wurde zusätzlich einzeln gegen
seine Vorlauffolge und seine `int_in`-Werte gelesen; die Summe der
Vorlaufzähler über alle 39 Fälle ergibt genau 223.

## Die 39 Fälle

Fall-ID = **physische Zeilennummer**, wie in allen Tabellen dieses Pakets.
`VL` = Anzahl Vorläufe.

### Abschnitt A — der Suchzeiger, zwei Fenster abwechselnd (A24)

Geometrie: Fenster 1 auf `30,40,320,240`, Fenster 2 auf `60,80,200,160`
darüber — genau die, die `aes-r2-wind.tsv` und `aes-r2b-rlist.tsv` bereits auf
beiden Maschinen gefahren haben. Fenster 1 hat mehrere Reststücke, Fenster 2 ist
unverdeckt und hat genau eines; das macht den gespiegelten Fall zu einem
sauberen Ja/Nein.

| ID | VL | Aufruf | Was der Fall entscheidet |
|---|---|---|---|
| 232 | 4 | `FIRST(1)` | der Anker: das erste Stück, gegen das jedes NEXT gelesen wird |
| 238 | 4 | `FIRST(2)` | der Anker für den gespiegelten Fall: Fenster 2 hat ein Stück |
| 245 | 5 | `F1` → `NEXT(1)` | die Kontrolle: zweites Stück, ohne dass jemand dazwischen fragt |
| 253 | 6 | `F1, F2` → `NEXT(1)` | **die Kernfrage von A24**: hat ein FIRST auf Fenster 2 den Zeiger von Fenster 1 verstellt? |
| 261 | 6 | `F2, F1` → `NEXT(2)` | die Spiegelung: Fenster 2 steht an seinem eigenen Ende — ein Stück hier wäre Fenster 1's |
| 270 | 7 | `F1, F2, N1` → `NEXT(1)` | läuft der verschränkte Durchgang zweimal hintereinander weiter, wo er aufgehört hat? |
| 280 | 8 | `F1, N1, N1, N1` → `NEXT(1)` | wie lang die Liste ohne Verschränkung ist — der Massstab für 253 und 270 |

253 und 261 sind gegenläufig gebaut, und das ist Absicht: bei einem Zeiger pro
Fenster liefert 253 ein Stück und 261 ein Ende, bei einem gemeinsamen Zeiger
genau umgekehrt. Keine der beiden Antworten hängt an einer Bildschirmgrösse —
entschieden wird an „Stück oder Ende", also `w`/`h` ungleich null gegen null.

### Abschnitt B — die drei Rechtecke über ein `wind_close` hinweg (A16)

Das Fenster wird auf `30,40,320,240` geöffnet und **vor** dem Schliessen mit
`wind_set(WF_CXYWH)` auf `70,90,260,180` verschoben. Ohne diesen Zwischenzug
wäre der Fall stumpf: `wm_open` überschreibt `WS_PREV` nachträglich mit dem
Öffnungsrechteck (`gemwmlib.c:1427`), `WF_PXYWH` läse also vor und nach dem
Close denselben Wert, und eine richtige Umsetzung wäre von einer, die gar
nichts schreibt, nicht zu unterscheiden.

**Der Vorschlag aus `A16-STATUS-UNFINISHED.MD` wurde nachgeprüft, nicht
übernommen.** Übernommen: der Aufbau mit dem Zwischenzug und je ein eigener
Aufruf pro Feld. Verworfen: die dort vorgeschlagene Gegenprobe „dieselben drei
Felder nach einem `wind_close` auf ein erzeugtes, nie geöffnetes Fenster". Für
`WF_CXYWH` und `WF_PXYWH` **entscheidet sie nichts**: `wm_create` setzt beide
schon auf `gl_rzero` (`gemwmlib.c:1378-1379`), eine Umsetzung ohne die frühe
Absage käme also auf denselben Wert und der Fall wäre blind. Entscheidend ist
dort allein `WF_WXYWH`, weil `wm_create` `WS_WORK` als einziges der vier Felder
**nicht** schreibt und der Slot deshalb noch `gl_rfull` aus `wm_delete`
(`:1480`) trägt — gegen das negative Arbeitsflächenrechteck, das ein
durchgelaufenes `draw_change` hinterlassen würde. Deshalb stehen von dieser
Gegenprobe nur die beiden `WF_WXYWH`-Fälle 340/344 in der Tabelle, dafür als
Paar mit und ohne Close. Neu hinzugenommen, im Vorschlag nicht enthalten: der
`WF_FXYWH`-Kontrollfall 335 und der `wind_calc`-Fall 337.

| ID | VL | Aufruf | Was der Fall entscheidet |
|---|---|---|---|
| 301 | 3 | `WF_WXYWH`, offen | die Arbeitsfläche, die der Close ersetzen muss |
| 306 | 3 | `WF_CXYWH`, offen | das verschobene Rechteck `70,90,260,180` |
| 311 | 3 | `WF_PXYWH`, offen | das Öffnungsrechteck `30,40,320,240` |
| 317 | 4 | `WF_WXYWH`, geschlossen | muss `wm_calc(WC_WORK)` von `gl_rzero` sein, also negativ |
| 323 | 4 | `WF_CXYWH`, geschlossen | muss `gl_rzero` sein — eine Konstante, kein Geräteparameter |
| 329 | 4 | `WF_PXYWH`, geschlossen | muss das Rechteck sein, das das Fenster hatte |
| 335 | 4 | `WF_FXYWH`, geschlossen | Kontrolle: `WS_FULL` liegt nicht auf diesem Pfad, muss stehen bleiben |
| 337 | 0 | `wind_calc(WC_WORK, 0x0FEF, 0,0,0,0)` | **das Orakel für 317 auf derselben Maschine** — macht das Soll geräteunabhängig |
| 340 | 1 | erzeugt, nie geöffnet, `WF_WXYWH` | `wm_create` schreibt `WS_WORK` nie; der Ausgangswert |
| 344 | 2 | dasselbe + `wind_close(1)` | hat der Close seine frühe Absage (`:1446-1447`) eingehalten oder doch geschrieben? |

Jedes Feld hat seinen **eigenen** gemessenen Aufruf; nichts ist aus `WF_CXYWH`
hochgerechnet. Das ist Abnahmepunkt 1 von A16, auf der Maschine statt im
Hostharness.

### Abschnitt C — eine nichtleere Liste, die nichts trifft: der P2-Pfad

Fenster 2 sitzt auf **derselben oberen linken Ecke** wie Fenster 1 und ist in
Breite und Höhe je 2 Pixel kürzer (`30,40,318,238` gegen `30,40,320,240`).
Damit sind `have_piece[TOP]` und `have_piece[LEFT]` falsch
(`gemwrect.c:113-114`), übrig bleiben genau ein RIGHT- und ein BOTTOM-Stück,
und beide liegen auf dem äusseren Rand: mit Schlagschatten vollständig
ausserhalb des eigenen Rechtecks von Fenster 1, ohne ihn auf dessen letzten
zwei Randspalten und -zeilen. Die Liste ist also **nicht leer** und trotzdem
trifft kein Stück die Arbeitsfläche — genau der Aufbau, den `FIX-05` als
fehlende Gegenmessung benannt hat.

| ID | VL | Aufruf | Was der Fall entscheidet |
|---|---|---|---|
| 375 | 4 | `WF_WXYWH(1)` | die Arbeitsfläche, gegen die geschnitten wird — die Vorbedingung, auf der Maschine gelesen |
| 381 | 4 | `WF_CXYWH(1)` | Fenster 1 wird durch das Verdecken nicht verschoben |
| 387 | 4 | `WF_CXYWH(2)` | das verdeckende Rechteck: was von Fenster 1 übrig bleibt |
| 393 | 4 | `FIRST(1)` | **der P2-Fall**: vier geschriebene Wörter, `w = h = 0`, `x`/`y` vom letzten geprüften Stück |
| 400 | 5 | `F1` → `NEXT(1)` | der P1-Fall einen Aufruf später: zwei Wörter; `x`/`y` haben dort kein Soll |

Das Soll für 393 ist aus der Geometrie ableitbar und steht im Tabellenkopf:
`x = max(x des BOTTOM-Stücks, x der Arbeitsfläche)` und
`y = 40 + 238 + DROP_SHADOW_SIZE = 280`. Die beiden Bestandteile davon liefern
375 und 381 im selben Lauf, sodass auf jeder Maschine mit ihren eigenen
Randstärken nachgerechnet werden kann.

### Abschnitt D — drei Fenster, und die Reihenfolge, in der sie geöffnet wurden (A22)

Dieselben drei Rechtecke in beiden Gruppen — 1 auf `30,40,320,240`, 2 über
dessen linke obere Ecke auf `60,80,200,160`, 3 über dessen rechte untere Ecke
auf `180,160,200,160`, sodass 3 sowohl 1 als auch 2 überlappt — und dasselbe
abgefragte Fenster. Der **einzige** Unterschied ist die Öffnungsreihenfolge:

- **Gruppe I** öffnet 1, 2, 3 → Fenster 1's Liste wird von 2 und dann von 3 gebrochen.
- **Gruppe II** öffnet 1, 3, 2 → von 3 und dann von 2.

Belegt: `newrect` bricht die Listen aller Fenster, die der `everyobj`-Lauf
**vor** dem gerade geöffneten besucht (`gemwrect.c:198`, Abbruch bei
`this == last`, `gemobjop.c:93`), und `w_obadd` hängt ein neu geöffnetes Fenster
als **letztes** Kind an (`gemwmlib.c:186-202`) — die Bruchreihenfolge ist also
die Öffnungsreihenfolge. Die sichtbare Fläche ist in beiden Gruppen dieselbe,
die Zerlegung nicht: `brkrct` zerschneidet jedes **überlebende** Stück erneut.
Aus der Quelle gerechnet ergibt Gruppe I vier Stücke und Gruppe II fünf.

Das Entscheidende braucht diese beiden Zahlen nicht: eine Umsetzung, die die
verdeckenden Rechtecke in **Handle-Reihenfolge** statt in Öffnungsreihenfolge
abzieht, kann die beiden Gruppen nicht unterscheiden und muss zweimal dasselbe
antworten. Das ist ein Vergleich **innerhalb** eines Laufs und damit von jedem
Geräteparameter unabhängig.

| ID | VL | Aufruf | Was der Fall entscheidet |
|---|---|---|---|
| 431 | 6 | Gruppe I, `WF_WXYWH(1)` | die Arbeitsfläche, gegen die alle sechs Antworten geschnitten sind |
| 439 | 6 | Gruppe I, `FIRST(1)` | Stück 1 des Fensters unter beiden anderen |
| 448 | 7 | Gruppe I, `NEXT(1)` | Stück 2, oder das Ende |
| 458 | 8 | Gruppe I, `NEXT(1)` | Stück 3, oder das Ende |
| 469 | 9 | Gruppe I, `NEXT(1)` | Stück 4, oder das Ende |
| 481 | 10 | Gruppe I, `NEXT(1)` | Stück 5, oder das Ende — nach der Quelle das Ende |
| 494 | 11 | Gruppe I, `NEXT(1)` | Stück 6, oder hinter dem Ende |
| 502 | 6 | Gruppe I, `FIRST(2)` | das **mittlere** Fenster: es bricht eine Liste und wird selbst gebrochen |
| 511 | 7 | Gruppe I, `NEXT(2)` | dessen Stück 2, oder dessen Ende |
| 519 | 6 | Gruppe II, `FIRST(1)` | Stück 1 — dieselben Rechtecke, 3 vor 2 geöffnet |
| 528 | 7 | Gruppe II, `NEXT(1)` | Stück 2 |
| 538 | 8 | Gruppe II, `NEXT(1)` | Stück 3 |
| 549 | 9 | Gruppe II, `NEXT(1)` | Stück 4 |
| 561 | 10 | Gruppe II, `NEXT(1)` | Stück 5 — nach der Quelle das letzte |
| 574 | 11 | Gruppe II, `NEXT(1)` | Stück 6, oder das Ende |
| 588 | 12 | Gruppe II, `NEXT(1)` | Stück 7, oder hinter dem Ende |

Die Läufe sind bewusst ein bis zwei Schritte **länger** als die aus der Quelle
erwartete Stückzahl. Eine Umsetzung, die mehr Stücke erzeugt, soll nicht am
Rand der Tabelle abgeschnitten werden.

### Abschnitt E — das Desktopfenster, und warum es zuletzt steht

| ID | VL | Aufruf | Was der Fall entscheidet |
|---|---|---|---|
| 603 | 1 | `wind_close(0)` → `WF_CXYWH(0)` | ist es noch der ganze Bildschirm? `0,0,0,0` hiesse, die frühe Absage hat nicht gegriffen |

`D.w_win[0]` ist `VF_INUSE`, aber nie `VF_ISOPEN`; `wm_close` muss bei
`:1446-1447` umkehren. Schriebe eine Maschine trotzdem, wäre `WS_CURR` des
Desktops `gl_rzero` und jede Trefferprüfung ohne Bildschirm — und `wm_new`
räumt nur Handles ≥ 1 (`:1886`), kein Reset dieses Laufs würde es heilen. Das
ist der **einzige** Ausgang in dieser Datei, der einen anderen Fall erreichen
könnte. Deshalb ist es die letzte Zeile der Tabelle.

## Welche der vier Lücken die Tabelle schliesst

| Lücke | Abschnitt | Fälle | geschlossen? |
|---|---|---|---|
| **A24** verschachtelte FIRST/NEXT über zwei Fenster | A | 232–280 | **ja**, in beiden Richtungen (253 und 261) plus Kontrolle und Massstab |
| **A16** die drei Rechtecke nach `wind_close` | B | 301–344 | **ja**, jedes Feld einzeln, vorher und nachher, mit dem `wind_calc`-Orakel auf derselben Maschine |
| **P2** am Listenende (Gegenmessung zu FIX-05) | C | 375–400 | **ja**, mit dem P1-Fall daneben im selben Lauf |
| **A22** drei und mehr überlappende Fenster | D | 431–588 | **ja** für Stückzahl, Reihenfolge und Geometrie, und zusätzlich für die Bruchreihenfolge, weil die beiden Gruppen sich nur darin unterscheiden |

Alle vier. Keine wurde weggelassen.

**A30 ist auftragsgemäss nicht enthalten** und auch nicht enthaltbar: der
Lesepfad ist gesperrt, weil `appl_read` auf leerer Queue blockiert
(`gemaplib.c:88`, Opcode-Riegel in `core/cases.c`) und leer genau der Fehlerfall
ist. Das steht auch im Tabellenkopf, damit später niemand die Lücke hier sucht.

## Was ich bewusst weggelassen habe

1. **`wind_set(WF_TOP)`, um z-Reihenfolge und Handle-Reihenfolge zu trennen.**
   Wäre der direktere Weg zu A22's Reihenfolgefrage gewesen, führt aber über
   `wm_mktop` → `ob_order` + `draw_change` (`gemwmlib.c:1605-1613`) und damit in
   zusätzliche `WM_REDRAW`-Nachrichten an die eigene, nie gelesene Queue. Sie
   würden zwar je Handle unioniert (`gemqueue.c:45-56`), aber die
   Vereinigungsbedingung hängt am Nachrichtentyp, und ein voller Puffer blockiert
   den Sender, der hier der Empfänger ist. Die Öffnungsreihenfolge liefert
   dieselbe Trennung ohne diesen Zusatzweg — Gruppe II tut genau das.
2. **NEXT über ein `wind_close` hinweg.** `ob_delete` nimmt das Fenster vor
   `draw_change` aus dem Baum (`gemwmlib.c:1452`), die Stückliste bleibt stehen,
   der Zeiger zeigt in Speicher, den niemand mehr besitzt. Die Quelle sagt
   dazu nichts; ein Messwert ohne Soll wäre kein Beleg, sondern eine Zahl. A31
   deckt die Testseite davon ab. Abschnitt B enthält deshalb **kein** FIRST und
   kein NEXT.
3. **Ein NEXT ohne vorangehendes FIRST** (L2 aus A14). `w_setup` initialisiert
   `w_rnext` nicht, EmuTOS hat dort kein definiertes Verhalten, und der Aufruf
   würde einen freigegebenen Zeiger verfolgen. Die Vorlaufregel der bestehenden
   Tabelle gilt hier unverändert und ist im Kopf begründet.
4. **Die drei Felder nach einem Close auf ein nie geöffnetes Fenster für
   `WF_CXYWH`/`WF_PXYWH`** — siehe Abschnitt B: entscheidet nichts, weil
   `wm_create` beide schon auf `gl_rzero` setzt.

## Warum kein Fall blockieren kann — nachgeprüft, nicht angenommen

Jeder Opcode der Tabelle wurde einzeln gegen `aes_opcodes[]`
(`tosdump/core/cases.c:452 ff.`) gelesen. Es kommen genau sechs vor, alle mit
`refusal == NULL`: **100** `wind_create`, **101** `wind_open`, **102**
`wind_close`, **104** `wind_get`, **105** `wind_set`, **108** `wind_calc`.
Nichts, was auf den Bediener wartet, nichts, was die Anwendung beendet oder
ersetzt, nichts, was in TOS 3.06's AES fehlen könnte. Zusätzlich:

- kein `wind_open(0)`, kein `wind_delete`, kein `wind_set` auf Handle 0;
- kein `wind_update`, also auch kein unpaariges `END_MCTRL`;
- kein Feld mit Zeiger (`WF_NAME`, `WF_INFO`, `WF_NEWDESK`) oder ungeprüftem
  Index (`WF_COLOR`, `WF_DCOLOR`);
- **drei** verschiedene Fensterhandles über die ganze Datei (1, 2, 3) gegen die
  Schranke von vier; im schlechtesten Fall drei der acht Queue-Plätze;
- jedes gezeichnete Rechteck nichtnegativ, ohne WORD-Überlauf, und samt
  Schlagschatten innerhalb 640×400 — das grösste ist `180,160,200,160` mit der
  `WS_TRUE`-Ecke `382,322`;
- `n_intout` ist überall 5, weit unter `TD_AES_INTOUT_N = 7`.

Der Loader hat die Tabelle angenommen; der Riegel hätte sonst die Zeile und den
Grund genannt.

## Geräteabhängigkeit

Jede Rechteckantwort trägt die Randarithmetik von `wm_calc`, die auf
`gl_wbox`/`gl_hbox` läuft — ein Geräteparameter. Deshalb ist keine der vier
Fragen an einer absoluten Zahl aufgehängt:

- **A** entscheidet an „Stück oder Ende", also `w`/`h` ungleich null gegen null.
- **B** entscheidet an `gl_rzero` (Konstante), an einem Rechteck, das die
  Tabelle selbst gesetzt hat, und für `WF_WXYWH` am `wind_calc`-Fall 337
  **derselben Maschine**.
- **C** entscheidet an „vier geschriebene Wörter gegen zwei"; die
  Vorbedingung hält auf jeder Maschine, deren rechter und unterer Rand
  mindestens zwei Pixel breit ist (ein Fenster mit Sizer und beiden Schiebern
  hat weit mehr), und die Arbeitsfläche wird als eigener Fall mitgemessen.
- **D** entscheidet am Vergleich der beiden Gruppen **innerhalb** eines Laufs.

Die absoluten Koordinaten werden aufgezeichnet, nicht wegverglichen. Zur
Einordnung: A14 hat nachgefahren, dass das Soll für diese Geometrie im
Pi-Profil (1920×1080) Wert für Wert dasselbe ist wie im TT-Profil (640×480) —
die Arbeitsfläche von `30,40,320,240` ist auf beiden `31,63,296,194`. Das ist
ein Befund, keine Annahme der Tabelle: die Fälle 375 und 431 messen sie
ohnehin mit.

## Laufanleitung

**Eine Datei, byteweise dieselbe auf beiden Karten.** Der Vergleicher lehnt
zwei Dumps mit verschiedener Tabellen-CRC ab; das ist die Bindung, auf die es
hier ankommt.

### Vorbereiten am Mac

    cd /Volumes/Daten/Development/PiRusTOS
    shasum -a 256 tosdump/cases/aes-r3-wind-state.tsv
    # muss f739b4e1...2125677 ergeben, 30935 Bytes

Binär kopieren (`cp`), **nicht** durch einen Editor schicken und keine
Zeilenenden wandeln: die CRC läuft über die Rohbytes, aus LF würde CRLF und die
Tabelle wäre eine andere. Die Datei hat reines LF, wie alle vorhandenen.

Die beiden Bereitstellungsordner `tosdump/deploy-tt/` und `tosdump/deploy-pi4/`
sind gitignoriert und genau dafür da; ich habe **nichts** hineinkopiert, damit
das Bestücken in einer Hand bleibt:

    cp tosdump/cases/aes-r3-wind-state.tsv tosdump/deploy-tt/AES-R3.TSV
    cp tosdump/cases/aes-r3-wind-state.tsv tosdump/deploy-pi4/AES-R3.TSV
    # und auf der Karte zusaetzlich unter dem Namen AES.TSV ablegen
    cmp tosdump/cases/aes-r3-wind-state.tsv tosdump/deploy-tt/AES-R3.TSV

### Auf die Atari-TT-Karte

| Datei auf der Karte | Herkunft |
|---|---|
| `AES.TSV` | Kopie von `tosdump/cases/aes-r3-wind-state.tsv` — **unter diesem Namen liest das Programm sie** |
| `AES-R3.TSV` | zweite, gleiche Kopie fürs Archiv, wie `AES-R2B.TSV` danebenliegt |
| `DUMPAES.PRG` | liegt bereits in `tosdump/deploy-tt/`, unverändert |

Vorher: ein etwaiges altes `AES.BIN`/`AES.LOG` von der Karte sichern — der Lauf
überschreibt beide. Dann `DUMPAES.PRG` aus diesem Ordner per Doppelklick
starten. Zurückbringen: `AES.BIN` und `AES.LOG`.

### Auf die Pi-4-Karte

Dasselbe: die Tabelle als `AES.TSV` (und als `AES-R3.TSV`) neben `DUMPAES.NAP`,
dann `DUMPAES.NAP` vom Desktop starten. Zurück kommen `AES.BIN` und `AES.LOG`.

### Was der Lauf melden muss

    AES.TSV: 39 cases, CRC 979FAC60
    running 39 cases
    all 39 cases ran
    AES.BIN: 39 records, 4712 bytes

**39 Datensätze, 4712 Bytes.** Weicht die CRC-Zeile ab, ist die Datei beim
Kopieren verändert worden und der Lauf ist wertlos — dann nicht weitermessen,
sondern neu kopieren.

### Danach prüfen

    ./tosdump/host/build/tosdiff --validate <AES.BIN> tosdump/cases/aes-r3-wind-state.tsv
    -> area AES, 39 records of 120 bytes, build ...., case CRC 979FAC60, accepted

    ./tosdump/host/build/tosdiff tosdump/cases/aes-r3-wind-state.tsv \
        <TT-AES.BIN> <Pi-AES.BIN>

## Was beim Lauf zu beachten ist

1. **Der TT-Lauf ist Pflicht.** Für diese 39 Fälle gibt es keinen
   EmuTOS-Referenzdump; ohne ihn steht der Pi-Dump allein da und es gibt nichts
   zu vergleichen. Beide Läufe mit derselben Datei.
2. **Der Lauf zeichnet.** Fenster gehen auf, wandern, schliessen sich — in
   Abschnitt D sind es drei gleichzeitig. Das ist der Lauf, kein Fehler. Der
   Desktop malt sich beim Programmende neu.
3. **Nichts wartet auf eine Taste** (QUIRK-002). Alles, was das Programm
   ausgibt, steht im `.LOG`.
4. **Die Dumps gehören in ein eigenes Verzeichnis**, nach der Regel aus
   `TASK/README.md`: neue Läufe nicht in `run1/run2/run3` hineinmessen. Der
   TT-Dump neben die vorhandenen als eigenes `EMUTOS14-AES-r3windstate.BIN`.
5. **Reihenfolge der Auswertung**: erst 337 gegen 317 (Abschnitt B trägt sein
   Orakel selbst), dann 375/381 gegen 393 (Abschnitt C rechnet sich damit
   nach), dann 431 als Massstab für Abschnitt D. Abschnitt A braucht kein
   Orakel, nur 232/238 als Anker.
6. **Fall 603 steht zuletzt und muss zuletzt bleiben.** Wird die Tabelle
   jemals für einen Teillauf gekürzt, darf nichts hinter ihn rutschen.

## Was diese Quittung nicht behauptet

Die Tabelle ist gebaut und vom Loader angenommen; **gemessen ist noch nichts**.
Alles, was oben als „muss" steht, ist aus der gepinnten Quelle abgeleitet, nicht
von Hardware abgelesen. Die Einordnung der Dumps — welche Abweichung ein Defekt
ist und welche ein Geräteparameter — gehört zur Auswertung nach dem Lauf, nicht
hierher.

## Nicht angefasst

`tosdump/cases/aes-r1.tsv`, `aes-r2-wind.tsv`, `aes-r2b-rlist.tsv`,
`vdi-*.tsv`, `tosdump/core/*`, `tosdump/deploy-tt/*`, `tosdump/deploy-pi4/*`,
alle `*.BIN`, `*.LOG`, `MESSRUN.TXT`, `RUNINFO.TXT`, alles unter
`measurement-2026-09-09/` und `measurement-2026-09-10-qemu/`, `TASK/README.md`,
`TASK/A16.md`, `TASK/A22.md`, `TASK/A24.md`, `TASK/A25*`, `TASK/evidence/**`,
`docs/STATUS.md` und jede `.rs`-Datei. Gebaut wurde nur das Hostwerkzeug
`tosdump/host/build/casecheck` (Bauartefakt, gitignoriert); die Statuszeilen von
A16, A22 und A24 habe ich **nicht** gesetzt — diese Tabelle ist deren
Messvoraussetzung, nicht deren Abschluss.
