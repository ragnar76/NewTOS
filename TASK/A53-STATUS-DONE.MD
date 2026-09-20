# A53 — Quittung

**Status: DONE** · 2026-09-20 · Branch `desk-rebuild`, nicht committet.

Jede Zeile aus `TASK/A53.md` einzeln gegen den heutigen Baum geprüft (nicht
blind aus dem Ticket übernommen — mehrere Punkte waren zwischen 2026-09-12 und
heute bereits durch andere Tickets erledigt worden). Reine Doku; Code nur an
den zwei erlaubten Ausnahmen angefasst (Kommentartext).

## Tabelle

| Punkt | Ergebnis | Beleg |
|---|---|---|
| `docs/STATUS.md:244` — `rsrc_gaddr` deckt 6 Typen, `rsrc_saddr` No-op-Stub | **geändert** — `docs/STATUS.md:244` | Heute 17 Typen (`references/emutos/include/aesdefs.h:578-594`), `get_addr` `tos-api/src/aes/resource.rs:2589-2650`, `rsrc_gaddr` `:2668-2680`, `rsrc_saddr` `:2693-2719` (kein Stub). **Dieselbe Behauptung stand noch zweimal in `docs/api/aes/_full-function-inventory.md`** (§2.11 `:266-271`, Tabellenwert `:260`) — trotz einer eigenen, bereits richtigen Korrektur weiter unten im selben Dokument (`:682-688`, „STATUS.md gehört korrigiert"). Beide mitkorrigiert, plus die veralteten `resource.rs`-Zeilenangaben dort (`:1345-1420`/`:1451-1475` → `:2589-2650`/`:2693-2719`) und in der A-4-Zeile (`:891`) |
| `tos-api/tests/aes_reference/harness.rs:948-949` — 1.4 `aesdefs.h` endet bei `WF_COLOR` 18 | **schon erledigt** — Commit `7181835` (A05) | Datei sagt heute (bei `:1262-1334`, Zeile verschoben) korrekt „ends at `WF_DCOLOR` 19"; geprüft, keine weitere Änderung nötig |
| `docs/analysis/full-regression-2026-09-12.md:33-35` — „uncommittete Änderung" | **geändert** — Erratum-Absatz nach `:35` eingefügt | Ursache sind drei committete Fixes: `675e311`, `f76a666`, `1eb4e68` (`git log`, Commit-Betreffs geprüft); Zusammenhang bereits in `docs/analysis/vdi-r2-improvement-traced-2026-09-12.md` §2/§3 belegt |
| `sdk/gemlib/overlay/_gc_vdi.c:19` — „handler leaves x0 = 0" | **geändert** — `sdk/gemlib/overlay/_gc_vdi.c:19-27,43` (Kommentar + Inline-Kommentar `:43`) | Seit `675e311`/V16 liefert Selektor 115 `flip_y` (0/1) in `x0` (`kernel/src/arch/aarch64/exceptions.rs`, Arm `115 => i64::from(vdi::dispatch(...))`). `sdk/gemlib/gemlib-0.44.0/_gc_vdi.c` **nicht** angefasst — laut `sdk/.gitignore:13` generiert/vom Overlay überschrieben, keine eigene Quelle |
| `docs/api/vdi/_working-list.md:139` — D-080 „offen" | **geändert** — `docs/api/vdi/_working-list.md:142-150` (Absatz nach dem D-080-Eintrag ergänzt) | Seit `675e311` beantwortet: NewTOS' GEM-Trap liefert `flip_y` jetzt über Selektor 115 |
| Nachzutragen: nicelines-Versionsgatter `4b0d9518` (`len > 3`) | **schon erledigt** — Commit `4aa3bf2` (A46) | `docs/api/aes/object-form.md:256-260` „Version delta" — bereits neben der nicelines-Beschreibung dokumentiert, mit Commit, Datum und Host-Test |
| V16-Nachtrag: Selektorvergleich 64 Bit (NewTOS) vs. Wort (EmuTOS) | **geändert** — `docs/COMPATIBILITY.md:142-155` (neuer Absatz beim GEM-Trap-Eintrag) | Satz ergänzt wie verlangt, **kein Ticket angelegt**: EmuTOS `cmp.w`-Wortvergleich (`references/emutos/vdi/vdi_asm.S:33-52`) vs. NewTOS' voller 64-Bit-Vergleich (`kernel/src/arch/aarch64/exceptions.rs`, `match args.regs[0]`); heute folgenlos, relevant erst für einen künftigen m68k-Thunk |
| V16-Nachtrag: D-VDI-6 in `docs/api/vdi/_dispatcher.md` als offen geführt | **geändert** — `docs/api/vdi/_dispatcher.md:74-80` | Als beantwortet markiert: 115 (VDI) und 200 (AES) sind zwei Selektoren desselben Traps, kein Widerspruch — `docs/COMPATIBILITY.md:42` nennt beide, `kernel/src/arch/aarch64/exceptions.rs` implementiert beide. Dieselbe Korrektur zusätzlich in `docs/api/vdi/_discrepancies.md:25` (D-VDI-6-Zeile) nachgezogen — **eine Begründung stand an zwei Stellen**, beide waren veraltet |
| Q07-Nachtrag: `docs/api/gemdos/file/43-fattrib.md:167,258-260` Backslash-Behauptung | **schon erledigt** — Commit `64ad50a` (Q07) | Geprüft: `:164-166` beschreibt heute korrekt GET auf Ordner mit/ohne Trennzeichen, GET auf Datei mit Trennzeichen (`-34`), GET auf Wurzel (`-33`); `:307-326` trägt die eigene Selbstkorrektur „That was wrong" mit vollem Beleg. Die vom Ticket zitierten Zeilen `:167`/`:258-260` existieren im heutigen Baum nicht mehr in dieser Form — Zeilendrift, kein offener Fehler |
| Q07-Nachtrag: `kernel/src/selftest_emmc.rs:490` unverdrahtet, druckt nur `true`/`false` | **geändert** — `TASK/T06.md` (neuer Absatz nach den „Kleinigkeiten") | Wie angewiesen **nicht** an `selftest_emmc.rs` selbst geändert; Zeile an T06 („Housekeeping") angehängt. Nebenbei die Rückverweis-Zeile in `docs/api/gemdos/file/43-fattrib.md:305` von `TASK/A53.md` auf `TASK/T06.md` korrigiert (A53 hat keine Housekeeping-Sektion; das war ein Verweis ins Leere) |
| r4-Tabellenbau-Nachtrag: `_full-function-inventory.md` Teil II.5 „alle freien Opcodes" widerspricht Teil I | **geändert** — `docs/api/aes/_full-function-inventory.md:837-847` | „alle freien Opcodes" aus Abschnitt A entfernt (nur Opcode 0 ist frei messbar); Korrektur-Absatz mit Beleg (`aes/gemsuper.c:411-417`) ergänzt, Abschnitt B nannte den Alarmfall bereits richtig |
| r4-Auswertung-Nachtrag: A-6 (`wind_get(WF_SCREEN)`) als „bewusste Abweichung" geführt | **geändert** — `docs/api/aes/_full-function-inventory.md:446,892` (heutige Zeilen; Ticket nannte `:443,878`, seither verschoben) | Als gemessener Defekt richtiggestellt, mit Verweis auf `TASK/A54.md`. **`tos-api/src/aes/window.rs`-Kommentar NICHT angefasst** — A54 hat ihn laut eigener Quittung bereits ersetzt (`TASK/A54-STATUS-DONE.MD` „Der Code-Kommentar … ist ersetzt"); im aktuellen Baum verifiziert (Zeilen 2739-2766, benennt EmuTOS' Soll + `docs/COMPATIBILITY.md` + die vier Code-Belege). A54 selbst ist **uncommitted im Baum** (wie A53s Auftragstext auch schreibt) — die Inventar-Korrektur beschreibt also den heutigen Arbeitsstand, nicht zwingend den nächsten Commit |
| `_state.md:17` — „Reference (NewTOS reimplements freely)" | **bleibt** — kein Widerspruch gefunden | Kopfzeile nennt `gl_mowner` gar nicht; der Freiheits-Absatz im Fließtext (`:156-167`) grenzt selbst sauber ab: die Werte/Effekte sind Vertrag, `gl_mowner`/`gl_ctmown`/`ctl_pd` als *Register* bleiben frei — mit explizitem Verweis auf `menu.md`s Vertrag. Geprüft, nicht blind übernommen |
| `event.md:23` — „Reference (NewTOS reimplements freely)" | **bleibt** — kein Widerspruch gefunden | Body (`:175-219`) grenzt den `gl_mowner`-Quick-Check (frei) explizit vom blockierenden Arm und dem in `menu.md` dokumentierten Vertrag ab („See `menu.md` … for the resulting **contract**"); die Kopfzeile selbst behauptet nichts, was dem widerspricht |
| `menu.md:32` — „Reference (NewTOS reimplements freely)" | **bleibt** — kein Widerspruch gefunden | Die Freiheitsliste nennt `do_chg`, `menu_down`/`menu_sr`, die `mn_do`-Zustandsmaschine, `bb_save`/`bb_restore`/`popup_blit`, `gl_mntree`/`gl_mnppd`/… und `ct_msgup` — **nicht** die Mausbesitz-Garantie, die H02 direkt davor (`:32-36`) als eigenen Contract-Punkt einträgt. Kein Freibrief für das, was H02 belegt hat |
| `shell-loop.md:847` — „Reference (NewTOS reimplements freely)" | **schon erledigt**, vor A53 | Heutige Zeile 847 beschreibt nur noch die **bereits erfolgte** Korrektur vom 2026-09-01 (`scrap-shell.md:48-56,494-505`): die Freiheit wurde damals auf die *Mechanik* verengt, die Prozessidentitäts-Nummerierung (`ap_id`/`ap_private`/…) explizit als Vertrag ausgenommen. Kein „Reference (NewTOS reimplements freely)"-Absatz mehr an dieser Stelle selbst |

