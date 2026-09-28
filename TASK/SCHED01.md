# SCHED01 — Ein gestartetes GEM-Programm steht nie in der Laufmenge; `appl_exit` verliert es an das erste Accessory

Status: **OFFEN** · Prioritaet: **P1** · Typ: Belegter Defekt / Haenger — aus [PORT08](PORT08.md), gemessen 2026-09-29

Abhaengigkeiten: — · **Betrifft jede EL0-GEM-Anwendung, sobald mindestens ein Accessory geladen ist**

## Der Befund, direkt gemessen

Letzter Aufruf der Ball-Demo beim Beenden, mit Wire-Argumenten:

```
266 VDI op=101 v_clsvwk  handle=2          -> 0
267 AES op=19  appl_exit ctl=[19,0,1,0,0]  -> KEIN RUECKSPRUNG
```

Die Laufmenge im selben Augenblick:

```
runset cur=2 : s0(run=0 ctx=1 cond=0x200000000000000)
               s1(run=0 ctx=1 cond=0x200000000000001)
               s2(run=0 ctx=0 cond=0x0)
```

**`s2` ist die Anwendung — sie laeuft auf der CPU, steht aber mit `run=0` nicht
in der Laufmenge.** `sched::admit` wird **ausschliesslich** fuer Accessories
gerufen (`tos-api/src/aes/acc.rs:103`); ein gestartetes GEM-Programm wird
nirgends aufgenommen.

**Der Ablauf:** `appl_exit` → `mn_cleanup()` weckt beide Accessories →
`wait_for_accs(AP_ACCLOSE)` → `all_run()` → `sched::dsptch()` gibt die CPU ab
→ die Accessories holen `AC_CLOSE` und blockieren wieder → `pick_next` sucht
`runnable == true` und findet **nichts**: die Anwendung hat `run=0` **und**
`cond=0`, ist also weder waehlbar noch weckbar. Gemessen 103 600 000
Leerlaufdurchgaenge lang unveraendert.

**Warum die Maus dabei lebt:** ein Accessory dreht in `sleep`s Leerlaufausgang,
dessen Rumpf `host().pump_input()` ist. Die Projektregel
[[mouse-moves-but-nothing-responds-means-app-sleeping]] hat hier das richtige
Ziel getroffen, aber **aus dem falschen Grund** — nicht die Anwendung wird
bedient, sondern ein Accessory laeuft leer.

## Gegen die Quelle

EmuTOS' `disp` nimmt den laufenden Prozess vom Kopf von `rlr` und legt ihn ohne
`WAITIN` per `disp_act` → **`insert_process(p, &rlr)` wieder hinein**
(`references/emutos/aes/gemdisp.c:74-79,220-238`). Dort ist der laufende
Prozess **per Konstruktion** auf der Bereitliste.

Unser `sched::dsptch` legt nichts ein, sondern **setzt Mitgliedschaft voraus**
(`tos-api/src/bdos/process/sched.rs:592-605`). Der Kommentar dort — *„will
pick the caller again when it comes round"* — ist fuer ein gestartetes Programm
schlicht falsch. **ACC10s Commit `814de2c` zitiert genau dieses
`insert_process`; der Einfuegeschritt selbst ist nicht mitgekommen.**

## Tragweite — nicht demospezifisch

`sched::dsptch` hat genau einen Aufrufer (`acc::all_run`), erreichbar ueber
`appl_exit`. Betroffen ist damit **jede EL0-GEM-Anwendung, die `appl_exit`
ruft**, sobald mindestens ein Accessory geladen ist. Der EL1-Bootpfad hat
keinen Slot und ist nicht betroffen.

**Nachgezaehlt im Baum:** `Applications/qed` ruft `appl_exit` **null**-mal —
deshalb beendet QED sauber, und das ist **kein Gegenbeweis, sondern eine
Bestaetigung des Mechanismus**. `sdk/apps/teradesk` ruft es **achtmal**.

**Offene Frage an den Anwender:** wurde TeraDesk je selbst beendet? Die
Rueckmeldung vom 2026-09-26 („in TeraDesk QED starten und sauber beenden")
betraf QED.

## Gegenprobe, gefahren

Dieselbe Karte **ohne** die beiden `.NAC`: `appl_exit` kehrt sofort zurueck,
die Demo schreibt ihre Schlusszeilen, gibt ihre Puffer frei, der Desktop meldet
seine Menuezeile wieder an (`AES op=30 menu_bar in=[1,…]`) und landet in
`evnt_multi`. Vollstaendiger Desktop im Screendump.

## Kein eigener Defekt: der Streifen und die fehlende Menuezeile

EmuTOS' `fm_dial(FMD_FINISH)` zeichnet die Menuezeile **ebenfalls nicht** neu
(`aes/gemfmlib.c:375-379` → `w_drawdesk` → `do_walk(DESKWH, …)`,
`gemwmlib.c:340-359`). Der Streifen ist der sichtbare Fingerabdruck davon, dass
der Desktop nie wieder die CPU bekommt.

## Vermutet, nicht gefahren

Vor `814de2c` schlief die Barriere auf `wait_input()`; der Slot haette
`cond != 0` gehabt, und `idle_wake_pass` setzt solche Warter bei jeder
Mausbewegung wieder bereit (`sched.rs:704-707`). Auf der interaktiven Pi waere
der Stillstand vorher also **vermutlich von selbst weitergegangen**; heute ist
`cond=0` und keine Weckklasse greift. **Nicht reproduziert** — das haette einen
Rueckbau von Produktcode verlangt.

## Auftrag

`sched::dsptch` muss tun, was `disp_act` tut: den Aufrufer in die Laufmenge
**einlegen**, statt Mitgliedschaft vorauszusetzen. Zu klaeren ist dabei, ob
stattdessen (oder zusaetzlich) der **Programmstart** den Slot aufnehmen muss —
EmuTOS' Anwendung ist per Konstruktion auf `rlr`, und das ist die strukturell
naehere Nachbildung. **Erst lesen, dann entscheiden.**

## Abnahme

Die Ball-Demo beendet sich **mit** geladenen Accessories; der Desktop kommt
vollstaendig zurueck und meldet seine Menuezeile an. Dazu TeraDesk beenden,
ebenfalls mit Accessories. Regressionsflaeche: ACC02–ACC06 (der
Accessory-Weg selbst) und jeder Programmstart.

## Arbeitsregel

[Gemeinsame Regeln](README.md) gelten.
