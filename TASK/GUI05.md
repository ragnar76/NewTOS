# GUI05 — `CONF_WITH_ALT_DESKTOP_GRAPHICS`: linksbuendige Dialogtitel mit Linie

Status: **OFFEN** · Prioritaet: **P4** · Typ: Konformitaet AES/Desktop — aus der Schaltertabelle (REF01)

Abhaengigkeiten: —

## Sachverhalt

`CONF_WITH_ALT_DESKTOP_GRAPHICS` ist im Referenzbau **1**
(`references/emutos/include/config.h:1734-1741`): die Desktop-Dialoge richten
ihren **Titel linksbuendig** aus und zeichnen darunter eine **Linie**
(`references/emutos/desk/deskmain.c:1727-1739`).

Unsere Dialoge zentrieren den Titel (z. B. `inf::do_aboutdesk`s `ctr(&title)`)
und haben keine Linie. Das betrifft **alle** in DESK01/02/03/05 gebauten
Kaesten.

## Arbeit

Die Quelle lesen und entscheiden, ob es eine Konformitaetsluecke oder eine
bewusste Abweichung sein soll — der 3D-Look (DESK01b) kam aus derselben Ecke
und wurde damals ausdruecklich EmuTOS-konform gemacht. Wenn konform: die
Titelzeile aller sechs Dialoge umstellen, Screendump vor/nach je Dialog.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
