# DESK31 — Reste des Dateiwerks: `d_dofileren`, `STILLDIR`, `desk_verify`, `fun_rebld_marked`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecken — Nebenbefunde DESK24/DESK25/DESK26

Abhaengigkeiten: [DESK24](DESK24.md), [DESK25](DESK25.md), [DESK26](DESK26.md) (erledigt)

## Die Stellen

1. **`d_dofileren` fragt nicht.** EmuTOS hat dort eine eigene
   `item_exists`/`get_new_name`-Schleife samt „Ziel loeschen, damit `Frename`
   gelingt" (`references/emutos/desk/deskdir.c:785-825`); unsere ist ein
   blankes `dos_rename`. Betrifft das Verschieben einer **Datei** auf
   demselben Laufwerk ueber eine gleichnamige.
2. **`STILLDIR` fehlt**: `dir_op`s `source_is_parent`-Zweig setzt nur
   `ok = false`, EmuTOS fragt `Skip | Abort` (`deskdir.c:999-1003`).
3. **`desk_verify` fehlt in `rebuild_window`** (`deskfun.c:140`) — ungeprueft,
   ob beobachtbar.
4. **`fun_rebld_marked` / `WN_REBUILD` gibt es nicht** (`deskmain.c:678`,
   `deskfun.c:150-167`).
5. **`illegal_op_msg` uneinheitlich:** DESK24 hat EmuTOS' Wortlaut
   („Illegal operation!", `desk_rsc.c:2497`) an **einer** Stelle eingefuehrt;
   die zwei aelteren (`deskfun.c:794`, `:1075`) sagen weiter
   „Operation not allowed.".
6. **`fun_desk2desk`/Papierkorb** behaelt den NewTOS-`fun_confirm`-Vorbau
   statt EmuTOS' `wants_to_delete_files()` (`deskfun.c:1049-1051`). Die
   **Zahl** der Rueckfragen stimmt seit DESK25, Wortlaut und *Remove*-Zweig
   nicht.

## Arbeit

Der Reihe nach, jede Stelle mit ihrer Fundstelle; 5 ist eine Zweizeilen-
Angleichung, 1 und 6 brauchen je einen Messlauf am Kartenabbild.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
