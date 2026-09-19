# A39 — STATUS: Stufe 1 DONE (Hostbeleg + QEMU-Beleg) / Stufe 2 + HW-Abnahme offen

Der Resourcezustand ist jetzt **je AES-Prozess** gehalten (Liste, Auswahl =
zuletzt geladen) statt eines globalen `static`; `terminate_inner` (`Pterm`)
gibt beim Prozessende nur die eigenen Einträge frei. Unabhängig verifiziert
2026-09-19, Tester-Messung (nicht Entwickler-Selbstreport), Branch
`desk-rebuild`, Basis HEAD `fbd6856`. Messlauf:
[`measurement-2026-09-19-a39/`](../measurement-2026-09-19-a39/RUNINFO.TXT).

Diese Quittung dokumentiert die **Prüfung**, nicht die Implementierung (die
lag beim Start der Messung bereits uncommitted im Baum: `tos-api/src/aes/
resource.rs`, `tos-api/src/bdos/process/pterm.rs`, dazu Doku-Änderungen in
`appl.rs`/`pd.rs`/`aes_rsrc_global_fields.rs` und neu `tos-api/tests/
aes_rsrc_per_process.rs`). Auftrag: A39-Stufe-1-Fix unabhängig verifizieren,
dem Developer-Selbstreport nicht trauen.

## Was gebaut wurde (Kurzform, siehe `resource.rs`-Modulkopf und
`docs/analysis/rsrc-per-app-state-2026-09-19.md` § 6 für die volle Analyse)

EmuTOS hält den Resourcezustand vollständig in der Anwendung selbst (den drei
`AESGLOBAL`-Worten, `rs_sglobe` liest sie bei jedem Aufruf neu ein,
`gemrslib.c:673-679`). NewTOS kann diesen Zustand nicht aus dem
Anwendungsspeicher lesen (Blob-Zugriff über den Kernel-Alias, ABI-abhängige
Satzbreite) und hält deshalb einen eigenen Deskriptor — **Stufe 1: je AES-pid
eine Liste, Auswahl = zuletzt geladener Eintrag**. Stufe 2 (nach A60) liest
den Selektor aus dem Aufrufer-`AESGLOBAL` (`ap_rscmem`) zurück, wie EmuTOS'
`rs_sglobe`.

## Die Messungen — alle selbst gefahren, keine übernommen

| # | Messung | Ergebnis |
|---|---|---|
| 1 | `aes_rsrc_per_process` (Fix) | **5/5 grün** |
| 1 | Rot-Gegenprobe, `git worktree` auf HEAD `fbd6856` (vor dem Fix) | **2 FAILED** — exakt die vorhergesagten Fälle: (a) Eltern-`rsrc_gaddr` liefert `0` statt `1`; (b) neuer Mieter des wiederverwendeten Slots erbt die Ressource des Vorgängers (`left: (1, 535369367648), right: (0, 0)`) |
| 1 | Alle `aes_rsrc_*` + `aes_global_layout` + `aes_app_return_descriptor` + `aes_launch_boundary_reset` + `aes_shel_roundtrip` | **40/40 grün** (11 Testdateien) |
| 2 | `cargo run -p xtask -- test` | **575 bestanden / 0 gefallen / 1 übersprungen** — Sollstand exakt |
| 3 | rustc-Warnungen, beide Boards (frisch, `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` vor jedem Wechsel) | tos-api **64**, kernel **94** — Baseline (A66), Delta 0 |
| 3 | Clippy-Warnungen, beide Boards | kernel **125**, tos-api **81** — Baseline (A66), Delta 0 |
| 4 | QEMU raspi4b, TeraDesk startet QED per Doppelklick (`op 110`/`111` von pid 2), beendet über Datei→Beenden, Rückkehr | **op 110/111 (QED, pid 2) → 1/1; alle 7 `op 112` (TeraDesk, pid 1) danach → `out[0]=1`, 0 Treffer für `int_out[0]=0`.** Infozeile "610314 bytes used in 2 items" vor Start und in allen vier Nachher-Screendumps (0s/2s/5s/10s) identisch; Pixel-Diff Vorher/Nachher: 107 von 307200 Pixeln, ausschließlich der Mauszeiger — **0 Pixel Unterschied in Infozeile/Icons**. Kein EXCEPTION/panic. |
| 5 | Rauchtests ohne Injektor: `#Z` TeraDesk, `#Z` QED direkt, unser Desktop ohne Autorun | alle drei starten sauber (`[shell] sh_ldapp: ...` bzw. `[desk] desktop up (1 drives)`), kein panic/EXCEPTION, Screendumps zeigen den erwarteten Bildschirm |
| 6 | `git status --short` | genau die sechs Auftragsdateien + mein eigener Messordner; `kernel/src/host_env.rs` sha256 vorher = nachher (`287ff741...`) |

