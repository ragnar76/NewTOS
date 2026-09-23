# DESK01 — Desk → About (Desktop-Info): Versionsdialog

Status: **OFFEN** · Prioritaet: **P2** · Typ: Funktionsluecke desk/

Abhaengigkeiten: [DESK08](DESK08.md) (`form_center`/`form_dial`) — Dialog zentriert sich sonst nicht

## Sachverhalt

`kernel/src/desk/main.rs` `hndl_menu`: `Item::About` hat keinen Handler
(`_ => {}`). EmuTOS zeigt den Desktop-Info-Dialog ueber `do_aboutdesk`
(`references/emutos/desk/deskmain.c`, Baum `ADDINFO` in `desk/desk_rsc.c`)
mit Version, Datum und GPL-Hinweis.

## Arbeit

Quelle lesen (`do_aboutdesk`, `ADDINFO`-Baum, Textzeilen), Dialog ueber
unsere `form_do`-Kette zeigen (Muster: die vorhandenen desk/-Dialoge in
`dlg.rs`/`inf.rs`), Texte auf NewTOS anpassen (Name, Version, Datum aus dem
Build). Hosttest auf den Baumaufbau, Screendump gegen den EmuTOS-Dialog
(Hatari-Falcon: Desktop-Info ist per Tastatur nicht erreichbar → Injektor
oder Vergleich gegen `rscview`).

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
