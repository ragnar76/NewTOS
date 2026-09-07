# BALL.PRG auf dem Atari TT — was am Gerät bestätigt sein muss

`plat-tt.c` ist die TT/NOVA-Hälfte der Kugel-Demo. Sie **kompiliert und
linkt** (m68k-atari-mint-gcc 7.5.0, `-m68020-60`, Hardware-FPU; `BALL.PRG`,
GEMDOS-Header `0x601A`, 0 offene Symbole). Sie ist **auf keiner Atari-Hardware
gelaufen** — hier steht keine. Alles, was unten steht, ist deshalb offen, und
zwar in genau dieser Reihenfolge nach Wichtigkeit.

Der wichtigste Punkt vorweg: der Code **rät nichts**. Wo eine Antwort fehlt,
misst `plat_open()` sie am Gerät, und wo das Gerät die Messung verweigert,
steht die Annahme an genau **einer** Stelle (`assume_format()`) und wird
protokolliert. Die Liste unten sagt, wie man jede Messung nachprüft und
woran man sieht, dass sie falsch war.

---

## Was das Programm selbst beantwortet

Jeder Lauf hängt eine Zeile an `C:\BALL.LOG` an, z. B.

```
BALL: TT/NOVA screen 640x480x8, pixel format MEASURED packed-8,
      fd_wdwidth = pixels/16, pen map MEASURED, VBL running (Vsync used)
```

* `MEASURED` heißt: `plat_open()` hat es am Gerät gemessen (siehe C1–C3).
* `ASSUMED` heißt: das Gerät hat die Messung verweigert und es gilt die
  NOVA-Annahme aus `assume_format()`. **Dann ist das Bild nicht belegt,
  sondern gehofft.**

Diese eine Zeile ist der erste Blick nach dem ersten Lauf.

---

## C1 — Ist ein Gerätepixel ein Byte (packed) oder ein Bit in acht Ebenen?

**Stand:** gemessen von `probe_format()`. Ein bekanntes Chunky-Muster wird
gestaged, mit demselben `vro_cpyfm()` aufs Bild gelegt, das auch `plat_blit()`
benutzt, und an sechs Punkten mit `v_get_pixel()` zurückgelesen.

**Warum offen:** eine NOVA in 256 Farben ist packed 8 Bit; die TT-eigene
Grafik in 256 Farben sind acht **verschachtelte Bitebenen**. Beide sind
gültige „gerätespezifische Formate“ im Sinne des MFDB, und welches der
NOVA-Treiber erwartet, steht in keinem Dokument, das hier vorliegt.

**Bestätigen:** `BALL.LOG` sagt `MEASURED packed-8`. Zusätzlich, falls
strittig: `vq_scrninfo()` (VDI 102/1, EdDI) liefert in `work_out[0]`
0 = interleaved / 1 = standard / 2 = packed und in `work_out[2]` die Bits pro
Pixel. Der Aufruf ist bewusst **nicht** eingebaut (er braucht den EdDI-Cookie
und gemlib veröffentlicht keinen Prototyp dafür) — als Gegenprobe von Hand ist
er trotzdem der direkteste Weg.

**Symptom wenn falsch:** auf einer TT-eigenen Grafik (keine Karte im Betrieb)
schlägt die Messung fehl und `plat_open()` bricht sauber ab mit
*„the screen is not 8-bit packed pixels …“*. Kein verstümmeltes Bild, kein
Absturz. Nur wenn `v_get_pixel()` gar nicht antwortet (siehe C3) und die
Annahme greift, kann ein planares Gerät ein zerrissenes Bild zeigen.

---

## C2 — Was ist `fd_wdwidth` für so ein Formular?

**Stand:** gemessen, im selben Durchgang wie C1. Zwei Kandidaten werden
probiert, der erste, der die zurückgelesenen Bytes trifft, gewinnt:

