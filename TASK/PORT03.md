# PORT03 — cflib und QED sind sich ueber den Pfadtrenner uneinig

Status: **OFFEN** · Prioritaet: **P2** · Typ: Lead-Entscheidung + Fix — Nebenbefund PORT01

Abhaengigkeiten: [PORT01](PORT01.md) (erledigt)

## Sachverhalt

Seit PORT01 erreicht das Argument QED. Es kommt aber **ein Zeichen zu kurz**
an — QED meldet „Fehler beim Lesen der Datei: README.TX". Gemessen, nicht
gefolgert:

```
[port01] Fopen len 12 path "C:\README.TX"
```

Die Kette, an der Quelle gelesen:

* cflibs `make_normalpath` (`sdk/cflib/cflib-21/fsnorm.c:85-95`) haengt einen
  `\` an und **entfernt ihn wieder**, wenn `path_exists()` nicht
  „existierendes Verzeichnis" meldet.
* `path_exists` (`fsexists.c:35-45`) ist `Fattrib(p,0,0) >= 0 && (attr&FA_DIR)`
  — fuer einen **Dateipfad** liefert das auf TOS 1-3, TOS 4 **und** EmuTOS
  gleichermassen falsch (`xchmod` → `findit("C:\README.TXT\")` → `EPTHNF`,
  `references/emutos/bdos/fsdir.c:402-411`).
* QED entfernt den Trenner **bedingungslos** (`src/poslist.c:53-54`, dasselbe
  Muster in `src/dd.c:99-101`) — es ist also gegen ein `make_normalpath`
  geschrieben, das den Trenner **immer stehen laesst**.

**Kein Portierungsfehler von uns:** unsere `fsnorm.c` ist bytegleich mit dem
unveraenderten cflib-21-Tarball (geprueft). QED und cflib-21 sind ein nicht
zueinander passendes Paar.

## Belegt, aber bewusst nicht uebernommen

Im Scratch end-to-end gezeigt (`measurement-2026-09-25-port01/method/
cflib_normalpath_experiment.py`, eine Zeile ueber den ueberwachten
`sdk/cflib/overlay/`-Kanal): laesst `make_normalpath` den Trenner stehen,
fragt QED `Fopen len 13 "C:\README.TXT"` und **oeffnet die Datei**
(`screendumps/exp2-qed.png`, Fenstertitel mit Inhalt).

Nicht gelandet, weil es das Verhalten einer **fremden Bibliothek** fuer
**jeden** cflib-Aufrufer aendert — in cflib selbst (`filesel.c:205,227,265`)
und in QED (`clipbrd.c:281`, `dd.c:99`, `global.c:433`, `poslist.c:53`,
`projekt.c:938`) — und damit fuer jede kuenftig portierte cflib-Anwendung.

## Die Entscheidung, die ansteht

Welche Seite wird korrigiert:

1. **cflib-Overlay** — `make_normalpath` laesst den Trenner stehen. Wirkt
   sofort, trifft aber alle cflib-Anwendungen; es ist eine Abweichung von der
   Originalbibliothek, die begruendet und festgehalten werden muss.
2. **Eine andere cflib-Fassung** — pruefen, ob eine spaetere/andere cflib das
   Paar mit QED richtig bildet. Dann ist es kein Eingriff, sondern die
   passende Version.
3. **`path_exists` reparieren** statt `make_normalpath` — es prueft heute
   „ist ein Verzeichnis" und wird fuer Dateipfade benutzt. Moeglicherweise
   ist **das** der eigentliche Fehler, und die anderen beiden Wege kurieren
   nur sein Symptom.

Weg 3 ist vor den anderen zu pruefen: er ist der einzige, bei dem die
Bibliothek hinterher *richtiger* ist statt nur anders.

Die Projektregel bleibt: **QEDs Quelltext wird nicht geaendert.**

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