## Diff-Umfang je Datei

```
 TASK/T06.md                                 |  8 ++++++
 docs/COMPATIBILITY.md                       | 15 ++++++++++++
 docs/STATUS.md                              |  2 +-
 docs/analysis/full-regression-2026-09-12.md |  9 +++++++
 docs/api/aes/_full-function-inventory.md    | 38 ++++++++++++++++++++---------
 docs/api/gemdos/file/43-fattrib.md          |  2 +-
 docs/api/vdi/_discrepancies.md              |  2 +-
 docs/api/vdi/_dispatcher.md                 | 14 +++++++----
 docs/api/vdi/_working-list.md               | 12 +++++++--
 sdk/gemlib/overlay/_gc_vdi.c                | 13 +++++++---
 10 files changed, 90 insertions(+), 25 deletions(-)
```

`sdk/gemlib/gemlib-0.44.0/_gc_vdi.c` (pristine/gitignoriert Kopie desselben
Textes) bewusst **nicht** angefasst — `sdk/.gitignore:13` erklärt den ganzen
`gemlib-0.44.0/`-Baum bis auf zwei Overlay-Zieldateien als generiert;
`build-gemlib.sh` überschreibt sie bei jedem Lauf mit dem Overlay-Stand.

## Out-of-scope observations (nicht geändert, nicht im Auftrag)

