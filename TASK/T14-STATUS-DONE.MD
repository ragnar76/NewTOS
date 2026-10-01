# T14 — STATUS: DONE (2026-10-01) — Ursache: der VDI-Mauszeiger, Testfehler, Riegel gesetzt

**Auftrag:** den geteilten Zustand finden, ueber den die beiden Tests von
`tos-api/tests/aes_icon_unaligned_raster.rs` parallel gelegentlich
ineinanderschlagen, Test- oder Produktfehler entscheiden, beheben
(`TASK/T14.md`).

## Was tatsaechlich wettlaeuft

**Das Bild ist nicht leer.** Die Fehlerausgabe beginnt mit tausenden
`0`-Pixeln (Pen 0 = weiss, der `G_BOX`-Grund), deshalb sah sie leer aus.
Ausgezaehlt (alle sechs gesammelten Fehlschlaege, siehe unten, identisch):

- `left` (der zweite Render) hat **112** Pixel in Pen 1, `right` **58**;
- es unterscheiden sich genau **54** Pixel, alle in `x 64..71`, `y 48..61`;
- 54 ist die Bitzahl der `data`-Ebene der Preset-Form ARROW
  (`vdi/mouse.rs` `PRESET_FORMS[0]`: 1+2+3+4+5+6+7+8+5+4+3+2+2+2), gezeichnet
  in `FG_COL` = 1, die Maske in `BG_COL` = 0 (unsichtbar auf Weiss);
- `ensure_init` (`vdi/mouse.rs:340-352`) parkt den Zeiger bei
  `park_position(128, 96)` = `(63, 47)`, Hotspot `(0, 0)` — die Pfeilspitze
  liegt dort leer, die erste Datenzeile beginnt bei `(64, 48)`.

Der Fremdkoerper im Bild ist also **der Mauszeiger**. Und er scheitert
**nicht nur** in `:357`: unter Last scheiterte viermal der zweite Test,
`a_g_image_with_an_odd_bi_pdata_draws_like_an_even_one` (`:401` der alten
Datei), mit derselben Signatur.

Der Weg dorthin:

- Jeder Objektbaum-Zeichenaufruf wird von der Zeiger-Klammer umschlossen:
  `render_tree_t` (`aes/object.rs:1331`/`:1333`) →
  `hide_cursor_for_draw`/`show_cursor_after_draw` (`:1372`/`:1381`) →
  `crate::vdi::cursor_hide`/`cursor_show` = `vdi/mouse.rs` `hide` (`:852`)
  und `show` (`:861`). Beide Tests der Datei laufen durch
  `render_tree_lp64` → `render_tree_t`.
- Der Zeiger ist **ein** prozessweiter Satz Statics: Versteck-Tiefe
  `HIDE_COUNT` (`vdi/mouse.rs:33`, startet bei 1 = versteckt), Position
  `GCURX`/`GCURY` (`:21-22`), Save-under `BACK` (`:112`), Form `MASK`/`DATA`
  (`:41-42`). Kommentar dort (`:110`): "touched only from the single-CPU,
  IRQ-off VDI trap path".
- `hide` erhoeht atomar (`fetch_add`, `:856`), aber `dis_cur`
  (`:798-805`), der Kern von `show`, ist ein **getrenntes** `load` →
  `store`. Laeuft ein `hide` eines zweiten Threads zwischen diese beiden,
  ueberschreibt der `store` die Erhoehung.
- Ablauf des Fehlschlags: Thread A hat versteckt (1→2) und liest in
  `dis_cur` `prev = 2`; Thread B versteckt (2→3) und zeichnet; A speichert
  seinen veralteten Wert 1; B's `dis_cur` liest 1, speichert 0 — der
  Uebergang 1→0 ruft `draw` (`:712`) **in B's Backend**: Save-under aus
  B's Bild, dann der Pfeil hinein. B's Bild enthaelt den Zeiger, der
  Vergleich mit dem anderen Render faellt.

### Deterministische Reproduktion (nur Scratch)

In einer **Kopie** von `tos-api`/`tos-types` im Scratch-Verzeichnis
(eigenes Workspace, eigenes `target/`; der Baum unter `tos-api/src` blieb
unberuehrt) bekam `dis_cur` zwischen `load` und `store` ein Tor
(`T14_PAUSE_NEXT`/`T14_PAUSED`/`T14_RELEASE`), dazu ein Scratch-Test mit
einem Backend, dessen `blit_mono` auf ein zweites Tor wartet. Erzwungen
genau die obige Reihenfolge:

```
HIDE_COUNT at rest: 1
A parked in dis_cur, loaded prev, HIDE_COUNT now 2
B inside its draw (after its hide), HIDE_COUNT 3
A stored its stale value, HIDE_COUNT 1
B done, HIDE_COUNT 0
diff pixels: 54
x 64..71  y 48..61
pen-1 in got 112, in reference 58
```

Bit fuer bit die Signatur der echten Fehlschlaege, und zwar bei **jedem**
Lauf, nicht bei einem von tausend.

## Test- oder Produktfehler

