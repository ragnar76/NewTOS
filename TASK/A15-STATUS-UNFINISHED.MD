# A15 — Quittung

**Status: UNFINISHED** · 2026-09-10 · wind_get: geschriebene Felder am
Listenende

## Auftrag

Nur welche x/y/w/h-Wörter beim Listenende geändert bzw. erhalten werden.
Nicht pauschal alle vier Wörter nullen; interne und kopierte Ausgabepuffer
unterscheiden. A13, A14 und A16–A24 nicht anfassen.

## Was ich gemacht habe

1. Den Kopfkommentar der eingefrorenen Tabelle `tosdump/cases/aes-r2b-rlist.tsv`
   vollständig gelesen (Zeilen 1–109, einschließlich des `w_owns`-Wortlauts bei
   `:56-64` und der Poison-Begründung `:68-72`) und die Fall-IDs als
   **physische Zeilennummern** bestimmt. A15 sind die drei Fälle, in denen der
   **gemessene** Aufruf am Listenende steht: **127, 133, 204**. Für jeden im
   Prelude-Block nachgeprüft, welche Aufrufe ihm vorausgehen.
2. Beide BIN selbst mit `tosdiff --validate` gegen die Tabelle geprüft und die
   SHA-256 von Tabelle, Pi-Lauf und TT-Referenz nachgerechnet: `a81a7a6c…`,
   `e0280b7d…`, `e7ef0a27…`.
3. Die Rohwörter **nicht** aus `diff/06`/`diff/07` übernommen, sondern mit einem
   eigenen, rein lesenden Recordleser direkt aus beiden BIN gelesen (Layout
   `tosdump/core/tosdump.h:114-125`, Wörter big-endian nach
   `tosdump/core/record.c:43-54`, Poison `0xDEAD` nach `tosdump.h:78`), je Wort
   die Poison-Bilanz bestimmt und meine drei Fälle zusätzlich **byteweise**
   verglichen.
4. Den gepinnten EmuTOS-Stand selbst überprüft: `references/emutos` @
   `1a7484aba1bfba23e2c0899e243b6d8177fe41df`, `git describe` =
   `VERSION_1_4-71-g1a7484ab`.
5. Das Soll aus der Quelle geholt, nicht aus der Spec: `w_owns`
   (`aes/gemwmlib.c:1230-1241`, Listenende `:1240`), der FIRST/NEXT-Arm
   (`:1556-1562`), `get_pwin` (`:1391-1404`), `w_setup` (`:205-228`),
   `wm_create` (`:1369-1385`), `w_getsize`/`WS_TRUE` (`:254-262`,
   `DROP_SHADOW_SIZE` bei `:49`), `rc_intersect` (`util/rectfunc.c:63-78`,
   entscheidend `:72-75`), `newrect`/`brkrct`/`mkpiece`
   (`aes/gemwrect.c:165-205`, `:101-132`, `:64-98`, Reihenfolge `:30-33`),
   und der Dispatcher-Kopierpfad: `xif` mit dem **nicht initialisierten**
   lokalen `WORD int_out[O_SIZE]` (`aes/gemsuper.c:432`), die Rückkopie
   (`:444-445`), `WM_OX = int_out[1]` (`include/aesdefs.h:509-512`),
   `RET_CODE = int_out[0]` (`:140`), der prozesseigene Supervisorstack
   (`gemsuper.c:484`).
6. **Die beiden Listenende-Pfade getrennt** — das ist der Kern des Auftrags:
   P1 (Schleife gar nicht betreten, nur zwei Wörter geschrieben, `g_x`/`g_y`
   = Inhalt des dispatch-lokalen Puffers) und P2 (Schleife betreten, kein
   Schnitt gelingt, alle vier Wörter geschrieben, `g_x`/`g_y` = `tx`/`ty` des
   letzten fehlgeschlagenen `rc_intersect`).
7. Für jeden meiner drei Fälle bestimmt, welcher Pfad greift, und es belegt
   statt es zu vermuten: **P1 in allen dreien**, über (a) die Quelle
   (einelementige Liste, `w_rnext` vom FIRST verbraucht), (b) den Messbeleg
   innerhalb eines Records (in 127 wurde der Restwert an `[3]`/`[4]` von
   296/194 auf 0/0 überschrieben, an `[1]`/`[2]` nicht — zwei verschiedene
   Schicksale in einem Record), (c) die Herkunftsbilanz des Restwerts aus
   Records desselben Laufs, ohne Modell.