- `docs/INVENTORY.md:263,736` — trägt dieselbe längst überholte
  `rsrc_saddr`-No-op-Behauptung wie `docs/STATUS.md:244` (mit noch älterer
  Zeilenangabe `resource.rs:1299-1321`). Nicht im A53-Auftrag benannt, eigenes
  Auditdokument — nicht angefasst.
- `docs/STATUS.md:511-512` — zwei **datierte** Recent-Decisions-Einträge vom
  2026-07-20 nennen `rsrc_saddr` noch als No-op-Stub. Das war an dem Tag
  wahr; als historisches Log absichtlich unangetastet gelassen (Konvention:
  Recent-Decisions-Einträge sind Zeitmarken, keine laufend korrigierte
  Tabelle — anders als die Phase-13-Zusammenfassungszeile, die geändert
  wurde).
- `docs/api/aes/_full-function-inventory.md` A-4-Zeile referenzierte ebenfalls
  die veralteten `resource.rs:1345-1475`-Zeilen (dieselbe Ursache wie A-6);
  mitkorrigiert auf `:2589-2719`, da direkt neben den ohnehin bearbeiteten
  A-6/§2.11-Stellen und derselbe Beleg.

## Nicht angefasst

`TASK/A53.md` selbst (Kopfzeile/Status bleibt unverändert, wie angewiesen).
Kein `git add`/`commit`. Von den zwei erlaubten Code-Ausnahmen nur eine
gebraucht: `sdk/gemlib/overlay/_gc_vdi.c:19` (Kommentar, keine `.rs` — SDK-
Trap-Binding, kein Kernel-/tos-api-/tos-types-Produktcode). Kein Testdatei-
Kommentar war nachweislich falsch, also blieb die zweite Ausnahme ungenutzt.
Keine sonstige `.rs`-/`.c`-Datei angefasst; `kernel/src/selftest_emmc.rs`
**nicht** geändert (Zeile nur an `TASK/T06.md` angehängt, wie angewiesen).
