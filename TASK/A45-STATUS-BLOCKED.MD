# A45 — Quittung: abgeschlossen als BLOCKED (2026-09-13)

## Was das Ticket verlangte

Regel 2 laesst fuer eine nicht implementierbare Funktion zwei Wege und keinen
dritten: implementieren, oder ehrlich abweisen und die fehlende Abhaengigkeit
in `docs/STATUS.md` unter *Blocked* eintragen. Das Ticket verlangte, dem
Anwender vorher die Frage vorzulegen, ob eine Zielanwendung den Ereignisband
braucht.

## Die Vorlage und ihre Antwort

**Nein.** Ein einziger Treffer im ganzen `sdk/` — die Deklaration in
`sdk/gemlib/overlay/mt_gem.h.in`. Kein Aufruf, in keiner portierten Anwendung.

## Warum es nicht implementierbar ist

Die Funktionen sind nicht der Ort des Problems. EmuTOS zeichnet in `ap_trecd`
gar nicht auf: es setzt `gl_recd`/`gl_rlen`/`gl_rbuf`
(`aes/gemaplib.c:209-213`), und das Mitschreiben passiert in `forker()`
(`aes/gemdisp.c:123-146`), das jedes eingereihte `FPD` in den Aufruferpuffer
kopiert; `ap_trecd` pollt danach `ev_timer(100)`, bis die Queue voll ist oder
das Band stoppt, und wandelt `FPD` erst hinterher nach `EVNTREC` (`:216-230`).

`ap_tplay` speist die Ereignisse ueber `forkq`/`dsptch` zurueck
(`aes/gemaplib.c:126-166`) und tauscht fuer `MCHNG` zusaetzlich die
VDI-Cursor- und Bewegungsvektoren aus (`:139-152`), damit die AES den Zeiger
waehrend der Wiedergabe selbst zeichnet.

**Beides haengt an der Forkqueue, die NewTOS' AES nicht hat** — dieselbe
Luecke, die `tos-api/src/aes/mod.rs` an `chkkbd` fuer den Tastenpfad
dokumentiert (*„NewTOS' AES has no such queue and leaves the key in the IKBD
ring"*). Sie gehoert zur kooperativen Kernelarbeit in Meilenstein 2b.

## Was NICHT getan wurde, und warum

**Kein Fehlercode erfunden.** Die AES kennt fuer diesen Fall keine Konvention,
und ein Wert, den kein TOS je zurueckgibt, waere eine neue Abweichung anstelle
der alten.

Beide heutigen Rueckgabewerte sind uebrigens richtig, einer davon aus dem
falschen Grund:

- `appl_trecord` → `0`. Woertlich wahr: null aufgezeichnete Ereignisse.
- `appl_tplay` → `1`. Richtig, aber **nicht** weil etwas abgespielt wurde:
  EmuTOS' `APPL_TPLAY`-Arm setzt gar kein `ret`, es bleibt beim `ret = TRUE`
  aus `crysbind` (`aes/gemsuper.c:85,109-110`). Unser hartkodiertes `1` trifft
  denselben Wert auf einem anderen Weg. Genau diese Sorte Uebereinstimmung
  maskiert Defekte — deshalb steht sie jetzt im Kommentar.

## Geaendert

- `docs/STATUS.md` — Eintrag unter *Blocked* mit der Forkqueue als benannter
  Abhaengigkeit.
- `tos-api/src/aes/appl.rs` — die Doc-Kommentare von `appl_trecord` und
  `appl_tplay` nennen die Abhaengigkeit mit `Datei:Zeile` statt vage von einem
  „documented follow-up" zu sprechen. Kein Codeverhalten geaendert.

## Wann das Ticket wieder aufgeht

Sobald die Forkqueue steht. Diese beiden Funktionen sind ihre ersten zwei
Abnehmer; der Blocked-Eintrag ist die Stelle, an der sie wieder auftauchen.
