# M04 — Quittung

**Status: DONE** · 2026-09-09 · Laufmanifest und gemessenes Kernelabbild
eindeutig verbinden

## Auftrag

Nur die Zuordnung des tatsächlich gestarteten Kernels zum Messlauf entwerfen:
Abbild-Hash, Build mit Dirty-State, Dumper-Hash, Tabelle, Bootkontext. Kein
neues Cookie ohne Prüfung des bestehenden Vertrags. Implementierung ausdrücklich
erst als Nachfolgetask.

## Was ich gemacht habe

Quellen gelesen statt geschätzt:

- `tosdump/core/tosdump.h:24-36` — Kopf-Layout, `BUILDID`, `CASECRC`,
  12 reservierte Bytes
- `tosdump/core/record.c:172-174` und `host/tosdiff.c:40` — die reservierten
  Bytes werden erzwungen null
- `kernel/src/lib.rs:713-762` — die Uhrquelle
- `kernel/src/lib.rs:284` und `tos-api/src/cookie.rs:11-13,34-37` — was der
  `NTOS`-Cookie trägt und was ein Slot überhaupt fassen kann
- `kernel/build.rs` — was heute eingebacken wird
- `kernel/src/arch/aarch64/link-rpi4.ld:67,82` — Abbildgrenzen
- `references/freemint/sys/ssystem.c:130-139,572-576` und
  `docs/api/gemdos/154-ssystem.md:95-143` — die Transportgestalt

Ergebnis als Konzept in `M04.md`, Abschnitte 1–8.

## Ergebnis

Abnahme erfüllt. Das Konzept benennt Format, Transport, Ablage im Dump und
sagt, welche Identität der laufende Kernel selbst liefern kann.

Drei Befunde, die die Aufgabe verändert haben:

1. **Der Kernel liefert heute keine Identität.** Der `NTOS`-Cookie trägt ein
   hartkodiertes `0x0000_0100`, also eine statische API-Version, kein
   Baumerkmal. `build.rs` backt weder Git-Hash noch Dirty-Zustand noch Bauzeit
   ein. Identität muss erst erzeugt werden.
2. **Ein Cookie kann den Zweck nicht tragen.** Ein Slot ist
   `{ tag: u32, value: u32 }`, die Dose ist kernel-eigen und gibt EL0 keinen
   Zeiger. Mehr als 32 Bit sind darüber nicht zu holen. Damit ist die
   Vertragsprüfung erledigt und kein neues Cookie entstanden. Empfohlener Weg
   ist `Ssystem` in der `S_KNAME`-Gestalt — Puffer plus Länge; die Spec führt
   diese Modi bereits als zurückgestellt und nennt die Entscheidung
   ausdrücklich offen.
3. **Der Zeitstempelbeleg ist schwächer als beschrieben.** `RUNINFO.TXT` und
   `measurements/measurement-2026-09-09/README.md` sagen „Compile-Zeit des Kernels". Der
   Code setzt die Uhr aus dem **FAT-Verzeichniszeitstempel von
   `C:\KERNEL8.IMG`** — Metadatum einer Datei, von jedem Kopiervorgang neu
   gesetzt, 2-Sekunden-Auflösung, vom Host frei beschreibbar. Er bleibt Indiz,
   wie die Abnahme es verlangt.

Die Ablage im Dump geht in die 12 reservierten Kopf-Bytes und ist damit
Formatversion 3, kein stiller Zusatz. Kein vorhandener Dump wird berührt, keine
Fall-CRC ändert sich.

## Offen

Absichtlich nicht getan, weil nicht beauftragt: keine Implementierung, kein
Formatwechsel auf v3, keine Änderung an `build.rs`, `cookie.rs` oder
`ssystem.rs`.

Vor der Implementierung zu klären (in `M04.md` §7): ob der Abbildbereich
byteweise `kernel8.img` entspricht, ob vor dem Hashzeitpunkt hineingeschrieben
wird, was das Hashen von ~779 KB beim Boot kostet.

Nicht nachgeführt: die Uhrquellen-Beschreibung in `RUNINFO.TXT` und
`measurements/measurement-2026-09-09/README.md`. Beides sind Laufbelege; nachträgliches
Ändern wäre gegen die Arbeitsregeln. Eine Korrektur gehört in einen eigenen
Task, wenn sie gewünscht ist.
