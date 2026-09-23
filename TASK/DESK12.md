# DESK12 — Baudatum: es gibt keine Konstante, der About-Kasten kann es nicht zeigen

Status: **OFFEN** · Prioritaet: **P3** · Typ: Werkzeugkette — Nebenbefund DESK01

Abhaengigkeiten: [DESK01](DESK01.md) (erledigt)

## Sachverhalt

Der About-Kasten (`kernel/src/desk/inf.rs` `do_aboutdesk`) zeigt Name,
Version, Maschine und den Unabhaengigkeitshinweis, **kein Baudatum** — es
gibt im ganzen Baum keine Konstante dafuer. `os_date` ist ein ausdruecklicher
1980-01-01-Platzhalter (`tos-api/src/bdos/process/loader.rs:806-809`), die
`Boot time`-Zeile des Startschirms ist die GEMDOS-Uhr. Eine davon als
Baudatum zu drucken waere unwahr.

EmuTOS hat das Datum im BIOS-`version[]` (`deskmain.c:1881` zeigt `DEVERSN`
darauf).

## Arbeit

Eine Baukonstante erzeugen (`kernel/build.rs`, z. B. Datum + kurzer
Git-Hash), mit Blick auf Reproduzierbarkeit (`SOURCE_DATE_EPOCH`
beruecksichtigen); `initinfo.rs` und der About-Kasten lesen sie aus derselben
Quelle wie Version und Hinweis. Danach die Zeile im Kasten und ggf. im
Startschirm.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
