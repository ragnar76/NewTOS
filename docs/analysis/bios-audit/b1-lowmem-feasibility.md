# B1 — Machbarkeitsanalyse: Low-Memory-Sysvars + OSHEADER + `$5A0` per-Prozess in EL0-VA

> Stand: 2026-07-24, Branch `desk-rebuild`. Evidenzbasierte Feasibility-Analyse
> (Auditor, Read/Grep, kein Produktcode geändert). Beantwortet die vom User
> „nach dem Audit" terminierte Frage: **geht das technisch → implementieren,
> sonst App-Patch notieren.** Trigger: `../sysvar-osheader-lowmem-gap.md`,
> BIOS-Finding B1/B1a/B1b (`bios-findings.md`).

## Verdikt (Kurz)

**FEASIBLE-WITH-CAVEATS — kein harter Blocker.** Alle vier Bausteine (Low-Seite
mappbar, App verschiebbar, Populate am Loader, Setexc-Peek) sind mit dem
existierenden Code erreichbar. Der einzige „Preis" ist ein **Linker-Base-Bump +
Recompile aller nativen Apps** — die projekt-sanktionierte Toolchain-Route
([[port-target-app-unmodified]]), **kein** Quell-Patch. Damit ist die User-Regel
„technisch machbar → implementieren" mit **JA** beantwortet; die App-Patch-Route
ist nicht nötig.

## Kern-Erkenntnis

Die eigentliche Kollision ist **nicht** eine fehlende Seite, sondern dass NewTOS
Basepage + App-Text **und** den klassischen Sysvar-Bereich auf **dieselben
niedrigen EL0-VAs** gelegt hat. Im klassischen TOS liegen die Sysvars absolut in
Low-Memory und die TPA weit darüber; NewTOS lädt die TPA ab VA 0 → `sysbase@0x4F2`
fällt mitten ins App-`.text`. `*(long*)0x4F2` liest App-Code-Bytes (Müll) → Fault
(reproduziert den TeraDesk-Crash exakt).

## Belegte Ist-Fakten (file:line)

| # | Fakt | Beleg |
|---|---|---|
| A1 | TPA ab EL0-VA 0; Basepage `[0x000,0x100)` (256 B); App-`.text` ab `0x100` | `loader.rs:160` (`USER_VA_BASE=0x0`), `:484-493`; `tos-types/src/basepage.rs:178`; `sdk/newtos.ld:41` |
| A2 | Sysvar-Bereich `0x400–0x5FF` liegt **mitten im App-`.text`**; `0x4F2` = TPA-Offset 0x3F2 tief im Code | Rechnerisch + reproduzierter Fault (`sysvar-osheader-lowmem-gap.md:28-42`) |
| A3 | Loader **relokiert nicht**; Apps ET_EXEC mit fixer Link-VA (reine SDK-Wahl) | `loader.rs:441` (`dest_off = seg.vaddr - USER_VA_BASE`) |
| A4 | crt0 liest Basepage **hart bei VA 0** (`p_hitpa@0x04` über Nullbasis) | `sdk/crt0.S:40-42` |
| A5 | Desktop-/AUTO-Launch-Pfad setzt **x0 nicht** (nur svc-Pexec + Scheduler-Switch tun das) | `kernel/src/arch/aarch64/user_entry.rs`; `exceptions.rs:939,430`; GEM-Launch via `launch.rs::run_program` |
| A6 | MMU 4-KiB-Granularität; **Seite 0 heute schon per-Prozess RW-exec gemappt** → Sysvar-Reservierung kostet MMU-seitig nichts Neues | `mmu.rs:70-77,124-131,272-277` |
| A7 | EmuTOS-`setexc` hat **keine** Bereichsprüfung — roher Peek/Poke `*(LONG*)(4*num)` für jedes `num`; NewTOS' `NVEC=0x120`-Shadow ist die Deviation, die `0x168` blockt | `references/emutos/bios/bios.c:1070-1080`; `tos-api/src/bios/vectors.rs:21,74` |
| A8 | Loader schreibt die Low-Region **schon per-Prozess** (Basepage via `tpa.base_kva`) → derselbe Mechanismus trägt OSHEADER+Sysvars+Jar | `loader.rs:562-628,430` |

## Sub-Fragen

1. **Null-/Low-Page:** FEASIBLE, ohne Kosten. Seite 0 ist **kein** Null-Guard
   (heute schon RW-exec gemappt, Basepage liegt darin) → nichts zu verlieren;
   reine Populate-Operation in eine bereits per-Prozess-eigene Seite.
2. **App-Load-Kollision:** FEASIBLE per **Linker-Base-Bump** (Toolchain, kein
   App-Patch). Sysvar-Adressen sind ABI-fest (`0x4F2`/`0x5A0`) → das App-Image
   muss über den Sysvar-Bereich (höchster dokumentierter Sysvar `kcl_hook@0x5B0`,
   gelöschte Region bis `0x800`). `newtos.ld:41` `. = 0x100` → `. = 0x1000`.
   TeraDesks `*(long*)0x4F2` bleibt unangetastet und liest danach gültige Daten.
