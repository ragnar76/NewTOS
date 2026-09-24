# REF01 — Die Referenzkonfiguration festschreiben, statt sie je Extraktion neu zu raten

Status: **OFFEN** · Prioritaet: **P2** · Typ: Werkzeugkette / Quellenanalyse

Abhaengigkeiten: —

## Warum jetzt

Jede EmuTOS-Extraktion prueft die `CONF_WITH_*`-Schalter neu, und zweimal in
einer Woche ging das schief: bei DESK05 und bei DESK15 stand im
`#ifdef TARGET_192`-Block eine `0`, die fuer unser Profil gar nicht gilt.
Beide Male hat es der Entwickler gemerkt, beide Male hat es Zeit gekostet, und
beim naechsten Mal merkt es vielleicht keiner.

Die Projektregel „pruefe die CONF-Gatter" bleibt richtig — aber sie soll gegen
eine **festgeschriebene Tabelle** gehen statt gegen `config.h`s Verschachtelung.

## Der Befund, der das moeglich macht

Die Referenzkonfiguration ist exakt reproduzierbar:

* `DEF` ist leer (`references/emutos/Makefile:132`),
* `make 512` setzt genau **ein** Flag: `override DEF += -DTARGET_512`
  (`:558-563`),
* eine `localconf.h` liegt nicht im Baum (`:167-172` pruefen).

Damit liefert der Praeprozessor die ganze Wahrheit:

```
m68k-atari-mint-gcc -E -dM -DTARGET_512 -Iinclude -I. <leere .c mit #include "config.h">
```

Probe in diesem Baum: **106** `CONF_*`-Makros, darunter
`CONF_WITH_DESKTOP_CONFIG=1`, `CONF_WITH_WINDOW_ICONS=1`,
`CONF_WITH_DESKTOP_SHORTCUTS=1`, `CONF_WITH_READ_INF=1`,
`CONF_WITH_VDI_16BIT=1`.

## Auftrag

1. **Skript** `tools/emutos-config.sh` — erzeugt die Tabelle aus dem gepinnten
   Baum, ohne ihn zu veraendern, und schreibt sie nach `docs/api/_emutos-config.md`.
   Scratch nur im Job-Verzeichnis. Das Skript nennt im Kopf die drei Zeilen
   oben als Begruendung, warum `-DTARGET_512` allein genuegt.
2. **Tabelle** `docs/api/_emutos-config.md` — alle `CONF_*` mit Wert,
   alphabetisch, dazu der Commit-Hash des gepinnten EmuTOS-Baums und das
   Erzeugungsdatum, damit man sieht, wogegen sie gilt.
3. **Die Fallen benennen**, die die Tabelle ausdruecklich aufloest: der
   `TARGET_192`-Block, und jeder weitere `#ifdef TARGET_*`-Block, den es gibt.
   Pruefen und schreiben, ob es fuer `TARGET_512` ueberhaupt einen eigenen
   Block gibt (ACC01 sagt nein — gegenpruefen, nicht uebernehmen).
4. **Die Maschinenfrage beantworten.** Das 512K-Abbild ist laut Makefile-
   Kommentar fuer TT *oder* Falcon. Welche Schalter haengen dann an einer
   **Laufzeit**-Erkennung statt am Uebersetzen, und was heisst das fuer eine
   Aussage wie „im Falcon-Profil ist X an"? Das ist die Frage, die die Tabelle
   allein nicht beantwortet, und sie gehoert in ihren Kopf.
5. **Eine Zeile in `docs/SOURCE-ANALYSIS-PROTOCOL.md`**, die kuenftige
   Extraktionen auf die Tabelle verweist statt auf `config.h`.

## Abnahme

Das Skript zweimal laufen lassen ergibt dieselbe Datei. Fuenf Stichproben aus
frueheren Tickets gegen die Tabelle halten (DESK05 `DESKTOP_CONFIG`, DESK15
`WINDOW_ICONS` und `DESKTOP_SHORTCUTS`, DESK14 `READ_INF`, ACC01 — die
Lebenszyklusfunktionen haengen an **keinem** Schalter) und bestaetigen, dass
die Tabelle dasselbe sagt wie die damalige Handpruefung.

Keine Produktcodeaenderung.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
