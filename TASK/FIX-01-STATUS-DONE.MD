# FIX-01 — Handleprüfung im FIRST/NEXT-Arm von wind_get

Status: **DONE** (2026-09-10) · Stufe 1 der Fix-Reihenfolge aus
[A23](A23.md), Abschnitt 8.2 · deckt **D-A13-1** und **D-A14-2** ab
(laut A23 Abschnitt 2 dieselbe fehlende Prüfung, nicht zwei Reparaturen)

Kein Commit. Die Änderung liegt uncommittet im gemeinsamen Arbeitsbaum.

## Auftrag

`wind_get` verarbeitete `WF_FIRSTXYWH` (11) und `WF_NEXTXYWH` (12), **bevor**
das Fensterhandle geprüft wurde. Beide lieferten auf einem ungültigen Handle
`ret = 1` samt Geometrie — und, schwerwiegender, sie fassten dabei die globale
Rechteckliste an: FIRST baute sie neu auf und leerte damit die Liste eines
**anderen** Fensters, NEXT verbrauchte ein fremdes Stück.

Ausdrücklich **nicht** Gegenstand: Zerlegungsreihenfolge (Stufe 2), Cursorbesitz
(Stufe 3), Schatten (Stufe 4), P1/P2-Wörter am Listenende (Stufe 5). Nichts
davon wurde angefasst.

## Was geändert wurde

### 1. `tos-api/src/aes/window.rs`, `wind_get` — +14 Zeilen, nur Einfügung

**Vorher** (`:1998-2010`): der FIRST/NEXT-Arm lief ohne jede Handleprüfung; die
einzige Prüfung (`win`, Definition `:1236-1241`) saß erst weiter unten
(vormals `:2056`) im Zweig für die Slot-Felder.

```rust
    // Rectangle-list iteration is computed separately from the slot fields.
    if field == WF_FIRSTXYWH || field == WF_NEXTXYWH {
        if field == WF_FIRSTXYWH {
            compute_rlist(handle);
        }
        let r = rlist_next();
        ...
        pb.set_ret(1);
        return;
    }
```

**Nachher** (`:1999-2024`), eingefügt sind `:2000-2013`:

```rust
    if field == WF_FIRSTXYWH || field == WF_NEXTXYWH {
        // (Kommentar mit dem EmuTOS-Beleg, :2000-2009)
        if win(handle).is_none() {
            pb.set_ret(0);
            return;
        }
        if field == WF_FIRSTXYWH {
            compute_rlist(handle);
        }
        ...
    }
```

Die Prüfung sitzt **innerhalb** des FIRST/NEXT-Arms, wie in A23 Abschnitt 2
verlangt — keine pauschale Vorprüfung vor den drei Ausnahmen. `WF_TOP` (`:2026`),
`WF_SCREEN` und `WF_DCOLOR` liegen unverändert dahinter und beantworten weiter
jedes Handle. `win(handle).is_none()` ist dasselbe Idiom, das `wind_set` bereits
benutzt (`:2176`).

### 2. `tos-api/tests/aes_desk_background.rs` — +107 Zeilen, nur Anhang

Neuer Abschnitt „6. the rectangle-list iterator and the window handle" mit
genau einem Test: `an_invalid_handle_does_not_disturb_the_rectangle_list`,
dazu die Feldnummern 11/12, die beiden ungültigen Handles und der lokale
Helfer `wind_get_ret_rect` (liefert `ret` **und** Rechteck).

Unterbringung in dieser Datei, weil sie den benötigten Aufbau bereits fertig
hat — `SERIAL`-Lock, `rig()`/`fresh()`, zwei überlappende Fenster RA/RB — und
weil ein eigenes Testbinary rund 450 Zeilen Harness dupliziert hätte. Die drei
Wächtertests W6 in derselben Datei wurden nicht berührt.

## EmuTOS-Beleg

Gepinnter Stand `references/emutos` @ `1a7484aba1bfba23e2c0899e243b6d8177fe41df`
(selbst geprüft, `git rev-parse HEAD`).