3. **Populate-Kosten:** FEASIBLE, ein Ort (`loader.rs:412` neben
   `populate_basepage`), per-Prozess. Statisch befüllbar: `sysbase@0x4F2`,
   OSHEADER-Felder, `p_cookies@0x5A0`, `drvbits@0x4C2`, `bootdev@0x446`,
   `memvalid`/`memval2`/`memval3`, `phystop`/`membot`, `resvalid`/`resvector`=0.
   **Live-Caveat:** nur `_hz_200@0x4BA` ist dynamisch; kein existierender
   hz_200-Low-Memory-Writer im Baum → Neuarbeit, aber unter Single-Tasking
   ([[singletasking-app-owns-full-screen]]) schreibt der EL1-Timer-ISR direkt in
   die aktive TTBR0. TeraDesk pollt hz_200 nicht live → kein Blocker.
4. **`Setexc($5A0/4,-1)` (B1a):** FEASIBLE, EmuTOS-treu. Sobald `p_cookies@0x5A0`
   in der per-Prozess-Low-Seite steht, liest/schreibt `setexc` für die Sysvar-
   Range `*(LONG*)(4*num)` aus der VA des aufrufenden Prozesses (Kernel läuft im
   svc-Handler mit aktiver App-TTBR0). Treuer Fix: EmuTOS-Verhalten, auf die
   Sysvar-Region gebounded statt echtes m68k-Corruption.

## Empfohlene Layouts (Lead-/User-Design-Entscheidung)

### Option 1 — minimal-invasiv (Basepage bleibt bei VA 0) — Auditor-Empfehlung

```
EL0-VA (per-Prozess-TPA, erste 4-KiB-Seite schon gemappt):
  0x000–0x0FF   BasePage (256 B)          ← unverändert; crt0 liest p_hitpa@0x04
  0x100–0x3FF   m68k-CPU-Vektor-Raum      ← ungenutzt (kein nativer App-Leser)
  0x400–0x5B4   klassische Sysvars        ← per-Prozess vom Loader befüllt
  0x5C0–0x5EB   OSHEADER-Instanz (44 B)   ← sysbase@0x4F2 zeigt hierher
  0x5F0–0x66F   Cookie-Jar (Slots)        ← p_cookies@0x5A0 zeigt hierher
  0x1000+       App .text/.data/.bss/heap/stack   ← newtos.ld base = 0x1000
```
`p_lowtpa=0`, `p_tbase=0x1000`. **crt0 + user_entry unverändert.** Einzige
Deviation: Basepage @0x000 überlappt den m68k-CPU-Vektor-Bereich (0x000–0x3FF),
den **kein nativer App liest** → unbeobachtbar.

**Touch-Points Option 1:**
1. `sdk/newtos.ld:41` `. = 0x100` → `. = 0x1000` + App-Recompile.
2. `loader.rs` — `validate_segment` min_va (`:766`) → 0x1000; neue
   `populate_sysvars()` in `populate_and_map` (`:412-535`) via `tpa.base_kva`.
3. `tos-api/src/bios/vectors.rs` — `setexc` (`:71-82`) für Sysvar-Range
   EmuTOS-treu (Peek/Poke aktive TTBR0), NVEC-Grenze anheben.
4. Optional: Timer-ISR schreibt `_hz_200@0x4BA` (nur falls Apps live pollen).

### Option 2 — klassik-treu (TPA hoch, 0x000–0x800 rein Low-Memory)

Basepage + Text hoch (z. B. 0x10000), 0x000–0x7FF rein Sysvars/OSHEADER/Jar
(`p_tbase = p_lowtpa+0x100`, exakt klassisch). Vermeidet die CPU-Vektor-
Überlappung. **Mehr Touch-Points:** zusätzlich `sdk/crt0.S:40-42` (Basepage aus
x0), `user_entry.rs` (x0=basepage auf Kernel-Launch-Pfad, A5), `exceptions.rs:430`,
`usermem.rs:64` (`WINDOW_BASE`), `seed_tpa`. Architektonisch sauberer, riskanter.

## Unabhängige offene Werte (keine Machbarkeitsfrage)

- **`os_version`-Zahlenwert** (0x0404?) — separate Entscheidung (Finding S3),
  Cross-Check reales Falcon-TOS4. Machbarkeit unberührt.
- **`_hz_200`-Live-Handling** — nur nötig, falls eine Ziel-App hz_200 direkt
  pollt (TeraDesk nicht).

## Konfidenz & Nicht-Verifiziertes (ehrlich)

- **Bewiesen** (Code + EmuTOS-Quelle gelesen): A1–A8, die 0x4F2-Kollision
  rechnerisch + gegen den reproduzierten TeraDesk-Fault.
- **Nicht per QEMU nachgestellt:** dass ein bei `.=0x1000` neu gelinktes Image +
  befüllte Sysvar-Seite den TeraDesk-Versions-Check real passiert — der Beweis
  kommt erst mit Implementierung + Selftest/QMP-Lauf (Frage war reine Machbarkeit).
- **Nicht auditiert:** ob Apps über TeraDesk hinaus (QED …) Sysvars < 0x100 oder
  echte m68k-CPU-Vektoren lesen (würde Option 1s Basepage@0x000 stören). Nur
  TeraDesks Zugriffe (alle ≥ 0x400) geprüft.
- **Out-of-scope-Beobachtung:** `setexc` für `num>=NVEC` gibt heute 0 statt
  EmuTOS' rohem Peek — generelle Setexc-Deviation (jeder `4*num`-Peek-Nutzer),
  gehört in den B1a-Fix.
