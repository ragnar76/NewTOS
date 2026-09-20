# A57 — Status

**Status: UNFINISHED** · 2026-09-20 · Entwurf fertig, Code nicht begonnen

Der Auftrag „Erst Entwurf, dann Code" ist zur Hälfte erledigt: der Entwurf
liegt vor, die zwei Entscheidungen sind mit Beleg vorgelegt, der Bau ist
nicht begonnen. Nichts im Produktcode wurde angefasst, kein Commit.

Entwurf: [`docs/analysis/a57-aes-screen-buffer-design-2026-09-20.md`](../docs/analysis/a57-aes-screen-buffer-design-2026-09-20.md).

## Vorgelegt

- **Pool (Entscheidung 1):** ein Block aus dem ST-/TT-RAM-Pool per
  `mxalloc(…, MX_PREFTTRAM)` = EmuTOS' `dos_alloc_anyram`
  (`util/gemdos.c:117-120`), belegt in `sh_tographic` vor dem ersten
  Prozess, damit kernel-eigen (`KERNEL_PD_SENTINEL`, überlebt jedes `Pterm`),
  seitenausgerichtet mit einer Seite Überbelegung wie `rsrc_load`. Statischer
  Kernelbereich abgelehnt (Größe nur zur Compile-Zeit; Linker-Reserve 952 KiB).
- **EL0-Abbildung (Entscheidung 2):** lazy beim ersten `wind_get(WF_SCREEN)`
  des Prozesses über `usermem::expose(slot, kva, len, ProtMode::G)` — das
  vorhandene `Getbpb`/`Physbase`-Muster (ADR-023), RW, cacheable,
  idempotent je Slot, `Kind::Shared`. Keine feste VA über Prozesse hinweg
  (Bump-Cursor; für Programme unsichtbar). EL1-Aufrufer erhält die PA
  (ADR-021). Ohne Puffer bleibt die A54-Antwort FALSE.
- **Größe:** `len = stride16(W)·H·2` Byte — ein Schirm in Gerätepixeln:
  614 400 (640×480) / 4 147 200 (1920×1080). Überlauf damit unerreichbar.
  Minimalvariante 212 992 Byte mit Überlaufregel als Alternative genannt.
- **Bauliste** mit Abnahmen (Puffer, Wrapper mit Wächter, `form.rs`,
  `menu.rs` = A71, `wind_get`, Spec/COMPATIBILITY, Hosttests, Fall 166 aus
  `SANCTIONED`, EL0-Schreibprobe in QEMU, TeraDesk-Screendump).

## Zwei Befunde, die das Ticket ändern

1. **Die Ticket-Prämisse „bei 1920×1080 passen unsere Dropdowns nicht
   hinein" ist für die gemessenen Anwendungen falsch.** Tafelgröße =
   Zeichenraster × Fontzelle, und die Fontzelle ist in beiden Profilen 8×16;
   EmuTOS' Formel ergibt an unserem Profil 163 840 Byte, TeraDesk (129 920),
   QED de (140 608), QED en (129 792), zView (86 912) passen. Was nicht
   passt, ist der theoretisch größte Alarm (172 800). Die Abweichung bleibt
   begründet, aber mit dem richtigen Grund.
2. **TeraDesk liest heute die Länge und löscht bei 0 dreizehn
   Menüeinträge** (`sdk/apps/teradesk/resource.c:307-320`, `_MENUDEL 1`).
   Mit A54s `FALSE` ist die Länge 0. Aus dem Code gefolgert, am Schirm noch
   nicht nachgemessen — ist als Abnahme im Entwurf (§5.7) vorgesehen.

## Offen

- Anwenderentscheidung: ein Schirm (4 MiB Fenster im fragenden Prozess bei
  1080p) oder Bedarfsdeckel (208 KiB, zweite beobachtbare Abweichung).
- Anwenderentscheidung: Untermenü/Popup transient aus dem Pool (EmuTOS) oder
  zweite Statik im Device-Format.
- Der Bau selbst (Developer), danach QEMU-Schreibprobe und
  TeraDesk-Screendump; keine Hardwareabnahme nötig, der Fall ist rechnerisch.

## Nicht angefasst

Produktcode, Specs, `docs/COMPATIBILITY.md`, `docs/STATUS.md`, `TASK/A57.md`
(Kopfzeile unverändert), `tosdump/`, `measurement-*`. Kein `git`-Befehl, der
den Baum verändert. Lesewerkzeug für die RSC-Vermessung liegt im
Session-Scratch, nicht im Repo.
