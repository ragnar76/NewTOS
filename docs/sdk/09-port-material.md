# 09 — Woher die Quellen kommen: Bestand, Fassungen, Lizenzen

Stand 2026-09-25. Diese Seite haelt fest, **welches Material fuer Portierungen
vorliegt, in welcher Fassung und unter welcher Lizenz** — damit die naechste
Portierung nicht wieder mit der Suche anfaengt.

Sie ist eine Bestandsaufnahme, keine Anleitung. Wie portiert wird, steht in
[04 — Porting anatomy](04-porting-anatomy.md) und
[08 — Porting checklist](08-porting-checklist.md).

---

## 1. Was bereits portiert ist und laeuft

| Was | Wo | Bemerkung |
|---|---|---|
| **mintlib** | `sdk/libc/` | libc-Unterbau. Bekannte Luecken in [07](07-mintlib-limits.md); `crt0.S` baut `argv` seit PORT01 aus `p_cmdlin` |
| **gemlib** | `sdk/gemlib/` | AES/VDI-Bindungen |
| **cflib** | `sdk/cflib/` | auf Commit `ce57817` (2026-07-14) gepinnt, seit PORT03 — siehe §3 |
| **QED** | `Applications/qed`, gebaut nach `sdk/apps/qed` | laeuft |
| **TeraDesk** | `sdk/apps/teradesk` | portiert |

## 2. Quellen im Baum, noch nicht portiert

| Was | Wo | Umfang / Lizenz |
|---|---|---|
| **zView** + zTask, zWeather, zcodeclib | `Applications/z-tools/` | Bildbetrachter; Kandidat fuer die Vorfuehrung ([PORT02](../../TASK/PORT02.md)) |
| **windom 1.21.3** | `Applications/windom-1.21.3` | **199 C-Dateien in `src/`, null Assembler**, LGPL 2.1. Bindet `gem.h`, `osbind.h`/`tos.h`, `mint/cookie.h`, `mint/sysvars.h`, `osheader.h` ein — also genau die Bibliotheken, die wir haben |
| **LDG 2.35** | `Applications/ldg-2` | **SDK-Fassung, nicht der Laderquelltext**: die 7 C-Dateien sind Demos, die Umsetzung kommt als m68k-`libldg.a` und `ldg.prg`. Brauchbar ist `include/ldg.h` plus eine Handbuchseite je Funktion. LGPL seit 2.32, Projektseite `ldg.sourceforge.net` |
| **cflib (Herkunft des Pins)** | `Applications/cflib-git` | `github.com/freemint/cflib`; aus `ce57817` (2026-07-14) ist `sdk/cflib/cflib-21-ce57817.tar.xz` gepackt. Der Klon ist **keine** Bauabhaengigkeit — siehe §3 |
| **Thing** | `Applications/thing-neo-codeSVN-r62` | alternativer Desktop, unbewertet |
| **HighWire** | `sdk/apps/highwire` | Browser — braucht Netzwerk (NET01-NET06, ungebaut) |

## 3. Die Altersfalle: cflib — behoben, aber anders als gedacht (PORT03)

**Stand seit 2026-09-25: `sdk/cflib/` ist auf den Git-Commit `ce57817`
(2026-07-14) gepinnt** (`sdk/cflib/cflib-21-ce57817.tar.xz`, aus
`git archive --prefix=cflib-21/` gepackt; Herkunft `Applications/cflib-git`).

Der Fehler, der dahin gefuehrt hat (PORT01/PORT03): `make_normalpath` kappte
den Pfadtrenner, QED erwartet ihn — QED las `README.TX` statt `README.TXT`.
Die gepflegte cflib hat das Kappen laengst entfernt, mit Kommentar:

```c
/* unsere alte Fassung */            /* seit 2ac3e85, 2024-06-12 */
ret = path_exists(p);                ret = path_exists(p);
if (!ret)                            /* always return path
    p[i] = '\0';                        with trailing backslash */
strcpy(path, p);                     strcpy(path, p);
```

### Die Lehre ist nicht „2004 gegen heute", sondern „die Versionsnummer luegt"

Die naheliegende Erzaehlung war: eine Bibliothek von **2004** trifft auf ein
QED von 2010. Gemessen stimmt sie nicht. Der eingelagerte Tarball war der
**Quellschnappschuss vom 2024-04-30** aus dem tho-otto-Spiegel (byteidentisch,
sha256 `7f0d69f4…4062e349`; er enthaelt `.github/workflows/`, `.travis.yml`).
Er war **26 Monate**, nicht 21 Jahre alt. Nur `VERSION`/`ChangeLog` sagen
„0.21.0, 18.07.2004" — **cflib hat seine Versionsnummer seit 2004 nie
erhoeht.**