| Konvention | `fd_wdwidth` | Zeilenabstand, den der Treiber daraus rechnet |
|---|---|---|
| 1 (Vorgabe) | Pixel / 16 | `fd_wdwidth * 2 * fd_nplanes` |
| 2 | Pixel / 2 | `fd_wdwidth * 2` |

**Warum offen:** das Feld ist für das planare Modell definiert und für
packed pixels sind beide Lesarten im Umlauf. Konvention 1 ist die von fVDIs
8-Bit- und 16-Bit-Treibern (`drivers/8_bit/8b_blit.c:290`,
`offset = dst->wdwidth * 2 * dst->bitplanes * y + x * PIXEL_SIZE`) und
deshalb der erste Versuch — aber fVDI ist nicht der NOVA-Treiber.

**Bestätigen:** die Zeile in `BALL.LOG` nennt die gewählte Konvention.

**Symptom wenn falsch:** wäre ein um 8 zu großer oder zu kleiner
Zeilenabstand, also ein schräg gescherter Ball. Die Messung ist vier Zeilen
hoch, damit genau das auffällt: bei falschem Abstand stimmt Zeile 0 noch und
die Zeilen 1–3 nicht mehr.

---

## C3 — Welcher VDI-Pen gehört zu welchem Gerätepixelwert?

**Stand:** gemessen von `probe_pen_map()`. Für jeden der 256 Pens wird ein
1×1-Rechteck gefüllt und mit `v_get_pixel()` der entstandene **Pixelwert**
gelesen. Das ist die Abbildung Pen → Pixelwert; invertiert ist sie das, was
`plat_palette()` braucht.

**Warum offen:** auf dem Atari-VDI ist die Pen-Nummer **nicht** der
Hardware-Index. Die Demo schreibt Pixelwerte direkt ins MFDB, stellt Farben
aber mit `vs_color()` und damit über Pens ein.

**Bestätigen:** `BALL.LOG` sagt `pen map MEASURED`. Sagt es
`ASSUMED identity`, dann hat `v_get_pixel()` keine brauchbare Antwort geliefert
(keine Bijektion über 256 Werte) — dann ist auch C1/C2 nicht gemessen, weil
beide Messungen auf `v_get_pixel()` stehen.

**Symptom wenn falsch:** die **Form** stimmt, die **Farben** nicht — ein
korrekt beleuchteter Ball in fremden Farben. Genau daran erkennt man diesen
Fehler und keinen anderen.

---

## C4 — Beschleunigt der NOVA-Treiber `vro_cpyfm` von Speicher aufs Bild?

**Stand: sehr wahrscheinlich nein, und das ist keine Treiberschwäche, sondern
die Hardware.** Kein eigener Messwert.

Die Hoffnung des Users war: „wenn wir VDI-Routinen nutzen können, wäre das bei
der NOVA null Kosten, da hardwarebeschleunigt, da werden nur die Register
angesprochen.“ Für **Bild → Bild** stimmt das. Für **Speicher → Bild** spricht
alles dagegen: der mach64-Blitter liest seine Operanden aus dem VRAM der
Karte, und die Karte hängt als **Slave** am VME/ISA-Adapter — sie kann den
TT-Speicher nicht lesen. fVDIs mach64-Treiber zieht dieselbe Grenze und sagt
sie ausdrücklich: der Hardware-Blit wird nur eingehängt, wenn Quelle **und**
Ziel auf dem sichtbaren Bild liegen, „everything else — anything involving
main memory — falls through to the CPU“
(`fVDI-MACH64/fvdi/fvdi/drivers/mach64/mach64_blit.c`).

**Folge, und der eigentliche Punkt:** der VDI-Weg kostet damit dasselbe wie
jeder andere Weg — eine CPU-Schleife über den VME-Bus. Er ist trotzdem der
richtige, weil er ohne die Framebuffer-Adresse der Karte auskommt, die auf
einem TT mit Karte **nicht** `Physbase()` ist und die kein TOS-Aufruf
herausgibt.