**Testfehler.** Der Zeigerzustand ist kein Zustand, der "zwischen
Zeichenaufrufen nicht ueberleben duerfte" — er **muss** ueberleben: es gibt
einen Bildschirm und einen Zeiger, und `HIDE_COUNT` ist der zaehlende
Semaphor, den der Doc-Kommentar zu `hide_cursor_for_draw`
(`aes/object.rs:1340-1371`) als Gegenstueck zu EmuTOS' `gl_moff` beschreibt
(Klammern duerfen sich verschachteln; nur 0→1 loescht, nur 1→0 zeichnet).
Nach jedem ausgeglichenen Zeichenaufruf steht er wieder auf 1; seriell
bleibt der Zeiger deshalb versteckt und kein Test sieht ihn. Ungesperrt ist
er absichtlich (Einkern-Trap-Pfad, `vdi/mouse.rs:110`) — derselbe Fall wie
`PROCESS_TABLE` in T12. Wettlaufen kann er nur, weil `cargo test` die Tests
einer Datei in Threads nebeneinander laufen laesst. `tos-api/src` ist
**unveraendert**.

## Der Riegel

Dieselbe Form wie `aes_rsrc_per_process.rs:69-75` und die drei
`sched_*`-Dateien aus T12 — ein lokaler `LOCK` je Binary, kein neuer
Mechanismus:

```rust
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
```

und `let _g = lock();` als erste Zeile beider `#[test]`. Der Kommentar
ueber dem `LOCK` nennt den Grund (die Zeiger-Klammer und `vdi/mouse.rs`).

Geaendert, nur eine Testdatei:

- `tos-api/tests/aes_icon_unaligned_raster.rs` — `MutexGuard`-Import,
  `LOCK`/`lock()`, 2× `let _g = lock();` (+16/−1). Die Asserts wandern
  dadurch von `:357`/`:401` auf `:371`/`:415`.

## Belege — Wiederholungslaeufe

Die Binaries **vor** und **nach** der Aenderung wurden als Kopie
festgehalten und direkt aufgerufen (Standardrunner = parallele Threads,
bzw. `--test-threads=1`). "Last" = Dauerschleife
`rm -rf <tgt>; CARGO_TARGET_DIR=<tgt> cargo test -p tos-api -p tos-types --no-run`
mit privatem Zielordner im Scratch, parallel zu den Laeufen (waehrend der
Laeufe 2–10 `rustc`-Prozesse gezaehlt).

| Modus | vorher | nachher |
|---|---|---|
| parallel, ohne Last | 1000/1000, dann **999/1000** | 1000/1000 |
| `--test-threads=1`, ohne Last | 300/300 | 300/300 |
| parallel, unter Last | 1000/1000, **2999/3000**, **4997/5000** | 3000/3000, 5000/5000 |
| `--test-threads=1`, unter Last | 300/300 | 300/300 |

Summe parallel: vorher **5 Fehlschlaege in 11 000** Laeufen, nachher
**0 in 9 000**. Alle fuenf mit der Zeigersignatur (54 Pixel, `x 64..71`,
`y 48..61`, Pen 1: 112 gegen 58); einer in `:357`, vier in `:401`.

Die Fehlerrate war heute deutlich niedriger als in T12 (6/700) und beim
Lead (3/300) — sie haengt an der Thread-Verteilung der Maschine. Deshalb
traegt die Abnahme vor allem die deterministische Reproduktion: mit dem
Riegel koennen sich die beiden Zeichenaufrufe dieser Datei nicht mehr
ueberlappen, die erzwungene Reihenfolge ist ausgeschlossen.

Ein erster Versuch, den Wettlauf mit einer blossen Warteschleife in
`dis_cur` (Scratch) zu verstaerken, schlug **nicht** an (100/100) — die
Schleife verlangsamte beide Threads gleich. Erst die zwei Tore (oben)
erzwangen die Reihenfolge.

## Basislinien

| | gemessen | Basis |
|---|---|---|
| `cargo test -p tos-api -p tos-types -p xtask --no-fail-fast` (100 Binaries summiert) | **964 / 0 / 1** | 964/0/1 |
| `cargo clippy -p tos-api --test aes_icon_unaligned_raster` | 0 Treffer in der Testdatei | — |

Nur eine Testdatei geaendert: Release-Bauten beider Boards, Kernel-clippy
und `tools/desk-drift-probe.sh` wurden **nicht** neu gefahren (beruehren
Testdateien nicht).

## Offen

- **Dieselbe Klammer, ohne Riegel, in weiteren Dateien:** `aes_cicon_draw.rs`
  (5 Tests), `aes_object_font.rs` (6), `aes_object_nicelines.rs` (4)
  zeichnen ueber `render_tree*` ohne jeden Riegel; sie teilen also denselben
  Zeigerzustand. In T12 waren sie je 300/300 — ob ihre Asserts den Pfeil bei
  `(63,47)` ueberhaupt sehen wuerden, ist nicht geprueft. Nicht angefasst
  (ausserhalb dieses Tickets); Vorschlag fuer ein Folgeticket.
- Die uebrigen acht Testdateien mit mehreren Tests und Zeichenaufrufen
  (`aes_objc_change_toggle`, `aes_object_selected_xor`,
  `aes_object_cursor_bracket`, `aes_desk_dialog_chain`,
  `aes_form_alert_cursor_bracket`, `aes_newdesk_root_word`,
  `aes_object_3d_box_border`, `aes_object_3d_text_button`) serialisieren
  bereits ueber einen lokalen `SERIAL`-Mutex — ein dritter Name fuer
  denselben Mechanismus neben `LOCK` und `harness::AES_LOCK`.