- `aes/gemwmlib.c:1513-1524` — `wm_get` holt das Fenster mit `get_pwin` und
  gibt für **jedes** Feld außer `WF_TOP`, `WF_SCREEN`, `WF_DCOLOR` FALSE zurück,
  bevor das `switch` überhaupt läuft.
- `aes/gemwmlib.c:1391-1404` — `get_pwin`: Handle < 0, Handle >= `NUM_WIN`
  oder Slot ohne `VF_INUSE` → `NULL`. Handle 0 ist gültig (Desktopfenster,
  `:1290` mit `w_setup:205-228`); ein geschlossenes, aber noch benutztes
  Handle ebenfalls.
- `aes/gemwmlib.c:1556-1561` und `w_owns:1230-1240` — erst hinter der Prüfung
  liest EmuTOS `w_rlist` und schreibt `w_rnext`. Genau das entspricht unserem
  `compute_rlist` / `rlist_next` (`window.rs:1129-1155`).
- `win` (`window.rs:1236-1241`) bildet `get_pwin` bereits vollständig ab —
  Bereich plus `in_use`. Der Fix benutzt sie, er erfindet keine zweite Regel.

## Der neue Test und was er prüft

`tos-api/tests/aes_desk_background.rs`,
`an_invalid_handle_does_not_disturb_the_rectangle_list`.

Aufbau: `fresh()`, dann Fenster A auf `RA = (40,60,400,300)` und darüber
Fenster B auf `RB = (240,180,340,250)`; B bricht A, A besitzt zwei sichtbare
Stücke. Zwei ungültige Handles, die beide Hälften von `get_pwin` treffen:
`-1` (außerhalb des Bereichs) und `15` (im Bereich, aber kein Fenster) — dass
`15` frei ist, prüft der Test vorab über `wind_get(15, WF_WXYWH) == FALSE`,
dieselbe Regel, die die gemessenen TT-Fälle 175–178 belegen.

Geprüft werden vier Dinge, davon sind die beiden Seiteneffekte der eigentliche
Wert:

1. `wind_get(-1, WF_FIRSTXYWH)` liefert `ret = FALSE`.
2. Danach liefert `wind_get(A, WF_NEXTXYWH)` **weiterhin** As zweites Stück —
   die laufende Liste wurde nicht geleert.
3. `wind_get(15, WF_NEXTXYWH)` liefert `ret = FALSE`.
4. Danach liefert `wind_get(A, WF_NEXTXYWH)` **weiterhin** As zweites Stück —
   der Cursor wurde nicht weitergeschaltet.

Der Test friert keine Zahl ein: er vergleicht die gestörte Folge gegen die
ungestörte Folge desselben Laufs (`p0`, `p1`). Damit bleibt er auch nach den
Stufen 2 bis 5 (Reihenfolge, Cursorbesitz, Schatten, P2-x/y) gültig, obwohl
sich die konkreten Rechtecke dort ändern werden.

**Zwei Ebenen, bewusst getrennt** (A23 Abschnitt 2, Vorgabe des Auftrags):
`ret = FALSE` ist die Antwort des *Handlers*. Der Test behauptet **nicht**,
dass der Aufrufer seine vier alten Wörter behält — die Rückkopie überträgt
weiterhin `min(OUT_LEN, O_SIZE)` Wörter aus dem sicher genullten lokalen Puffer
(`aes/gemsuper.c:432-445`; `tos-api/src/aes/mod.rs:132-140,263`). Über
`int_out[1..4]` einer abgelehnten Anfrage sagt der Test deshalb nichts; das
steht als Kommentar im Testkopf.

### Beleg, dass der Test den Defekt wirklich fängt

