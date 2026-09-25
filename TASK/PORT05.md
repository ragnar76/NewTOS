# PORT05 — QED aus TeraDesk gestartet: die Pi friert ein, oder QED beendet sich sofort

Status: **OFFEN** · Prioritaet: **P1** · Typ: Belegter Defekt — Anwender 2026-09-25

Abhaengigkeiten: —

## Der Befund

Echte Pi 4, Release-Kern `1ab0a413`, Karte C:–F:. **QED aus TeraDesk heraus
gestartet**: entweder **friert die Pi ein**, oder **QED beendet sich sofort**.

Wortlaut des Anwenders: *„Wenn ich aus TeraDesk heraus QED starte, freezed
der Pi, oder QED beendet sich sofort."*

Das ist alles, was gesichert ist. Insbesondere ist **nicht** gesichert, ob
die beiden Ausgaenge von der Art des Startens abhaengen oder derselbe
Vorgang mal so und mal so endet.

## Was sich seit dem letzten Lauf geaendert hat

Genau eines, und es gehoert hierhergeschrieben, damit es nicht uebersehen
wird: **der Lead hat `TERADESK.INF` auf der Karte veraendert** (2026-09-25,
~20:58). Ergaenzt wurden zwei `ptype`-Eintraege (`*.NTP` mit `appt=4`,
`*.NAC` mit `flag=0x2001`) und vier `ftype`-Masken (`*.NAP`, `*.NCP`,
`*.NTP`, `*.NAC`). Die Vorlage liegt unveraendert unter
`hw-round-2026-09-24/TERADESK.INF`.

Das ist **kein Verdacht**, sondern die eine bekannte Aenderung. Ob sie
beteiligt ist, entscheidet ein Rueckbau, nicht eine Ueberlegung.

## Gemessen 2026-09-25: der Desktop startet QED einwandfrei

Anwender, nach der ersten Trennung: *„Aus NewDesk funktioniert QED
einwandfrei."*

**Damit ist QED entlastet und der Startweg belastet.** Der Unterschied
zwischen den beiden Wegen ist nicht die Anwendung, sondern **wer sie
startet**:

* Unser Desktop (`kernel/src/desk/`) laeuft **im Kernel-Bootpfad** und startet
  ueber `kernel/src/launch.rs` / `shell_loop.rs` — kernelseitig.
* TeraDesk ist eine **EL0-Anwendung**, die ihrerseits eine EL0-Anwendung
  startet.

Ein von einer Anwendung ausgeloester Start hat also bei uns noch nie
funktioniert bewiesen bekommen — und ist ein anderer Pfad als der, ueber den
jede bisherige QED-Abnahme lief.

**`TERADESK.INF` ist dadurch nicht entlastet.** Die Datei ist ausschliesslich
TeraDesks Konfiguration; unser Desktop liest sie nicht. Der Rueckbau bleibt
der billigste Trenner und steht weiterhin aus.

**Offene Ticketnachbarschaft, als Verweis und nicht als Diagnose:**
[ARCH02](ARCH02.md) steht offen und betrifft genau diesen Pfad
(FP/SIMD-Register auf dem `Pexec`-Eltern/Kind-Weg). ACC03 hat denselben
Mangel fuer den Accessory-Kontextwechsel gefunden und behoben; fuer `Pexec`
ist er **nicht** behoben. Ob das hier beteiligt ist, ist ungemessen.

## Erste Trennung (billig, ohne neuen Kern)

1. **QED vom NewTOS-Desktop starten** statt aus TeraDesk. Geht es dort, liegt
   es am Startweg von TeraDesk; friert es auch dort ein, ist TeraDesk nur der
   Bote.
2. **Die unveraenderte `TERADESK.INF` zurueckspielen** und denselben Start
   wiederholen.

## Was vorher schon bekannt war

* TeraDesk startet selbst sauber (PORT03-Abnahme, `screendumps/tera-b-up.png`).
* QED startet und laeuft (PORT03: oeffnet eine gezogene Datei mit vollem
  Titel `C:\README.TXT`).
* Ein Start von TeraDesk **aus** und der Weg `shel_write` sind bisher
  **nirgends** abgenommen worden — [Start TeraDesk via the real
  mechanism](../docs/MEMORY.md) beschreibt den Weg, nicht seine Pruefung.

## Warum P1

Der Oktober-Termin verlangt: nichts darf haengen. Ein einfrierendes System
beim Start einer Anwendung ist der sichtbarste denkbare Fehler, und eine
Anwendung zu starten ist das Erste, was jemand ausprobiert.

## Auftrag

Erst die beiden Trennungen oben, dann messen, **wo** der Start stehenbleibt —
nicht raten. Der Weg ist `shel_write`/`Pexec`; Zeitstempel und der letzte
erreichte Punkt vor dem Stillstand sind die Groessen, die ihn eingrenzen.
Ein `[EXC]`/`[PANIC]` auf der seriellen Schnittstelle waere der Glueckfall —
der Anwender hat dort aber keine Aufzeichnung, also muss der Kernel selbst
sagen, wo er war.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
