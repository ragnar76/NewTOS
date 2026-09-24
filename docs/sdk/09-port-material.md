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
| **cflib 0.21.0** | `sdk/cflib/` | **eingefroren auf 2004** — siehe §3 |
| **QED** | `Applications/qed`, gebaut nach `sdk/apps/qed` | laeuft |
| **TeraDesk** | `sdk/apps/teradesk` | portiert |

## 2. Quellen im Baum, noch nicht portiert

| Was | Wo | Umfang / Lizenz |
|---|---|---|
| **zView** + zTask, zWeather, zcodeclib | `Applications/z-tools/` | Bildbetrachter; Kandidat fuer die Vorfuehrung ([PORT02](../../TASK/PORT02.md)) |
| **windom 1.21.3** | `Applications/windom-1.21.3` | **199 C-Dateien in `src/`, null Assembler**, LGPL 2.1. Bindet `gem.h`, `osbind.h`/`tos.h`, `mint/cookie.h`, `mint/sysvars.h`, `osheader.h` ein — also genau die Bibliotheken, die wir haben |
| **LDG 2.35** | `Applications/ldg-2` | **SDK-Fassung, nicht der Laderquelltext**: die 7 C-Dateien sind Demos, die Umsetzung kommt als m68k-`libldg.a` und `ldg.prg`. Brauchbar ist `include/ldg.h` plus eine Handbuchseite je Funktion. LGPL seit 2.32, Projektseite `ldg.sourceforge.net` |
| **cflib (gepflegt)** | `Applications/cflib-git` | `github.com/freemint/cflib`, Stand `ce57817` (2026-07-14) — siehe §3 |
| **Thing** | `Applications/thing-neo-codeSVN-r62` | alternativer Desktop, unbewertet |
| **HighWire** | `sdk/apps/highwire` | Browser — braucht Netzwerk (NET01-NET06, ungebaut) |

## 3. Die Altersfalle: cflib

Unsere `sdk/cflib` ist **cflib 0.21.0 vom 18.07.2004** (`cflib-21/ChangeLog`).
QED ist **5.0.5 vom 12.11.2004**, mit Aenderungen bis mindestens **2010** —
also juenger und jahrelang gegen ein cflib gepflegt, das sich weiterbewegt hat.

Das hat einen konkreten Fehler verursacht (PORT03): `make_normalpath` kappte
den Pfadtrenner, QED erwartet ihn. Die **gepflegte** cflib hat das Kappen
laengst entfernt, mit Kommentar:

```c
/* cflib-21, 2004 (unsere) */        /* github, heute */
ret = path_exists(p);                ret = path_exists(p);
if (!ret)                            /* always return path
    p[i] = '\0';                        with trailing backslash */
strcpy(path, p);                     strcpy(path, p);
```

**Verallgemeinert, und das ist der Merksatz dieser Seite:** eine eingefrorene
Fremdbibliothek ist eine Altlast mit Verfallsdatum. Bevor die naechste
Anwendung portiert wird, gehoeren **gemlib und mintlib** auf denselben
Pruefstand wie cflib — sie stammen aus derselben Ecke und derselben Zeit.

## 4. Das Materialarchiv

`/Volumes/Daten/tho-otto-backu` — 42 GB Spiegel von `tho-otto.de`,
Paketablage unter `www.tho-otto.de/download/`. Enthaelt unter anderem:

* `pml-2.03.tar.xz` — Quelltext, die von zView geforderte Fassung
* `windom-2.0.1.tar.xz` — **neuer** als die geforderte 1.21.3
* `cflib-21-2024*.tar.xz` — datierte Schnappschuesse
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