Der Test wurde **vor** dem Fix geschrieben und gefahren. Alle vier Punkte waren
rot. Damit die Seiteneffekt-Hälften sichtbar wurden, liefen zwei Proben, in
denen die `ret`-Assertionen vorübergehend durch `eprintln!` ersetzt waren
(danach aus einer beiseitegelegten Kopie zurückgeholt, kein `git`-Eingriff):

    PROBE ret of FIRST on invalid handle = 1
    PROBE piece after FIRST on invalid handle = (0, 0, 0, 0) (want (41, 180, 199, 179)) ret=1
    PROBE ret of NEXT on invalid handle = 1
    NEXT on an invalid handle consumed one of A's pieces: A's next piece is
    (0, 0, 0, 0) instead of (41, 180, 199, 179).

Also genau der in A23 Abschnitt 2 beschriebene Schaden: ein FIRST auf einem
ungültigen Handle löscht die laufende Liste eines gültigen Fensters, ein NEXT
verbraucht eines seiner Stücke.

## Tatsächlich gesehene Zahlen

Alle Läufe am 2026-09-10 im gemeinsamen Arbeitsbaum (`70caee2` plus fremde
uncommittete Änderungen), Host macOS/arm64.

Ausgangslage **vor** der Änderung, `cargo test -p tos-api --tests`:
**13 Binaries, 429 passed, 0 failed, 0 ignored.**

Nach der Änderung, derselbe Befehl:
**13 Binaries, 430 passed, 0 failed, 0 ignored** — die eine zusätzliche
Bestehende ist der neue Test. Kein vorhandener Test ist rot geworden, wie A23
Abschnitt 7.1 es vorhergesagt hatte.

Die sieben Wächter aus A23 Abschnitt 7.2, einzeln nachgefahren, alle grün:

| Wächter | Nachgefahren | Ergebnis |
|---|---|---|
| W1 | `case_0175…0178` (`wind_get` Handle 1 / −1 / 99 / 32767 auf `WF_WXYWH`) | 4× ok |
| W2 | alle Fälle mit `wf_wxywh`/`wf_cxywh`/`wf_pxywh`/`wf_fxywh` im Referenzbinary | 22 Fälle, 0 nicht-ok |
| W3 | `window_rectangles_after_create_and_open_match_the_tt` | ok |
| W4 | `aes_object_font` (6) + `aes_dialog_undraw` (2) | 6 ok + 2 ok |
| W5 | `aes_window_drag`: `sizer_drag_…`, `mover_drag_…`, `mover_drag_across_another_window_…` | ok |
| W6 | `aes_desk_background`: `closing_…`, `form_dial_finish_…`, `the_mover_drag_repaints_only_desktop_it_uncovered` | ok |
| W7 | `moving_a_window_off_another_redraws_the_one_underneath` | ok |

Build, beide Boards, mit erzwungener Neuübersetzung (`touch` auf die geänderte
Quelle je Board, weil ein zweiter Board-Build sonst fälschlich „up to date"
melden kann):

    cargo xtask build --board qemu-virt
      -> Compiling tos-api, Compiling kernel, Finished dev profile, 0 errors
    cargo xtask build --board rpi4
      -> Compiling tos-api, Compiling kernel, Finished dev profile, 0 errors
      -> objcopy -> kernel8.img (gitignoriertes Bauartefakt, keine Hardwareaktion)

Warnungen unverändert: `tos-api` (lib) 64, `kernel` (lib) 94 — vor wie nach der
Änderung, auf beiden Boards.

Clippy, `cargo clippy -p tos-api --all-targets`, einmal mit und einmal ohne die
14 eingefügten Zeilen gemessen (Fix vorübergehend entfernt, danach aus der
Kopie zurückgeholt):

| Ziel | ohne Fix | mit Fix |
|---|---|---|
| `tos-api` (lib) | 81 | 81 |
| `tos-api` (lib test) | 82 | 82 |
| `tos-api` (test `vdi_raster_reference`) | 2 | 2 |
| `tos-api` (test `vdi_reference`) | 3 | 3 |

**Delta: 0.** Das neue Testziel `aes_desk_background` erzeugt keine Warnung
(kein Summenzeile-Eintrag). Die einzige Clippy-Meldung, die überhaupt auf
`wind_get` zeigt, ist die vorbestehende `private_interfaces`-Warnung an der
Signatur (`aes::Aespb` ist `pub(crate)`) — sie steht so an jeder `pub fn` mit
`&Aespb` und wurde nicht angefasst.

