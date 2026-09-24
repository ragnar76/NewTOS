# DESK37 — Hinter einer laufenden GEM-Anwendung steht Weiss statt des Desktop-Hintergrunds

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt — Pi-Runde + Hatari-Gegenprobe 2026-09-24

Abhaengigkeiten: [DESK35](DESK35.md) (erledigt)

## Sachverhalt

DESK35 raeumt den Schirm vor dem Start einer GEM-Anwendung. Er bleibt danach
**weiss**. EmuTOS zeigt dort den **Desktop-Hintergrund** (bei unserer
`#Q`-Einstellung gruen) samt Menueleiste.

**Gemessen, nicht geschlossen** (`measurement-2026-09-24-hatari-appbg/`):
Hatari 2.6.1, Falcon/VGA, EmuTOS 1.4.0+ `etos512us.img`, Start ueber die
`#Z`-Autorun-Zeile (also ueber den Desktop-Startweg). Zwei Laeufe, beide mit
gruenem Hintergrund:

* ein minimales m68k-GEM-Programm, das nur ein Fenster oeffnet und **kein**
  `v_clrwk` macht — Hintergrund gruen, Menueleiste `BGWIN.PRG`;
* das echte m68k-QED — Hintergrund gruen, QEDs eigene Menueleiste.

## Die Stelle

Zwei Schritte, nicht einer:

```
sh_main   gemshlib.c:715-720   if (gl_shgem) { wm_init(); ratinit();
                               sh_draw(D.s_cmd, TRUE); }      <- weiss loeschen
sh_ldapp  gemshlib.c:633       sh_show(fname);                <- und DANN das
sh_show   gemshlib.c:291-297   sh_draw(lcmd, FALSE)
sh_draw   gemshlib.c:280-288   ted->te_ptext = lcmd;
                               ob_draw(tree, ROOT, MAX_DEPTH)
```

`sh_draw(…, TRUE)` setzt `tree[ROOT].ob_spec = CLEAR_SCREEN` (weiss, `:64`)
und zeichnet **nur** die Wurzel; `sh_show(fname)` zeichnet danach den
**ganzen** Baum mit dem urspruenglichen `ob_spec` — Hintergrundmuster und
Menueleiste, in der der Programmname steht.

NewTOS macht seit DESK35 den ersten Schritt und nicht den zweiten.

**Merke fuer die Diagnose:** `CLEAR_SCREEN`s Kommentar „white desktop screen"
ist wahr und trotzdem irrefuehrend — er beschreibt einen Zwischenzustand von
Millisekunden. Genau daran bin ich zuerst haengengeblieben.

## Arbeit

`sh_show`/`sh_draw`s `FALSE`-Zweig nachziehen und an EmuTOS' Stelle rufen —
in `kernel/src/shell_loop.rs` bzw. `desk::main::desktop_relinquish`, direkt
nach dem Loeschen. Vorher klaeren, woher unser Gegenstueck zu
`rs_trees[DESKTOP]` seinen `ob_spec` nimmt: die `#Q`-Farbe fuehrt der Desktop
in `g_patcol` (DESK09), und der ist zu dem Zeitpunkt gerade dabei, die AES
abzugeben — es muss also die **AES**-Seite sein, die das Muster haelt.

Ob die Menueleiste den Programmnamen zeigt, ist Teil desselben Schrittes
(`ted->te_ptext = lcmd`) und gehoert mit gemessen.

Abnahme: QED aus dem Desktop starten, Hintergrund **gruen**; Vergleichsbild
`measurement-2026-09-24-hatari-appbg/screendumps/emutos-qed-falcon-tc.png`.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
