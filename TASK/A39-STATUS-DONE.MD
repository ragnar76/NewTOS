# A39 — STATUS: Stufe 1 DONE (Hostbeleg + QEMU-Beleg) / Stufe 2 + HW-Abnahme offen

Der Resourcezustand ist jetzt **je AES-Prozess** gehalten (Liste, Auswahl =
zuletzt geladen) statt eines globalen `static`; `terminate_inner` (`Pterm`)
gibt beim Prozessende nur die eigenen Einträge frei. Unabhängig verifiziert
2026-09-19, Tester-Messung (nicht Entwickler-Selbstreport), Branch
`desk-rebuild`, Basis HEAD `fbd6856`. Messlauf:
[`measurements/measurement-2026-09-19-a39/`](../measurements/measurement-2026-09-19-a39/RUNINFO.TXT).

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

## Methode (Kurzform, voll unter `measurements/measurement-2026-09-19-a39/RUNINFO.TXT`)

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
measurements/measurement-2026-09-19-a39/    (Messlauf: RUNINFO.TXT, screendumps/, serial/,
                                build/, host-tests/, method/)
TASK/A39-STATUS-DONE.MD        (diese Quittung)
```

Produktcode (`tos-api/src/aes/resource.rs`, `tos-api/src/bdos/process/
pterm.rs`, `tos-api/src/aes/appl.rs`, `tos-api/src/aes/pd.rs`,
`tos-api/tests/aes_rsrc_global_fields.rs`, `tos-api/tests/
aes_rsrc_per_process.rs`) lag bereits uncommitted im Baum und wurde durch
diese Messung nicht verändert.

## Stufe 2 (Developer, 2026-09-19)

**Gebaut: der Selektor wird aus dem `AESGLOBAL` des Aufrufers gelesen** —
EmuTOS' `rs_sglobe` (`gemrslib.c:673-679`), Spec `resource-fileselector.md`
§ `rsrc_load`, A39-Callout Punkte 3–5. Basis HEAD `fc1bf02` (A60-Overlay),
uncommitted im Baum. Selbstbeleg des Developers; die Abnahme (QED aus
TeraDesk, echte Pi) steht beim Tester.

### Was geaendert ist

`tos-api/src/aes/resource.rs` (Produkt):

- `selected(pid, global)` liest das u32 `ap_rscmem` (Byte 14) aus dem
  `AESGLOBAL` des Aufrufers und liefert den Eintrag **dieses** pid, dessen
  `name` gleich ist; `global == NULL`, 0 bei lauter benannten Eintraegen,
  Blob eines anderen pid, fremde/hybride Adresse → `None` = „nothing loaded".
  Basisvergleich, kein Bereichsvergleich (A60: `0x407000` liegt in
  `desktop.rsc` und nennt keinen Blob).
- `RscEntry { pid, name: u32, latest_unnamed: bool, raw, desc }` statt
  `selected: bool`. `name` = das, was `report_load` als `ap_rscmem`
  geschrieben hat; eine Regel fuer beide Seiten: `published_name(desc)`
  (Basis und Baumfeld passen in 4 Byte → Basis, sonst 0). `register` traegt
  den Namen ein; `report_load` schreibt denselben Wert.
- **Ein unbenannter Blob** (In-Place-Kernelpfad, prä-v7-Blob — A35 Punkt 3,
  dort wird nichts geschrieben) traegt den Namen 0, den der Aufrufer
  behalten hat; der Nullselektor erreicht die zuletzt geladene solche Ladung
  des pid und sonst nichts (`latest_unnamed`, gesetzt beim Laden, geht mit
  dem Eintrag, wird nie neu gesetzt). Ein benannter Blob wird nie von 0
  erreicht, ein unbenannter nie von einem Wert ≠ 0. Das haelt A35' „the
  load itself still succeeds and `rsrc_gaddr` still answers at full width"
  wahr, ohne je einen Wert aufzuloesen, den der Kern nicht publiziert hat;
  auf dem laufenden System gibt es keine unbenannte Ladung (kein
  EL1-Aufrufer), auf dem 64-Bit-Host sind es alle 37 `aes_rsrc_*`-Tests.
- `rsrc_free`/`rsrc_gaddr`/`rsrc_saddr` rufen `selected(pid, pb.global_ptr())`;
  `rsrc_obfix` unveraendert (arbeitet auf dem uebergebenen Baum). `R_TREE`
  kommt aus dem Baumfeld des gewaehlten Blobs = `ap_ptree[i]` (R4; neuer
  In-Crate-Test fuer den ILP32-Fall mit 4-Byte-Slots).
- `appl.rs`/`mod.rs` unberuehrt: `Aespb::global_ptr()` gab es schon.

Tests: `resource.rs` `mod selector_tests` (7, in-crate, weil nur der
access/target-Split dort einen benennbaren Blob unter 4 GiB erzeugt — der
Host kann `load_into_user` nicht fahren, und eine In-Place-Ladung liegt
ueber 4 GiB): (a) X, dann Y laden → `rsrc_gaddr` in Y; Worte auf X
zurueckgestellt (wie `icon.c:2762-2763`) → in **X**, `R_TREE 0` ==
`ap_ptree[0]` von X; (b) `0x407000`, `0x12345678`, 0 → 0/0, `rsrc_free` 0,
nichts freigegeben; (c) Blob von pid 2 in den Worten von pid 1 → 0, pid 2
unberuehrt; (d) `rsrc_free` mit X benannt gibt X frei, Y bleibt und ist
erreichbar, sobald Y benannt ist, Worte bleiben stehen; (e) `global == NULL`
→ 0/0, nichts freigegeben; (f) zweite Ladung benennt sich selbst; (g) der
unbenannte Fall. `mod tests` +1: `ap_ptree`-Slots (4 Byte) == `rsrc_gaddr
(R_TREE, i)` fuer ILP32. Doku-Kommentare in `tests/aes_rsrc_per_process.rs`
und `tests/aes_rsrc_global_fields.rs` nachgezogen (kein Testcode geaendert).
Spec: nur der Statusabsatz des A39-Callouts („built, both stages").

### Messungen

| # | Messung | Ergebnis |
|---|---|---|
| 1 | Rot vor Fix: die 7 `selector_tests` auf HEADs `resource.rs` (Scratch-Kopie, nur die Testmodule aufgepfropft) | **6 FAILED** — exakt die Vorhersage: Zuruecksetzen loest weiter in Y; `0x407000` loest; Blob des anderen pid loest; `rsrc_free` trifft die letzte statt der benannten; Null-`global` loest (`left: (1, 4194400)`); 0 erreicht einen benannten Blob. 1 ok (zweite Ladung benannt — galt schon in Stufe 1) |
| 1 | Gruen nach Fix, in-crate `aes::resource` | **15/15** |
| 1 | `aes_rsrc_*` (8 Dateien, 37 Tests) + `aes_global_layout`, `aes_app_return_descriptor`, `aes_launch_boundary_reset`, `aes_shel_roundtrip`, `aes_appl_init` | **alle gruen** |
| 2 | `cargo run -p xtask -- test` | **610 bestanden / 0 gefallen / 1 uebersprungen** (Baseline 602 + 8 neue) |
| 3 | rustc beide Boards (frisch, `cargo clean -p kernel -p tos-api --target aarch64-unknown-none-softfloat` vor jedem Wechsel) | tos-api **64**, kernel **94** — Delta 0 |
| 3 | Clippy | kernel **125**, tos-api **81** — Delta 0 (die fuenf `resource.rs`-Meldungen sind die vorbestehenden `Aespb`-Sichtbarkeitswarnungen) |
| 4 | QEMU raspi4b, **neue `DESKTOP.NAP` `fb37bb3b`** per `#Z`, A60-Instrument + Injektor (HARD DISC C doppelklicken) | Nach `rsrc_load(desktop.rsc)` Worte `4400 4000 / 0000 4000 / 3699`; erster Trap nach `load_icons`: **keine Dump-Zeile** (Worte unveraendert), Statics `0x427028/0x427000`. **Alle 91 `op 112` von pid 1 = 1** (62 vor, 29 nach `load_icons`; die 29 loesen `0x408739..0x4088ff`, alle in `desktop.rsc`). Screendump Leerlauf **bytegleich** zu `measurements/measurement-2026-09-19-a60/fix/screendumps/fix1-after-boot.png` (sha256 `fb1e8b90…`); Fenster offen mit Infozeile **„374035 bytes used in 24 items."** und Ordner-/Dokument-Icons. Kein EXCEPTION/panic. |
| 5 | Gegenprobe **alte `DESKTOP.NAP` `db31aae1`** (ohne A60), gleiche Sequenz | **Der erwartete Ausfall, keine Regression**: nach `load_icons` Worte `2870 4200 / 0070 4000` (`ap_rscmem` = `0x407000`), **alle 28 `op 112` danach = 0** (`addrout 0x0`), die 62 davor = 1; Fenster offen mit **leerer Infozeile** und generischen Disketten-Icons (die H05-Symptomatik). Beleg, dass der Selektor wirklich gelesen wird. Kein EXCEPTION/panic. |

Artefakte: `measurements/measurement-2026-09-19-a39/stage2/` (RUNINFO-STAGE2.TXT, method/,
serial/, screendumps/, build/ mit den drei Kernel-Images, host-tests/,
manifest.sha256).

### Nicht gefahren / offen

- QED aus TeraDesk starten und beenden (die Stufe-1-Sequenz) unter Stufe 2 —
  Tester. Erwartung: unveraendert (QED biegt seine Worte nicht um, pid 2 hat
  eine eigene Liste; TeraDesks Worte nennen `desktop.rsc`).
- `free_icons`/`rsrc_free()` beim Beenden von TeraDesk (Geste): Erwartung
  `rsrc_free` = 1 (Worte = `0x400000`).
- Echte Pi 4 (HW-Abnahme A39 insgesamt).
- `docs/STATUS.md`-Eintrag (Lockstep-Regel) — nicht im Auftragsumfang, vom
  Lead beim Commit nachzuziehen.