## Textvorschlag für docs/STATUS.md

Die Datei ist fremd modifiziert und wurde **nicht** angefasst. Vorschlag zum
Eintragen beim Commit, unter *Recent Decisions*:

> **2026-09-10 — `wind_get(WF_FIRSTXYWH/WF_NEXTXYWH)` prüft das Handle**
> (Stufe 1 der Fix-Reihenfolge aus `TASK/A23.md` 8.2; schließt D-A13-1 und
> D-A14-2, laut A23 Abschnitt 2 dieselbe fehlende Prüfung). Der FIRST/NEXT-Arm
> lief bisher vor der Handleprüfung: er antwortete auf ungültigen Handles mit
> TRUE **und** fasste dabei die globale Rechteckliste an — FIRST leerte die
> laufende Liste eines anderen Fensters, NEXT verbrauchte eines seiner Stücke.
> `wind_get` weist beide Felder jetzt vor `compute_rlist`/`rlist_next` mit
> FALSE ab, so wie `wm_get` sein `get_pwin`-Ergebnis vor dem `switch` prüft
> (`aes/gemwmlib.c:1513-1524`, `:1391-1404`) und `w_owns` erst dahinter
> `w_rlist`/`w_rnext` anfasst (`:1230-1240`). `WF_TOP`, `WF_SCREEN` und
> `WF_DCOLOR` bleiben ausgenommen; Handle 0 und ein geschlossenes Handle
> bleiben gültig. Neuer Regressionstest
> `aes_desk_background::an_invalid_handle_does_not_disturb_the_rectangle_list`.
> Suite `cargo test -p tos-api --tests`: 430 passed, 0 failed (vorher 429).

## Was offen bleibt

- **Hardwareabnahme ist für diesen Schritt nicht nötig** — A23 Abschnitt 8.3,
  Zeile „Stufe 1": das Soll stammt vollständig aus der Quelle (`get_pwin`
  `gemwmlib.c:1391-1404`, `wm_get:1513-1524`) und betrifft nur den
  Rückgabewert und den Iteratorzustand, keine Geometrie, keine Auflösung, kein
  Pixel. Es gibt zudem **keinen** TT-Record für FIRST/NEXT mit ungültigem
  Handle (A14, Messlücke), also auch kein Messorakel, das ein Pi-Lauf befragen
  könnte. Die gemessenen TT-Fälle 175–178 belegen die Regel mittelbar für ein
  Rechteckfeld und sind grün geblieben. Ein Hostlauf genügt deshalb hier — und
  nur hier; für die Stufen 2, 4 und 5 gilt das ausdrücklich nicht.
- Die übrigen Defekte aus A13 und A14 bleiben offen: D-A13-2
  (Zerlegungsreihenfolge), D-A14-1 (Cursorbesitz pro Fenster), D-A13-3
  (Schatten). Deshalb bleiben beide Tasks OFFEN.
- Deckungslücke A23 7.5/1 ist nur teilweise geschlossen: FIRST/NEXT haben jetzt
  überhaupt einen dauerhaften Test, aber nur für die Handleprüfung. Reihenfolge,
  verschachtelte Iteration und P1/P2-Wörter bleiben ungetestet.
- Kein Commit gesetzt; `docs/STATUS.md` bewusst nicht angefasst.

## Zusätzlicher Befund — nur gemeldet, nicht angefasst, kein Task angelegt

`compute_rlist` (`window.rs:1129-1139`) und `rlist_next` (`:1142-1155`) arbeiten
weiterhin auf dem globalen `RLIST` (`:1054-1064`). Der jetzige Fix verhindert
die Störung durch *ungültige* Handles; die Störung durch ein zweites *gültiges*
Fenster (`FIRST(1)`, `FIRST(2)`, `NEXT(1)`) besteht unverändert. Das ist
D-A14-1 und Stufe 3 der A23-Reihenfolge, kein neuer Befund.
