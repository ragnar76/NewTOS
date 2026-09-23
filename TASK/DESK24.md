# DESK24 — Ordner-Namenskonflikt fragt nicht (`output_path`/`get_new_name`)

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke — Nebenbefund DESK13

Abhaengigkeiten: [DESK13](DESK13.md) (erledigt)

## Sachverhalt

DESK13 hat den **Datei**-Namenskonflikt gebaut (`ADCPALER` in
`output_fname`). Der **Ordner**-Konflikt ist der andere Zweig: EmuTOS fragt
in `output_path`/`get_new_name` (`references/emutos/desk/deskdir.c:700-779`)
nach einem neuen *Ordner*namen, wenn der Zielordner schon existiert.

NewTOS mischt stattdessen weiter hinein — der Inhalt landet im bestehenden
Ordner, ohne Rueckfrage.

## Arbeit

`output_path`/`get_new_name` nachbauen, Dialog wie beim Dateikonflikt ueber
die DESK08-Kette; Abnahme: ein Ordner mit demselben Namen im Ziel, beide
Antworten am Kartenabbild nachgewiesen.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
