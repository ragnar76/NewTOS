# KEYS01-SEQUENCE-CHECK -- UP halten, RIGHT dazu, RIGHT loslassen

Datum 2026-10-03, HEAD 436919f. Messung: `measurements/measurement-2026-10-03-keyseq/` (RUNINFO.TXT dort).

**Verdikt: gleiches Verhalten.** NewTOS (KEYS01) und EmuTOS verhalten sich bei dieser
Tastenfolge gleich. Das gemeldete Verhalten (nach dem Loslassen von RIGHT laeuft der
Spieler nicht wieder los) ist EmuTOS-konform und kein KEYS01-Fehler. Beheben laesst es
sich nur im Tastaturvektor (K4, Break-Ereignisse an die Anwendung), nicht in der Wiederholung.

## Ablauf (beide Systeme, gleiche K06-Instrumentierung in doomgeneric_gem.c)
Folge: UP down (t=0), RIGHT down (1,5 s), RIGHT up (3 s), UP up (5 s).

| Ereignis | A NewTOS, QEMU raspi4b | B EmuTOS 1.4, Hatari TT-low |
|---|---|---|
| UP PRESS | 36040 ms | 16815 ms |
| UP-Wiederholungen | 31 (erste nach 295 ms, dann 40 ms) | 19 (bis 18155; Doom liest gepuffert, ~2 fps) |
| RIGHT PRESS | 37530 ms | 18555 ms |
| UP RELEASE | 37635 ms (RIGHT +105) | 18610 ms (RIGHT +55) |
| RIGHT-Wiederholungen | 31, enden mit dem Break (letztes Wort 7804, Break 7805 hz_200) | 18 bis 19850 ms |
| RIGHT RELEASE | 39150 ms (~110 ms nach dem Break) | 20255 ms (Lesetakt) |
| UP-Worte nach RIGHT-Make | 0 | 0 |
| UP wieder gedrueckt nach RIGHT-Break | nein | nein |

Quelle der Regel, `references/emutos/bios/ikbd.c`:
* `:399-423` Kommentar: nur die zuletzt gedrueckte Taste steht in `kb_last`; ein Break einer
  Nicht-Modifier-Taste stoppt die Wiederholung (`kb_ticks = 0`).
* `:875` `kb_last_actual = scancode` und `:891` `kb_ticks = kb_initial` bei jedem Make: RIGHT ersetzt UP.
* `:816-817` Break: `if (scancode_only == kb_last_actual) kb_ticks = 0;` -- der Break von RIGHT
  stoppt; der Break von UP (nicht `kb_last_actual`) aendert nichts.
* `do_key_repeat` (`:451-527`) liest nur `kb_last`; es gibt keine Liste gehaltener Tasten, UP wird
  nie wieder aufgenommen. Das Hatari-Log bestaetigt es (kein UP-Wort mehr nach 18155 ms).
NewTOS: `kernel/src/key_repeat.rs` (Zustand `kb_last`/`kb_last_actual`) und der Lauf A zeigen
dieselbe Folge.

## Unterschiede
Keine im Verhalten. Zeitlich: Hatari-Doom liest wegen ~2 fps in Stuessen (Zeitstempel = Lesezeit,
20/19 statt 32/32 Worte sichtbar), die Reihenfolge und das Ende sind identisch. UP wird von Doom
in beiden Faellen ~0,1 s nach dem ersten RIGHT-Wort freigegeben, weil Doom ohne Wiederholung nach
`hold_repeat_ms` (100 ms) auf "losgelassen" schliesst (doomgeneric_gem.c key_poll).

## Grenzen
QEMU hat keine USB-Tastatur: A speist den IKBD-Strom wie `hid.rs` (ein Make, ein Break) ein; der
echte USB-Pfad bleibt Pi-Sache. Der EmuTOS-Lauf ist TT-low, nicht Falcon.