8. **Über alle 15 Records geprüft**, nicht an einem: genau vier Records stehen
   am Listenende (127, 133, 144, 204), alle vier sind durch den Restwert des
   unmittelbar vorangehenden AES-Aufrufs widerspruchsfrei erklärt, mit zwei
   **verschiedenen** Wertepaaren (31,63 und 0,19). Fall 144 (nicht meiner, nur
   als Beleg) trägt den Wert über eine **Fallgrenze** samt Reset hinweg, in
   einen Zustand, zu dem er geometrisch nicht passt.
9. Die Gegenhypothese „der Aufrufer behält seinen Wert" **widerlegt**: der
   Runner vergiftet `int_out` unmittelbar vor dem Messaufruf
   (`runner_aes.c:178` → `record.c:224`), der TT meldet dort kein `0xDEAD`.
   Damit ist die Formulierung im Kopfkommentar der eingefrorenen Tabelle
   (`:70-72`) durch die Messung widerlegt, die sie begründet hat — Datei
   unverändert gelassen, Punkt nur festgehalten.
10. Die Umsetzung gelesen: `tos-api/src/aes/window.rs:1999-2010`
    (FIRST/NEXT-Arm), `:1142-1155` (`rlist_next`, `Grect::default()` bei `:1152`),
    `:1129-1139` (`compute_rlist`), `:1110-1124` (`work_rects`), `:600-615`
    (`border_pieces`), `:1188-1199` (`visible_bound`), `:633-646`
    (`walk_rects`); `tos-api/src/aes/mod.rs:263` (genullter lokaler Puffer),
    `:132-140` (Rückkopie).
11. **Grenzfälle gefahren**, über einen temporären Testaufsatz
    (`tos-api/tests/zz_audit_a15_tmp.rs`, danach gelöscht), in **beiden**
    Profilen (640×480/4 Planes und 1920×1080/16 Planes), mit dem
    Poison-/Reset-/Prelude-Protokoll aus `tosdump/core/runner_aes.c:123-191`
    nachgebaut: die drei Tabellenfälle plus die Referenzfälle 122 und 201;
    `OUT_LEN` 0…7 am Listenende **je zweimal**, einmal mit vergiftetem und
    einmal mit `0x5A5A` vorbelegtem Aufruferarray, um *geschrieben* von
    *erhalten* zu trennen; der P2-Aufbau in zwei Varianten; ein vollständiger
    Redraw-Lauf mit vorbelegtem Aufruferarray.
12. Geprüft, ob es schon einen dauerhaften Test gibt — **nein**: weder
    `FIRSTXYWH` noch `NEXTXYWH` kommen in `tos-api/tests/` vor.
13. Neue Vergleichsdatei mit Nummer 13 angelegt, erzeugende Befehle im Kopf.

## Artefakte

- `measurement-2026-09-09/diff/13-a15-wind-get-listend-words.txt` —
  Validierungen, SHA-256, Fallzuordnung, Rohwörter feldweise aus beiden BIN,
  Bytevergleich, die Poison-Bilanz über alle 15 Records, die Herleitung von P1
  und P2 aus der Quelle, die Herkunftsbilanz des Restwerts, die vollständigen
  Hostläufe beider Profile, das Verdikt je Fall und je Wort, der minimale Fix,
  der Regressionstestvorschlag, die Kompensationsprüfung und die Messlücken.
- `TASK/A15.md`, Abschnitt „Auswertung — 2026-09-10". Statuszeile bleibt
  **OFFEN**.
- `TASK/README.md` **unberührt** — die A15-Zeile bleibt OFFEN, Zeile 5 und die
  Einstiegszahlen sind nicht angefasst.

Eingefrorene Artefakte (`cases/*.tsv`, alle `*.BIN`, `*.LOG`, `MESSRUN.TXT`,
`RUNINFO.TXT`) wurden nicht angefasst; die Diffs 00–12 sind unverändert.
Keine `.rs`-Datei im Baum geändert; der temporäre Testaufsatz
`tos-api/tests/zz_audit_a15_tmp.rs` ist **gelöscht** (per `rm`, nicht per
`git checkout`/`clean`/`stash`).

