# DESK43 — Der Desktop muss im User-Space laufen, wie jede andere Anwendung

Status: **OFFEN** · Prioritaet: **P2** · Typ: Architektur — Anwenderentscheidung 2026-08-31, geschaerft 2026-09-01

Abhaengigkeiten: Plan liegt: `docs/analysis/desktop-userspace-plan.md` (68 KB, 2026-09-01)

## Warum dieses Ticket ueberhaupt existiert

Die Entscheidung ist **ein Jahr alt in Projektzeit und nie eingeplant worden**,
weil es kein Ticket gab — nur einen Gedaechtniseintrag und einen fertigen Plan.
Der Anwender hat am 2026-09-25 von selbst wieder danach gefragt: *„aber wieso
ist unser Desktop nicht auf einer EL0-Anwendung? Das sollte eigentlich so
sein."*

## Ist-Zustand

`desk::run()` haengt direkt im Boot-Pfad (`kernel/src/lib.rs:329`);
`desk/mod.rs:159` nennt sich selbst *„the desktop's kernel-boot context"*.
Der Desktop ist Kernel-Code mit Oberflaeche, kein Prozess.

Gemessener Ausgangspunkt (2026-09-01): `kernel/src/desk/` = **9 234 Zeilen in
15 Dateien**; nur **sechs** direkte `crate::`-Abhaengigkeiten zum Kernel, aber
rund **40 direkte `tos_api::`-Aufrufe am Trap vorbei** (`aes::object` 9,
`bdos::fs` 9, `vdi::font` 6, `host::host` 6, `aes::window` 6, `bios::drive` 3,
`vdi::mouse` 1). Das sind die Stellen, die der Umzug geradezieht.

## Was sich damit von selbst aufloest

In EmuTOS **ist** der Desktop der AES-Prozess `rlr`, und eine gestartete
Anwendung uebernimmt **denselben** Deskriptor (`sh_ldapp` ruft
`p_nameit(rlr, fname)`, `aes/gemshlib.c:622-634` — kein `pstart`/`getpd`).
Daher misst EmuTOS `ap_id = 0` fuer die gestartete Anwendung.

Zwei gemessene Abweichungen sind **reine Symptome** davon und duerfen nicht
vorher einzeln „behoben" werden — das waere eine hingeschriebene Zahl statt
nachgebautem Verhalten:
* `ap_id` 1 statt 0 (D-R1-5)
* `ap_private` aus dem Fallback statt der DESKTOP-Ressource (D-R1-6)

Ebenso faellt unser `save_windows`/`restore_windows`-Handoff: EmuTOS'
`sh_ldapp` schliesst **keine** Fenster. Der Handoff existiert nur, weil unser
Desktop strukturell etwas anderes ist.

**Nebeneffekt, der uns nuetzt:** der Umzug beendet den In-Kernel-Zeichenpfad,
wegen dem ein gut aussehender Desktop den EL0-Anwendungspfad bisher **nicht**
validiert.

## Beruehrungspunkt mit [PORT05](PORT05.md)

PORT05 (QED aus TeraDesk gestartet friert ein) ist der erste Fehler, der
**wegen** dieser Struktur auffaellt: der Weg „EL0-Anwendung startet
EL0-Anwendung" ist bei uns nie gegangen worden, weil der Desktop ihn nicht
geht. **PORT05 wartet aber nicht auf dieses Ticket** — dieser Weg muss
funktionieren, ganz gleich wo der Desktop lebt, und vor dem Oktober-Termin ist
der Umzug zu gross. Erst PORT05, dann hier weiter.

## Die wichtigste Randbedingung

**Es ist der dritte Anlauf.** Zwei Rebuilds sind vorher abgedriftet. Jeder
Plan muss beantworten, **was den dritten Abdrift verhindert** — sonst ist er
nicht fertig.

## Auftrag

`docs/analysis/desktop-userspace-plan.md` gegen den heutigen Stand pruefen
(er ist vom 2026-09-01, seitdem sind DESK01–DESK42 gelaufen), dann in
Arbeitseinheiten schneiden, die einzeln abnehmbar sind. Keine Umsetzung, bevor
der Schnitt steht und der Anwender ihn gesehen hat.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
