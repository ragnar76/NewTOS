# ACC04 — Desk-Menue: registrierte Titel, `AC_OPEN`/`AC_CLOSE`

Status: **OFFEN** · Prioritaet: **P2** · Typ: Funktionsluecke

Abhaengigkeiten: [ACC01](ACC01.md), [ACC03](ACC03.md)

## Sachverhalt

`menu_register` (35) legt bei uns Titel und `ap_id` ab
(`tos-api/src/aes/menu.rs`), mehr nicht: die Titel erscheinen in keinem
Menue, und niemand postet `AC_OPEN`. Der Kommentar im Code nennt das
ausdruecklich als offenen Punkt. EmuTOS traegt die Titel unter `THEDESK`
ein und postet beim Anklicken `AC_OPEN` an den Besitzer; `AC_CLOSE` geht an
alle Accessories, wenn eine Anwendung startet bzw. endet (`mn_cleanup`
`aes/gemmnlib.c:740-760`, Aufrufer `ap_exit` `gemaplib.c:254`, `sh_ldapp`
`gemshlib.c:652`). Genaue Woerter und Fundstellen liefert ACC01.

## Arbeit

1. Registrierte Titel in den `THEDESK`-Teilbaum des jeweils installierten
   Menuebaums eintragen (auch wenn die Anwendung ihren eigenen Baum
   installiert — EmuTOS' Regel aus ACC01 uebernehmen).
2. Klick auf einen Accessory-Titel → `AC_OPEN` mit dem Menue-Index an den
   Besitzerprozess.
3. `AC_CLOSE` an alle beim Programmstart und -ende (unser Sweep-Punkt aus
   A107 ist der Ort fuer das Ende).
4. Kernel-Desktop (`kernel/src/desk/menu.rs`): Platz fuer die Titel unter
   *Desk*, wie EmuTOS' Desktop es baut (`desk/desk_rsc.c`, `deskmain.c`).

## Abnahme

Hosttest: zwei registrierte Accessories erscheinen unter *Desk*, Klick
liefert `AC_OPEN` an den richtigen Slot, Programmstart liefert `AC_CLOSE` an
beide. QEMU: Titel sichtbar, Klick oeffnet das Testaccessory-Fenster.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
