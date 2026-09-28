# PORT08 — Die Ball-Demo parkt beim Beenden in einem AES-Aufruf

Status: **OFFEN** · Prioritaet: **P2** · Typ: Belegter Defekt — Pi-Lauf 2026-09-28

Abhaengigkeiten: [PORT07](PORT07.md) (dort als ungeprueft benannt)

## Der Befund

Echte Pi 4, Kern `09f3e928`, 1920x1080. Die konforme Ball-Demo laeuft, liefert
ihre Messung — und **laesst sich nicht beenden**.

**Anwender auf die Rueckfrage: das Bild blieb stehen, die Maus lebte noch.**

Nach der Projektregel [[mouse-moves-but-nothing-responds-means-app-sleeping]]
heisst das: **die Anwendung schlaeft in einem AES-Aufruf.** `pump_input` laeuft
in der AES-Warteschleife, die Maus lebt also gerade **weil** die AES weiter
bedient wird. Es ist damit **kein** Kernel-Haenger wie [PORT05](PORT05.md) oder
[DESK46](DESK46.md) — dort war die Maus jedes Mal tot.

## Der zweite Beleg: die Logdatei bricht mitten im Wort ab

`C:\BALL.LOG`, gesichert als
`measurement-2026-09-28-port07/raw/pi01-BALL.LOG`: **1755 Byte, und das letzte
Zeichen ist das `l` von `cal`** — kein Zeilenende, kein Abschluss, mitten im
letzten Satz:

```
… one page over vro_cpyfm -- no XBIOS screen-base cal
```

`plat_close()` lief also weit genug, um die Zusammenfassung zusammenzustellen
**und mit dem Schreiben zu beginnen**, und hoerte mittendrin auf.

## Damit ist der Suchraum fuenf Aufrufe gross

Der Ausstieg von `plat_close()`, in seiner Reihenfolge:

1. `Fwrite` der Zusammenfassung — **hier bricht der Text ab**
2. `Fclose`
3. Palettenwiederherstellung (`vq_color`/`vs_color`)
4. `v_clsvwk`
5. `form_dial(FMD_FINISH)` und `appl_exit`

**Welcher es ist, ist ungemessen.** Die Regel dazu ist benannt: beim naechsten
Lauf den **letzten AES-Aufruf samt seinen Wire-Argumenten** feststellen, bevor
irgendwer eine Schicht verdaechtigt — eine tote Prozessebene zu vermuten war in
diesem Projekt schon mehrfach falsch.

## Wo der Fehler liegen koennte — und warum das offen bleibt

Er kann **in der Demo** stecken (`plat-newtos.c`, ausserhalb des Repos) oder
**bei uns**. Das ist ein echter Unterschied: im ersten Fall kostet die Reparatur
nichts am Kernel und ist risikofrei vor dem Termin, im zweiten nicht.

Nicht als Diagnose, sondern als Nachbarschaft: `appl_exit` ist die Stelle, die
[ACC10](ACC10.md) am 2026-09-26 repariert hat, und auf der Karte sind
Accessories geladen. Ob das hier beteiligt ist, ist **ungemessen** — ACC10s
Abnahme lief headless und ohne GEM-Anwendung davor.

## Auftrag

1. **Headless nachstellen.** Die Demo laeuft in QEMU `raspi4b`
   (`measurement-2026-09-28-port07/method/`); dort fehlte bisher nur die
   Tastatur zum Beenden — der IKBD-Injektor liefert sie.
2. **Den letzten Aufruf feststellen**, mit dem Instrument aus
   [DESK43](DESK43.md) U1: der AES-Tracer sieht seit `c141be2` jeden `svc`,
   auch aus EL1.
3. Erst dann die Ursache suchen — und **sagen, ob sie in der Demo oder bei uns
   liegt**, bevor etwas geaendert wird.

## Abnahme

Die Demo beendet sich, der Desktop kommt vollstaendig neu gezeichnet zurueck,
die Palette ist wiederhergestellt, und `C:\BALL.LOG` endet mit einer
**vollstaendigen** letzten Zeile.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