## Warum QED und nicht MINIGEM.NAP

`MINIGEM.NAP` (H05/H05-Verify) ruft nie `rsrc_load`/`rsrc_free` — A39 greift
dort nicht (im Trace kein `op 110`/`111`). `QED.NAP` ruft `rsrc_load` über
`cflib`s `appinit` (bereits im H05-Verify-Smoketest bestätigt: "cflib appinit
-> shel_read -> gl_appdir -> rsrc_load funktioniert weiterhin"). Nur mit QED
als Kind ist der A39-Mechanismus (TeraDesks Ressource wird vom Kind
überschrieben/gelöscht) überhaupt beobachtbar — deshalb die Wahl im Auftrag.

## Methode (Kurzform, voll unter `measurement-2026-09-19-a39/RUNINFO.TXT`)

Scratch-Kopie des Baums, die drei H05-Instrument-Diffs (`aes/mod.rs`
pid+opcode-Trace, `aes/pd.rs`, `host.rs`) trafen unverändert (`patch -p0`,
kein Fuzz); der `exceptions.rs`-Teil (Pexec/Pterm-Trace) von Hand
nachgezogen. **Keine eigene Instrumentierung in `resource.rs` nötig** — der
bestehende Dispatch-Trace (`[aes] pid=<pid> op <opcode> ... out=(...)`) liegt
bereits vor/nach jedem AES-Aufruf inkl. `rsrc_load`/`rsrc_free`/`rsrc_gaddr`
und trägt die aufrufende pid, das war exakt das verlangte Signal. Koordinaten
für HARD DISC C → APPS → QED → QED.NAP → Datei-Menü → "Beenden" wurden in
fünf kurzen Recon-Läufen selbst am laufenden Bild ermittelt (nicht aus einer
älteren Messung übernommen), da QED sein eigenes (deutsch lokalisiertes)
Vollbild-Menü zeigt, das in keiner bisherigen Messung koordiniert wurde.

## Was offen bleibt

1. **Stufe 2 (nach A60).** Auswahl über den Aufrufer-`ap_rscmem` statt
   "zuletzt geladen" — erst sinnvoll messbar, sobald TeraDesks eigene
   `load_icons`-Sequenz (A60) die Strings korrekt zurückliest. Bis dahin
   funktioniert Stufe 1 für jede Anwendung, die ihr `AESGLOBAL` nicht selbst
   umbiegt (das betrifft QED nicht, TeraDesk biegt `global[5..8]` selbst um —
   A39.md § Analyse, A60-Abhängigkeit).
2. **HW-Abnahme auf echtem Pi 4.** QEMU `raspi4b` hat keinen Eingabeweg für
   echte Maus/Tastatur — dieser Lauf nutzte den QUIRK-frei funktionierenden
   IKBD-Skriptinjektor (`kernel/src/host_env.rs`, scratch-only), keine echte
   HID-Eingabe. Auf realer Hardware zu wiederholen: QED aus TeraDesk
   starten/beenden über echte Maus, Infozeile und Icons müssen unverändert
   bleiben. Route: `docs/api/desktop/_hw-test-checklist.md`.
3. Cache-Kohärenz- und TCG-spezifische Effekte (QUIRK-001 TTBR0-Wedge,
   Non-Cacheable-FB-Klasse) sind durch diesen QEMU-Lauf nicht abgedeckt — für
   diesen rein AES-logischen Fix ohne Framebuffer-Cache-Beteiligung nicht
   einschlägig, der Vollständigkeit halber genannt.

## Aus dem Skopus, nicht angefasst

- `post_msg`-Trace (`aes/pd.rs`-Diff) wurde mitinstrumentiert, aber nicht
  ausgewertet — für den A39-Beleg nicht gebraucht.
- Die deutschen QED-Menütexte ("Datei", "Beenden", "Sichern & Beenden") sind
  Teil des unveränderten, portierten QED — nicht bewertet, nur als Koordinate
  benutzt.

## Berührte Dateien (durch diese Messung neu, kein Produktcode)

```
measurement-2026-09-19-a39/    (Messlauf: RUNINFO.TXT, screendumps/, serial/,
                                build/, host-tests/, method/)
TASK/A39-STATUS-DONE.MD        (diese Quittung)
```

Produktcode (`tos-api/src/aes/resource.rs`, `tos-api/src/bdos/process/
pterm.rs`, `tos-api/src/aes/appl.rs`, `tos-api/src/aes/pd.rs`,
`tos-api/tests/aes_rsrc_global_fields.rs`, `tos-api/tests/
aes_rsrc_per_process.rs`) lag bereits uncommitted im Baum und wurde durch
diese Messung nicht verändert.
