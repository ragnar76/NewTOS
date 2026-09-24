# DESK35 — Beim Start einer GEM-Anwendung bleiben die Desktop-Fenster stehen und frieren ein

Status: **OFFEN** · Prioritaet: **P1** · Typ: Belegter Defekt — Pi-Runde 2026-09-24

Abhaengigkeiten: [DESK06](DESK06.md) (erledigt), [DESK14](DESK14.md) §4 (zurueckgestellt)

## Sachverhalt

Auf der echten Pi 4 bleiben beim Start von QED oder TeraDesk die offenen
Desktop-Fenster auf dem Schirm stehen. Anklicken laesst sich alles, aber
nichts reagiert — sie sind gezeichnete Pixel ohne laufende Ereignisschleife.

Das widerspricht der Projektentscheidung „eine gestartete Anwendung besitzt
den **ganzen** Schirm" und sieht nach einem haengenden System aus.

EmuTOS' Stelle ist `sh_main` (`references/emutos/aes/gemshlib.c:710-720`),
**vor** jedem Programmlauf:

```c
sh_chdef(psh);
sh_chgrf(psh);
if (gl_shgem)
{
    wm_init();                  /* Fenstertabelle zuruecksetzen */
    ratinit();
    sh_draw(D.s_cmd, TRUE);     /* clear the screen */
}
rc = sh_ldapp(psh);             /* jetzt erst laeuft das Programm */
```

DESK06 hat den verschachtelten Lauf gebaut und `wm_init()` + `sh_draw(…,
TRUE)` **bewusst ausgelassen** (Luecke G2), weil der Desktop danach ohnehin
neu zeichnet — das stimmte fuer die **Konsole**, deren Textschirm alles
ueberdeckt. Fuer eine GEM-Anwendung stimmt es nicht: sie zeichnet nur in ihre
eigenen Rechtecke, und der Rest des Schirms bleibt stehen.

## Abgrenzung — was dieses Ticket NICHT ist

Nicht der vollstaendige Desktop-Neustartpfad. Der ist [DESK14](DESK14.md) §4
und bleibt zurueckgestellt. Hier geht es um die zwei Schritte **vor** dem
Programmlauf, die EmuTOS auch im verschachtelten Fall braeuchte.

## Arbeit

Erst am Quelltext klaeren, was `wm_init` und `sh_draw` in unserem
verschachtelten Lauf jeweils anrichten wuerden (DESK06s Begruendung fuer das
Auslassen steht in `kernel/src/shell_loop.rs` und ist zu pruefen, nicht zu
uebernehmen) — insbesondere, ob die AES-Fenstertabelle danach noch den
Zustand traegt, den der zurueckkehrende Desktop erwartet, oder ob dafuer
`cnx_put`/`cnx_get` einspringen muessen. Dann den Schirm vor dem Start
raeumen und nach der Rueckkehr wiederherstellen.

Abnahme auf der Pi: zwei Fenster offen, QED starten → **kein** Fensterrest
sichtbar, QED besitzt den Schirm; QED beenden → Desktop mit seinen zwei
Fenstern zurueck. Dasselbe mit TeraDesk.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
