# GUI04 — `CONF_WITH_EXTENDED_MOUSE`: Mausrad und Tasten 3 bis 5

Status: **OFFEN** · Prioritaet: **P2** · Typ: Funktionsluecke USB/AES + **benannte Erweiterung ueber EmuTOS hinaus** — aus der Schaltertabelle (REF01)

Abhaengigkeiten: —

## Sachverhalt

`CONF_WITH_EXTENDED_MOUSE` ist im Referenzbau **1**
(`references/emutos/include/config.h:1491-1498`): EmuTOS wertet die
IKBD-Scancode-Erweiterung fuer Maustasten 3/4/5 und das **Mausrad** aus
(`references/emutos/bios/aciavecs.S`, `bios/ikbd.c:748`).

`kernel/src/ikbd.rs` entschluesselt nur das Standardpaket (relative Bewegung,
zwei Tasten); `0xF6` gilt dort als Statusbericht-Kopf, eine Rad- oder
Zusatztastenauswertung gibt es nicht.

**Der Punkt, der es interessant macht:** die VDI-Seite ist schon da —
`vex_wheelv` (`tos-api/src/vdi/mouse.rs:710`) kann einen Radvektor
installieren. Es fuettert ihn nur niemand. Ein Programm, das sich dort
eintraegt, wartet bei uns vergeblich.

## Hochgestuft — Lead 2026-09-24

Von P3 auf **P2**: der Benutzer will das Mausrad. Es ist die einzige der fuenf
Luecken aus der Schaltertabelle, die man an einer Vorfuehrung sofort
bemerkt — und die halbe Strecke liegt schon da (`vex_wheelv`).

## Neuzuschnitt 2026-09-25 — die Quelle ist USB, und EmuTOS ist hier nicht fertig

**Der Weg ist USB, nicht IKBD.** Der Anwender: *„natuerlich eine USB-Maus,
eine Atari-Maus haette kein Mausrad."* Die IKBD-Erweiterung bleibt trotzdem
die Form, in der das Rad durch unser System laeuft — `hid.rs:106` uebersetzt
heute schon jeden USB-Mausbericht in ein IKBD-Paket
(`0xF8|Tasten, dx, dy`). Es liest dabei nur die Bytes 0–2; **Byte 3, das
Rad, bleibt liegen.**

**EmuTOS' eigene Radbehandlung ist auskommentiert.** Die Kette dort:
IKBD-Scancode (`bios/ikbd.c:748-760`: `0x59`/`0x5a` hoch/runter, `0x5c`/`0x5d`
links/rechts) → `mousexvec` → AES → `wheel_change` (`aes/geminput.c:395`).
Und dort steht:

```c
/* We have a problem here.
 * This function is called by forker(), where rlr is set to -1.
 * As a result, we can't call ap_sendmsg() which internally calls iasync().
 * Uncomment the following code when some solution has been found.
 */
/*  ap_sendmsg(appl_msg, WM_ARROWED, D.w_win[wh].w_owner, wh, type, 0, 0, 0);  */
```

EmuTOS nimmt das Rad entgegen, rechnet `WA_UPLINE`/`WA_DNLINE` aus — und
wirft es weg. Ein 1:1-Nachbau liefert ein Mausrad, das nichts tut.

**Lead-Entscheidung des Anwenders 2026-09-25: das Rad soll scrollen.** Also
ueber EmuTOS hinaus, und das ist hier ausdruecklich **keine** Drift im Sinne
von [Don't rationalize EmuTOS deviations] — EmuTOS selbst schreibt in den
Quelltext, dass hier eine Loesung fehlt. Wir bauen sie.

## Was bei uns fehlt (gelesen, nicht vermutet)

| Stueck | Stand |
|---|---|
| USB-Bericht Byte 3 (Rad) | `kernel/src/drivers/usb/hid.rs:106` liest `r[0..3]`, Byte 3 ungenutzt |
| IKBD-Erweiterung (Scancodes 0x37/0x59–0x5f) | `kernel/src/ikbd.rs` kennt sie nicht; `0xF6` gilt dort als Statusbericht-Kopf |
| `mousexvec` | fehlt |
| `wheel_change` | fehlt |
| AES traegt sich per `vex_wheelv` ein | fehlt (`aes/gemgsxif.c:291` ist das Vorbild) |
| `vex_wheelv` (Opcode 134) | **vorhanden**, `tos-api/src/vdi/mouse.rs:711` — niemand ruft es, niemand traegt sich ein |

## Auftrag

1. **Die Kette in EmuTOS' Form bauen**, Stueck fuer Stueck mit Zitat:
   USB-Byte 3 → IKBD-Scancode `0x59`/`0x5a` (und `0x5c`/`0x5d`, falls die Maus
   waagerecht liefert) → `mousexvec` → `wheel_change` mit `WA_UPLINE`/
   `WA_DNLINE`. So laeuft spaeter eine echte Rad-Quelle ueber denselben Weg,
   und `vex_wheelv` wird endlich bedient.
2. **Den letzten Schritt loesen**, den EmuTOS offengelassen hat: `WM_ARROWED`
   muss beim Fenster unter dem Mauszeiger ankommen. EmuTOS' Grund ist
   benannt (`forker()`, `rlr == -1`, `ap_sendmsg` ruft `iasync`); bei uns
   sieht der Weg vom Eingabepfad zur Nachrichtenzustellung anders aus —
   **erst pruefen, ob dieselbe Einschraenkung bei uns ueberhaupt gilt**, und
   das Ergebnis aufschreiben. Falls ja, ist eine andere Zustellform zu
   entwerfen und zu begruenden.
3. Die Abweichung **benennen**: ein Kommentar an `wheel_change`, der sagt,
   was EmuTOS an dieser Stelle tut, warum wir weitergehen und wie.

## Abnahme

Auf der **echten Pi** mit der USB-Maus: im Desktopfenster mit dem Rad
scrollen, Inhalt bewegt sich. Dazu ein Testprogramm, das sich per
`vex_wheelv` eintraegt und die Radereignisse sieht — das ist der Beleg, dass
die Kette und nicht nur der Desktop bedient wird. Menue aktiv ⇒ Rad wird
ignoriert (`geminput.c:401-402`).

## Arbeit

Die Scancode-Erweiterung in `ikbd.rs` nachziehen und an `vex_wheelv` binden.
Abnahme braucht echte Hardware mit Rad-Maus; in QEMU ist der Pfad nur bis zur
Dekodierung pruefbar.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
