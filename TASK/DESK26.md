# DESK26 — Infozeile bleibt nach `fun_rebld` auf der alten Auswahl stehen

Status: **OFFEN** · Prioritaet: **P4** · Typ: Defekt — Nebenbefund DESK13

Abhaengigkeiten: [A110](A110.md) (erledigt)

## Sachverhalt

Nach einem Loeschvorgang baut `fun_rebld` das Fenster neu; die Auswahl ist
damit weg, die Infozeile zeigt aber weiter `… 1 selected items.`
(sichtbar in `measurement-2026-09-23-desk13/screendumps/f-deleted.png`).

A110 hat `win_sinfo(handle, check_selected)` an EmuTOS' fuenf Aufrufstellen
gesetzt; `fun_rebld` ist offenbar keine davon oder ruft es mit dem falschen
Argument. Zu klaeren an der Quelle (`desk/deskfun.c` `fun_rebld`,
`desk/deskwin.c:867-906`), welcher der beiden Faelle es ist.

## Arbeit

Quelle lesen, die fehlende Aktualisierung nachziehen, Screendump vor/nach.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