**Bestätigen:** die `fps`-Zeilen in `BALL.LOG` gegen C7 rechnen. Wer es genau
will: dieselbe Fläche einmal als ein großes und einmal als vier kleine
`vro_cpyfm` schicken; beschleunigt wäre der Aufwand pro Aufruf dominant,
CPU-kopiert ist er es nicht.

**Wenn doch beschleunigt:** nichts zu tun, die Demo wird nur schneller.

---

## C5 — Gibt es Double Buffering?

**Stand: als „nein“ angenommen.** `plat_open()` meldet `pages = 1`,
`plat_present()` wartet nur auf den Rücklauf.

**Warum offen:** die Annahme stammt aus der Einschätzung des Users, der
NOVA-Treiber habe keine zweite Seite; nachgeprüft ist sie nicht. Über
Standard-VDI gibt es auch keinen Weg, einem Kartentreiber eine andere
sichtbare Basis zu geben — `Setscreen()` bewegt die TT-eigene Grafik, nicht
die Karte.

**Bestätigen:** wenn der Ball sichtbar zerreißt (eine waagerechte Naht, die
mit ihm wandert), fehlt eine zweite Seite spürbar.

**Wenn es sie doch gibt:** der Einbau ist auf `plat_present()` begrenzt — Seite
zeigen, Zielbasis des Stage-Blits auf die andere legen, `pages = 2` melden.
`ball.c` führt bereits **pro Seite** ein Schmutzrechteck; am Rest der Demo
ändert sich nichts.

---

## C6 — Läuft hinter der Karte noch ein vertikaler Rücklauf?

**Stand:** gemessen von `probe_vbl()`. Der Zähler `_vbclock` ($462) wird bis
zu 100 ms lang beobachtet; bewegt er sich, benutzt `plat_present()` `Vsync()`,
sonst wartet es gar nicht.

**Warum offen:** `Vsync()` wartet auf den VBL der **TT-eigenen** Grafik. Ist
die hinter der Karte abgeschaltet, kommt kein VBL mehr und `Vsync()` kehrt nie
zurück. Ein hängendes Demo ist das Schlimmste, was passieren kann (Termin
9./10.10.), deshalb die Messung statt des blinden Aufrufs.

**Bestätigen:** `BALL.LOG` sagt `VBL running` oder `VBL STOPPED`.

**Nebenbefund, falls `running`:** der TT-VBL und der Bildaufbau der Karte sind
nicht dasselbe Signal. `Vsync()` taktet dann, aber es schützt nicht sicher vor
Tearing. Die Physik der Demo hängt am 200-Hz-Takt und nicht am Bildtakt, also
verzieht sich dadurch nichts.

---

## C7 — Wie viel geht wirklich über den VME-Bus?

**Stand:** gerechnet, nicht gemessen. Der User nennt rund **5 MB/s**
(16 Bit bei 16 MHz mit einem Waitstate). Der VME des TT zum NOVA-Adapter ist
**16 Bit breit**, ein VME-Zyklus bewegt also ein Byte (`move.b`) oder zwei
(`move.w`) — 2-Byte-Ausrichtung zählt, 4-Byte-Ausrichtung bringt nichts mehr.

Deswegen staged `plat_blit()` das Rechteck an Versatz `x & 1`: Quelle und Ziel
haben damit dieselbe Byte-Parität, und ein Treiber, der wortweise kopieren
kann, darf es auch. Das kostet nichts und ist der einzige Punkt, an dem diese
Datei etwas über den Bus voraussetzt.

**Bestätigen:** Schmutzrechteck pro Bild ≈ 20–30 KB → bei 5 MB/s ≈ 4–6 ms nur
für die Übertragung. Zusammen mit `compose()` sollte die Demo in die Nähe der
Bildwiederholrate kommen. Die `fps`-Zeilen in `BALL.LOG` und der eingebaute
`bench()`-Wert sagen, was wirklich herauskam.

---

## C8 — Die Kugelkarte kostet am Start **Sekunden**, nicht Millisekunden

**Stand:** abgeschätzt aus dem erzeugten Code, nicht gemessen.

