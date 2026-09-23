# DESK13 — Reste aus DESK02: Kopier-/Ueberschreibruecksprache, `Supexec`, Datumstrenner, `_IDT`

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecken — Nebenbefunde DESK02

Abhaengigkeiten: [DESK02](DESK02.md) (erledigt)

## Die Stellen

1. **Kopier- und Ueberschreibruecksprache fehlen.** EmuTOS fragt in `d_doop`
   (`references/emutos/desk/deskdir.c:939`) und beim Ueberschreiben
   (`:325`); `kernel/src/desk/dir.rs` hat beides nicht und ueberschreibt
   immer (`Fcreate` kuerzt). Deshalb fehlen zwei Felder in
   `Set preferences` (DESK02 liess sie bewusst weg statt tote Knoepfe zu
   bauen).
2. **`Supexec` (XBIOS 0x26) ist nicht implementiert**
   (`tos-api/src/xbios/mod.rs:129`) — damit fehlen die `Default`-Knoepfe
   beider Formatgruppen, die EmuTOS ueber den `_IDT`-Keks fuellt
   (`desksupp.c:219`).
3. **Datumstrenner**: `win.rs` `format_fnode` druckt `MM-DD-YY`, EmuTOS nimmt
   an beiden Aufrufstellen `DATEFORM_SEP` = `/` (`desk/deskbind.h:64`).
4. **Der `_IDT`-Keks traegt `0x112F`** (24H, DMY, `/`) und widerspricht dem,
   was die Textansicht druckt (MDY, `-`) — Quelle der Wahrheit festlegen.
5. `check_alt_letter_key` ist auf der aktuellen Tastaturschicht wirkungslos
   (seit Task #8 vermerkt); eine der DESK11-Aufrufstellen liegt dadurch brach.

## Arbeit

1 und 2 je als eigener Schritt mit Test; 3/4 zusammen entscheiden (ein
Trenner, eine Quelle); 5 pruefen, ob die Tastaturschicht das inzwischen
kann.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