## Ausgeführte Prüfung

    $ ./tosdump/host/build/tosdiff --validate \
          measurement-2026-09-09/run3-pi4-rlist/AES.BIN tosdump/cases/aes-r2b-rlist.tsv
      -> area AES, 15 records of 120 bytes, build 70ca, case CRC B4CC4A5A, accepted
    $ ./tosdump/host/build/tosdiff --validate \
          tosdump/reference/EMUTOS14-AES-r2brlist.BIN tosdump/cases/aes-r2b-rlist.tsv
      -> area AES, 15 records of 120 bytes, build c911, case CRC B4CC4A5A, accepted

    Rohwertlesung aller 15 Records aus beiden BIN (eigener Leser), Poison je
    Wort, Bytevergleich der drei A15-Records mit Feldnamen.

    $ cd references/emutos && git rev-parse HEAD
      -> 1a7484aba1bfba23e2c0899e243b6d8177fe41df  (VERSION_1_4-71-g1a7484ab)

    $ A15_PROFILE=pi cargo test -p tos-api --test zz_audit_a15_tmp -- --nocapture --test-threads=1
    $ A15_PROFILE=tt cargo test -p tos-api --test zz_audit_a15_tmp -- --nocapture --test-threads=1
      -> 1 passed; der vollständige Ausdruck steht in diff/13.

    Der Hostlauf reproduziert alle drei gemessenen Fälle Wort für Wort
    (127, 133, 204 je `1, 0, 0, 0, 0`) sowie die Referenzfälle 122 und 201 —
    der Aufsatz ist damit gegen echte Hardware geeicht.

## Ergebnis

Drei Fälle, je vier Wörter, getrennt ausgewertet.

| Fall | TT `c911` | Pi 4 `70ca` | `g_w`/`g_h` | `g_x`/`g_y` |
|---|---|---|---|---|
| 127 | `1, 31, 63, 0, 0` | `1, 0, 0, 0, 0` | beide **geschrieben**, 0, **gleich** | TT **nicht geschrieben**, Pi geschrieben |
| 133 | `1, 31, 63, 0, 0` | `1, 0, 0, 0, 0` | beide **geschrieben**, 0, **gleich** | TT **nicht geschrieben**, Pi geschrieben |
| 204 | `1, 0, 19, 0, 0` | `1, 0, 0, 0, 0` | beide **geschrieben**, 0, **gleich** | TT **nicht geschrieben**, Pi geschrieben |

Kein Wort innerhalb `OUT_LEN 5` ist auf einer der Maschinen POISON geblieben;
`int_out[5]`/`[6]` sind auf beiden POISON.

**Der greifende Mechanismus ist in allen drei Fällen P1** — die Schleife wird
gar nicht betreten, `w_owns` schreibt nur `g_w`/`g_h` (`gemwmlib.c:1240`), und
was der Aufrufer an `g_x`/`g_y` bekommt, ist der **dispatch-lokale, nicht
initialisierte** Puffer aus `gemsuper.c:432`, unbedingt zurückkopiert durch
`:444-445`. **Nicht** der Rest einer `rc_copy` dieses Aufrufs. Belegt über die
Quelle, über zwei verschiedene Schicksale innerhalb eines Records, über die
Herkunftsbilanz aus Records desselben Laufs und über alle vier
Listenende-Records inklusive des Falls, der die Fallgrenze überschreitet.

**Ein Wert aus einem uninitialisierten Puffer ist kein Soll.** Festgemacht an:
(1) `gemsuper.c:432` deklariert eine `auto`-Variable, die EmuTOS nirgends
initialisiert; (2) der Wert folgt der **Aufrufgeschichte** (31,63 gegen 0,19),
nicht dem Fensterzustand; (3) er überlebt in Fall 144 einen Reset und eine
Fallgrenze in einen Zustand, zu dem er geometrisch nicht passt; (4) über alle
15 Records geprüft, nicht an einem.

**Gemessener Teil: hinnehmbare Abweichung, kein Fix.** Die beiden Wörter mit
Soll stimmen; das dokumentierte Abbruchkriterium ist `w == 0 && h == 0`, und
die einzige Anwendung im Baum, die die Liste läuft, liest `g_x`/`g_y` danach
nie (QED `rc_first`/`rc_next`, `sdk/apps/qed/src/window.c:224-249`).

**Aber ein belegter Unterschied bleibt: P2.** Ist die Liste nicht leer und
schneidet trotzdem kein Stück die Arbeitsfläche, schreibt EmuTOS **alle vier**
Wörter und liefert **deterministische Geometrie** — im gefahrenen Aufbau
`1, 31, 259, 0, 0` gegen unser gemessenes `1, 0, 0, 0, 0`. P2 existiert in
unserer Umsetzung überhaupt nicht, weil `work_rects` (`window.rs:1110-1124`)
nicht schneidende Stücke schon beim Bauen verwirft. Das ist ein Soll, das wir
nicht treffen; es ist auf realer EmuTOS-Hardware **nicht gegengemessen**.

