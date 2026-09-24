# GUI02 — `CONF_WITH_VIEWER_SUPPORT`: Standardbetrachter fuer nicht zugeordnete Dateien

Status: **OFFEN** · Prioritaet: **P3** · Typ: Funktionsluecke AES/Desktop — aus der Schaltertabelle (REF01)

Abhaengigkeiten: [GUI01](GUI01.md)

## Sachverhalt

`CONF_WITH_VIEWER_SUPPORT` ist im Referenzbau **1**
(`references/emutos/include/config.h:1854-1861`): eine Datei ohne passende
Anwendungszuordnung wird an einen eingetragenen **Standardbetrachter**
uebergeben.

Unser eigener Quelltext sagt, dass es das nicht gibt —
`kernel/src/desk/inf.rs:1847-1850`: *„AF_VIEWER is CONF_WITH_VIEWER_SUPPORT,
which this desktop does not implement."* Die `AF_VIEWER`-Flagge wird beim
Einlesen der `NEWDESK.INF` also gelesen und ignoriert.

## Arbeit

`AF_VIEWER` in der Zuordnungsliste auswerten und den Startweg bauen. Reihenfolge
gegen [GUI01](GUI01.md) an der Quelle klaeren: eingebaute Ansicht gegen
externen Betrachter — welcher gewinnt, und was passiert, wenn der eingetragene
Betrachter fehlt.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
