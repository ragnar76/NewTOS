# DESK34 — Ein Unterverzeichnis oeffnet ein neues Fenster statt im selben zu navigieren

Status: **OFFEN** · Prioritaet: **P1** · Typ: Belegter Defekt — Pi-Runde 2026-09-24

Abhaengigkeiten: —

## Sachverhalt

Auf der echten Pi 4 (Karte C:–F:, Stand `25396fa`) oeffnet ein Doppelklick auf
einen **Ordner in einem Fenster** jedes Mal ein **neues** Fenster. Erwartet ist
Navigation im selben Fenster.

EmuTOS entscheidet das in `do_aopen` (`references/emutos/desk/desksupp.c`):

```c
if (allow_new_win)
{
    graf_mkstate(&junk, &junk, &junk, &keystate);
    if ((keystate & MODE_SCA) == MODE_ALT)
        new_win = TRUE;             /* :1130-1135 */
}

if (new_win)
{
    pw = win_alloc(curr);           /* :1141-1149 */
    ...
}
else
{
    pn_close(&pw->w_pnode);         /* :1151-1154  <- derselbe Knoten wird neu belegt */
}
do_diropen(pw, new_win, curr, app_path, &t, TRUE);
```

Ein **neues** Fenster gibt es also nur in drei Faellen: beim Oeffnen von der
Schreibtischflaeche (Laufwerkssymbol — eigener Aufrufer, `:1067`, uebergibt
`TRUE`), bei einer Desktop-Verknuepfung (`WN_DESKTOP`, `:1112-1127`,
`CONF_WITH_DESKTOP_SHORTCUTS`) und wenn beim Oeffnen **allein Alt** gehalten
wird. Sonst `pn_close` + Wiederverwendung.

## Arbeit

`do_aopen`s Fallunterscheidung in `kernel/src/desk/main.rs` nachziehen —
`allow_new_win`, die `graf_mkstate`-Abfrage der Umschalttasten und den
`pn_close`-Zweig. Vorher am Quelltext feststellen, welcher unserer Aufrufer
heute welchen Wert setzt; der Laufwerkspfad muss weiterhin ein neues Fenster
bekommen.

Abnahme: Ordner in einem Fenster oeffnen → derselbe Fensterrahmen, neuer
Pfad im Titel; Laufwerkssymbol oeffnen → neues Fenster; Alt+Doppelklick auf
einen Ordner → neues Fenster. Die Alt-Probe braucht eine echte Tastatur.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