Bei 640×480 ist der Ball 102 Pixel breit, die Karte deckt also rund
**8 200 Pixel** ab (gegen 73 568 auf dem Pi, wo sie 20 ms kostet). Der
Faktor liegt nicht an der Pixelzahl, sondern am Rechenwerk. Aus dem
Disassemblat von `build_sphere_map()`:

* `b_asin` — 14 Schleifendurchläufe zu 8 FPU-Befehlen (2 × `fmulx`,
  1 × `faddx`, 5 × `fmoves`), zweimal pro Pixel;
* `b_sqrt` — 4 Newton-Schritte zu je 6 FPU-Befehlen, davon einer `fdivx`,
  zwei- bis dreimal pro Pixel;
* dazu rund 90 weitere FPU-Befehle für Normale, Beleuchtung und Glanzlicht.

Das sind grob **20 000 FPU-Takte pro angenommenem Pixel** bei den üblichen
68882-Zeiten (`fdivx` ≈ 108, `fmulx` ≈ 76, `faddx` ≈ 56, `fmoves` ≈ 45
Takte). Bei 32 MHz und 8 200 Pixeln macht das **etwa 4 bis 8 Sekunden**
Startzeit, je nachdem wie viel der 68882 überlappt.

**Bestätigen:** nichts zu tun — `ball.c` misst sich selbst und schreibt
`map_ticks` (200-Hz-Takte) in `BALL.LOG`. Der erste Lauf liefert die Zahl.

**Wenn es wirklich Sekunden sind:** der Vorschlag des Users, die Karte im
Dateisystem abzulegen („wieso, wir können das doch im Filesystem ablegen“),
wird damit relevant. Hier ist bewusst **nichts** dafür gebaut. Zwei
billigere Hebel wären vorher zu prüfen, beide in `ball.c` und damit außerhalb
dieser Aufgabe:

1. `b_sqrt` sind vier `fdivx` (≈ 1 470 Takte), der 68882 hat `fsqrt`
   (≈ 107 Takte) — Faktor 14 auf jeder Wurzel.
2. GCC hält `float` auf m68k zwischen den Anweisungen in **D-Registern** und
   wandelt bei jedem Zugriff um; das sind die vielen `fmoves` oben und rund
   die Hälfte aller Befehle. Mit `double` fällt die Umwandlung weg, weil die
   FPU intern ohnehin erweitert rechnet. Auf dieser Maschine ist `double`
   voraussichtlich **schneller** als `float`.

---

## C9 — Voraussetzung an die Werkzeugkette (macOS)

`build-gemlib-m68k.sh` und `build-ball-tt.sh` prüfen jedes Cross-Werkzeug mit
einer Wachhund-Frage, bevor sie es benutzen. Grund: unter macOS **hängt** ein
unsigniertes Cross-Werkzeug mit `com.apple.quarantine` beim nicht-interaktiven
Aufruf, statt zu scheitern — es wartet auf einen Gatekeeper-Dialog, den
niemand sieht. Auf dieser Maschine trifft das `ar`, `ranlib`, `nm`, `objdump`,
`strip` und `readelf`; `gcc`, `as`, `ld` und `size` sind bereits freigegeben.
Einmalig zu klären, außerhalb des Repositories:

```
sudo xattr -dr com.apple.quarantine /opt/cross-mint
```

Die Skripte ändern **nichts** an `/opt/cross-mint`; sie sagen nur, was zu tun
ist, und brechen ab.

---

## Kurzfassung für den ersten Lauf am TT

1. Bildschirm auf **640×480, 256 Farben** stellen (sonst bricht `plat_open()`
   mit einer Meldung ab, die genau das sagt).
2. `BALL.PRG` in die Wurzel von `C:` legen, vom Desktop starten.
3. Beliebige Taste beendet.
4. `C:\BALL.LOG` lesen: die `TT/NOVA`-Zeile (C1–C3, C6), `map_ticks` (C8),
   die `fps`-Zeilen und `bench` (C4, C7).