Fix **F-A15-1** und Regressionstest **R-A15-1** sind in `TASK/A15.md` und
`diff/13` benannt und **nicht** umgesetzt; die Entscheidung liegt beim Anwender.

**Kompensationsprüfung** (für A23): Die Größe wird an **zwei** Stellen
erzeugt, und die heben einander auf — ein Fix nur an `window.rs:2004-2005`
ändert beim Aufrufer nichts, weil `mod.rs:263` den lokalen Puffer nullt und
`mod.rs:132-140` ihn unbedingt hinauskopiert; ein Fix nur an `mod.rs:263` wird
von `window.rs:2004-2005` sofort überschrieben. `work_rects` hat einen zweiten
Leser, `visible_bound` (`:1190`) → `broadcast_redraw` (`:1223`), also die
`WM_REDRAW`-Rechtecke — der Fix darf `work_rects` nicht anfassen. Und der
Erwartungswert `31, 259` setzt **beide** A13-Fixes voraus (Schatten am
brechenden Rechteck, Reihenfolge TOP/LEFT/RIGHT/BOTTOM), weil er der Wert des
**letzten** Stücks ist: **F-A15-1 darf nicht vor A13 F2/F3 umgesetzt werden.**

## Offen

- **Ein Fix steht aus** (F-A15-1, für P2), und er ist an A13 F2/F3 gekoppelt.
  Deshalb UNFINISHED, nicht DONE.
- **P2 ist auf realem EmuTOS nicht gemessen.** `1, 31, 259, 0, 0` ist
  quellenhergeleitet und nur auf unserer Seite gefahren. Eine echte
  Gegenmessung braucht eine **neue** Falltabelle mit zwei Fenstern, deren
  oberes die Arbeitsfläche des unteren vollständig deckt. Ich habe keine
  angelegt — das ist ein Messdesign und gehört nicht in diesen Auftrag.
- `OUT_LEN` 0, 2, 4, 6, 7 sind auf dem TT nicht gemessen (nur 1, 3, 5). Unser
  Verhalten dort ist quellenbelegt und hostgefahren, nicht messbelegt.
- Ob der TT-Restwert bei einem tieferen AES-Aufruf dazwischen kippt, ist nicht
  gemessen; die Aussage stützt sich auf `gemsuper.c:432` und die
  Herkunftsbilanz, nicht auf ein Gegenexperiment.
- **Spezifikationslücke, gemeldet, nicht geändert** (Researcher):
  `docs/api/aes/window-graphics.md:341` und `:848-851` verschweigen die
  Zwei-Wort-Regel und den Unterschied P1/P2; dass der Aufrufer innerhalb
  `OUT_LEN` nie seinen eigenen Wert behält, fehlt ebenfalls. Kein Task von mir
  angelegt; A18 deckt den Bereich ab.
- **Ungenauigkeit in einem eingefrorenen Artefakt, nur festgehalten:**
  `tosdump/cases/aes-r2b-rlist.tsv:69-72` behauptet „they keep whatever the
  caller's buffer held" (durch die Poison-Messung widerlegt) und „whatever the
  last rc_copy left" (übersieht das nachfolgende `rc_intersect`,
  `rectfunc.c:72-73`). Datei nicht angefasst.
- **Unabhängiger Nebenbefund, gemeldet, kein Task von mir angelegt:** bei
  `OUT_LEN` 6/7 liefert der genullte dispatch-lokale Puffer (`mod.rs:263`) an
  `int_out[5]`/`[6]` Nullen, wo EmuTOS Stapelinhalt liefert. Gleiche Klasse und
  gleiches Verdikt wie P1, betrifft aber **jeden** AES-Aufruf, dessen Handler
  weniger Wörter schreibt als der Aufrufer deklariert — unter anderem
  `WF_COLOR`/`WF_DCOLOR`, wo die Spec in D-095 (`:312-325`) behauptet,
  `int_out[1]` lese sich „unchanged" zurück.
- **Nicht bewertet, außerhalb des Auftrags:** Fall 144 (A13, hier nur als
  Beleg für den Mechanismus herangezogen), 166/174/183/213 (A14),
  122/159/189/201/208/226/230 und die `global[]`-Abweichung an jedem Record
  (A06–A09).
