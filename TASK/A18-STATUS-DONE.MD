# A18 — Quittung

**Status: DONE** · 2026-09-19 · Rechteckliste: Soll in der Spec nachtragen
statt lizenzieren

## Auftrag

Drei Stellen in `docs/api/aes/window-graphics.md` gegen die Quelle
nachziehen, keine Codeänderung. Quelle: `references/emutos` gepinnt auf
`1a7484aba1bfba23e2c0899e243b6d8177fe41df` (selbst geprüft: `git rev-parse
HEAD` == `git describe --tags` == `VERSION_1_4-71-g1a7484ab`, unverändert).

## Vorbefund — die alten Zeilennummern sind gewandert

Die Zeilen `:354-358`/`:807-811`/`:848-851` aus dem Ticket (Stand
2026-09-10) existieren in dieser Form nicht mehr. Über `git show
<rev>:docs/api/aes/window-graphics.md` an dem Commit, den A13/A18 tatsächlich
meinten (`1683e78`, 2026-07-21, letzter Stand vor 2026-09-10), verifiziert:
`:341` = die `w_owns`-Zeile "When the list is exhausted it returns a rect
with `w=h=0`" (heute Zeile ~355, vor dem Fix), `:354-358` = die
Freistellungsklausel, `:807-811` = der `wind_get(WF_FIRSTXYWH/NEXTXYWH)`
rectangle-list-Absatz unter „Verification criteria" (heute ~1030-1034, vor
dem Fix), `:848-851` = „Edge cases discovered in source" Punkt 2 (heute
~1082-1084, vor dem Fix). Ein dazwischenliegender, unabhängiger Commit
(`9896451`, 2026-09-13, Konsolidierung „docs: the tickets, the audits and
the measurements behind the fixes") hatte die Freistellungsklausel bereits
gestrichen und durch eine tiefere Herleitung zur Subtraktionsreihenfolge bei
**mehreren** verdeckenden Fenstern ersetzt (Befund C,
`docs/analysis/aes-r3-wind-state-einordnung-2026-09-12.md`) — das deckt
Stelle 1 teilweise ab, zitiert aber nirgends `gemwrect.c:30-33`/`:118-122`
und benennt nicht explizit, dass ein fehlendes TOP-Stück ein anderes
Rechteck zum `WF_FIRSTXYWH`-Ergebnis macht. Stellen 2 und 3 waren von diesem
Commit unberührt. Alle drei Stellen sind hier eigenständig nachgezogen.

## Die drei Stellen — vorher/nachher

1. **Zerlegungsreihenfolge** (Absatz „The owner rectangle list itself is
   built by …", jetzt Zeile ~368-390). Vorher: nannte TOP/LEFT/RIGHT/BOTTOM
   nur als Fakt ohne Quellenzeilen für die feste Reihenfolge und ohne die
   Konsequenz für `WF_FIRSTXYWH`. Nachher: zitiert `gemwrect.c:30-33`
   (`#define TOP 0`/`LEFT 1`/`RIGHT 2`/`BOTTOM 3`), die `brkrct`-Schleife
   `gemwrect.c:118-122` (append nur bei `have_piece[i]`, exakt in dieser
   Reihenfolge), die `have_piece[]`-Berechnung `gemwrect.c:113-116`, und
   benennt explizit: fehlt `have_piece[TOP]` (`gemwrect.c:113`), weil das
   verdeckende Fenster bis an die Oberkante reicht, entfällt das TOP-Stück
   und die Liste beginnt mit LEFT/RIGHT/BOTTOM — `WF_FIRSTXYWH` liefert dann
   ein anderes Rechteck als das TOP-Stück.
2. **Zwei-Wort-Regel am Listenende**, an beiden vom Ticket genannten
   Stellen. (a) Die `w_owns`-Zeile in „The rectangle-list iteration"
   (Zeile ~352-362): vorher „returns a rect with `w=h=0`" ohne Hinweis, dass
   `g_x`/`g_y` unangetastet bleiben; nachher zitiert `gemwmlib.c:1240`
   (die einzige Anweisung nach der Schleife: `poutwds->g_w = poutwds->g_h =
   0;`) und erklärt, was in `g_x`/`g_y` tatsächlich steht (letzter
   `rc_intersect`-Schreibversuch, `util/rectfunc.c:72-75`, der alle vier
   Worte auch bei `FALSE` schreibt — oder der unveränderte Aufruferpuffer,
   wenn die Liste von Anfang an leer war). (b) „Edge cases discovered in
   source" Punkt 2 (Zeile ~1085-1090): vorher „terminates with a `w=h=0`
   rect" ohne Nuance; nachher ergänzt denselben `gemwmlib.c:1240`-Beleg und
   das Verbot, `g_x`/`g_y` als Teil des Sentinels zu lesen.
3. **Verdeckendes Rechteck ist `WS_TRUE`**, im `wind_get(WF_FIRSTXYWH/
   NEXTXYWH)`-Absatz unter „Verification criteria" (Zeile ~1030-1052, vormals
   `:807-811`). Vorher: „up-to-four non-overlapping pieces" ohne Reihenfolge
   und ohne `WS_TRUE`/Schatten. Nachher: nennt dieselbe Reihenfolge wie
   Stelle 1 (mit denselben Zeilenbelegen) **und** dass das Rechteck, gegen
   das ein verdeckendes Fenster geschnitten wird, dessen `WS_TRUE`-Rechteck
   ist — Rand plus `DROP_SHADOW_SIZE` auf Breite/Höhe, belegt mit
   `w_getsize(WS_TRUE, wh, &gl_mkrect.o_gr)` an `gemwrect.c:190` (das globale
   Schnittrechteck) und derselben Fetch-Art an `gemwrect.c:203` (das
   Listenkopf-Rechteck des eigenen Fensters), der Addition in `w_getsize`s
   `WS_TRUE`-Zweig `gemwmlib.c:254-262`, und der Konstante
   `DROP_SHADOW_SIZE = 2` in `gemwmlib.c:49`.

## Nebenkorrektur

Beim Formulieren von Stelle 3 zunächst versehentlich `gl_mkrect.o_gr =
w_getsize(WS_TRUE, wh)` geschrieben (Ausdrucksform aus `TASK/A13.md`
übernommen) — `w_getsize` ist aber `void` mit Ausgabeparameter
(`references/emutos/aes/gemwrect.c:190`: `w_getsize(WS_TRUE, wh,
&gl_mkrect.o_gr);`). Vor dem Schreiben selbst gegen die Quelle geprüft und
auf die tatsächliche Aufrufsyntax korrigiert.

## Selbst nachgelesene Quellzeilen (gepinnter Stand)

`aes/gemwrect.c:30-33` (Konstanten), `:64-98` (`mkpiece`), `:101-132`
(`brkrct`), `:113-116` (`have_piece[]`), `:118-122` (Einfüge-Schleife),
`:136-162` (`mkrect`), `:165-205` (`newrect`), `:190`, `:203`
(`w_getsize(WS_TRUE, …)`-Aufrufe); `aes/gemwmlib.c:49`
(`DROP_SHADOW_SIZE`), `:254-262` (`w_getsize`), `:1230-1241` (`w_owns`),
`:1240` (die Zwei-Wort-Zeile); `util/rectfunc.c:63-75` (`rc_intersect`).
Alle Zeilennummern stimmen mit dem gepinnten Checkout überein (per `sed -n`/
`awk` gegen `references/emutos` gegengelesen, nicht aus `TASK/A13.md`
übernommen).

## Diff-Umfang

Nur `docs/api/aes/window-graphics.md`, drei zusammenhängende Hunks
(43 Zeilen eingefügt, 9 gelöscht) an genau den drei benannten Stellen. Keine
weitere Datei angefasst, kein Commit erstellt, `TASK/A18.md`-Kopfzeile
unverändert. Bestehende Tests (`tos-api/tests/aes_reference/
cases_r2b_rlist.rs` u. ä.) nicht angefasst.

## Restbefund — außerhalb dieses Auftrags, nicht bearbeitet

Der „Correction, 2026-09-12"-Block (Zeile ~394 ff.) und die dortige
Belegkette bleiben unverändert; sie behandeln die Subtraktionsreihenfolge
bei **mehreren** verdeckenden Fenstern (aufsteigende z-Ordnung), ein
verwandtes, aber eigenständiges Thema (siehe `docs/analysis/
aes-r3-wind-state-einordnung-2026-09-12.md`, Befund C) — nicht Teil dieses
Tickets und nicht angetastet.