Merksatz dieser Seite, praeziser gefasst: *eine eingefrorene Fremdbibliothek
ist eine Altlast mit Verfallsdatum — und ihre Versionsnummer sagt nicht, wie
alt der Stand ist.* Deshalb wird hier nach **Commit** gepinnt, wie bei
`references/emutos` und `references/freemint`, und nicht nach Version oder
Datum.

### Was der Abstand von 13 Commits enthielt

Vier uebersetzte Dateien: `fsnorm.c` (der Defekt), `fsexists.c` + `fexists.c`
(`path_exists`/`file_exists` pruefen mit `Fsfirst` + DTA statt `Fattrib` und
schneiden einen Schluss-`\` ab) und `fontsel.c` (Leck auf dem
FONTSELECT-Pfad). Alles Uebrige waren CI-Skripte, `Makefile` und `README.md`.
`SRCFILES`/COBJS blieben bei 129 Dateien, `expected-failures.txt` bleibt leer.

**Wichtig fuer kuenftige Wechsel:** `sdk/cflib/overlay/fontsel.c` ist eine
Vollkopie der Originaldatei. Ein Pin-Wechsel muss sie **umsetzen** (rebasen),
sonst nimmt sie stromaufwaerts behobene Fehler wieder zurueck. Das ist die
generelle Gefahr jedes Voll-Datei-Overlays.

### Der Wechsel muss auch ankommen

Einen neuen Tarball hinzulegen genuegte nicht: `build-cflib.sh` extrahierte
nur bei **fehlendem** Baum, und `sdk/lib-stale.sh` beobachtete die Tarballs
bewusst nicht. Seit PORT03 extrahiert cflibs Skript neu, wenn der Tarball
juenger als der Baum ist, und `lib-stale.sh` beobachtet ihn. **mintlib und
gemlib haben diese Paarung noch nicht** — wer ihren Pin bewegt, braucht sie
dort ebenso (`MINTLIB_REFRESH=1` / `GEMLIB_REFRESH=1` bis dahin).

**Der naechste Pruefstand:** gemlib (0.44.0) und mintlib (0.60.1) stammen aus
derselben Ecke und derselben Zeit und sind bisher nicht gegen ihren gepflegten
Stand gehalten worden.

## 4. Das Materialarchiv

`/Volumes/Daten/tho-otto-backu` — 42 GB Spiegel von `tho-otto.de`,
Paketablage unter `www.tho-otto.de/download/`. Enthaelt unter anderem:

* `pml-2.03.tar.xz` — Quelltext, die von zView geforderte Fassung
* `windom-2.0.1.tar.xz` — **neuer** als die geforderte 1.21.3
* `cflib-21-2024*.tar.xz` — datierte Schnappschuesse. **Achtung (PORT03):**
  nur `download/mint/cflib-21.tar.xz` (2024-04-30) ist **Quelltext**; die
  `*-dev.tar.xz` und die neueren `snapshots/cflib/*.tar.bz2` sind
  m68k-**Binaer**pakete (`usr/lib/**/libcflib.a`, `usr/include/`, `stguide/`),
  fuer eine Portierung also wertlos. Der Spiegel hatte keinen Quelltarball,
  der den PORT03-Fix enthielt — deshalb der Commit-Pin
* dazu Dutzende weiterer portierter Bibliotheken

**Der eigentliche Wert liegt in den `*-build.sh`-Skripten**, die jedem Paket
beiliegen: sie zeigen, wie die Bibliothek fuer Atari konfiguriert und gebaut
wird. Genau dieses Wissen hat bei QED gefehlt und musste erarbeitet werden.

Nicht darin gefunden: `libshare`, `libmem` (letzteres steckt laut zViews
eigener Liste moeglicherweise in `ldg-2`).

## 5. Die Regel, die ueber allem steht

**Die portierte Anwendung bleibt unveraendert.** Korrekturen gehoeren in die
Werkzeugkette (`sdk/`), nicht in den Quelltext der Anwendung. Die einzige
Ausnahme ist m68k-Assembler, der auf AArch64 neu gezielt werden muss.

PORT03 ist das Musterbeispiel: der Fehler sass in einer Bibliothek, die
Versuchung war, ihn in QED zu umgehen — richtig war, die Bibliothek auf den
Stand zu bringen, auf dem sie ihn selbst behoben hat.
