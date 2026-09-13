# A25 — Status

**Status: UNFINISHED** · 2026-09-10 · appl_init Wiederaufruf

Implementierung abgeschlossen und hostverifiziert; reale Gegenprobe fehlt.

- Neuer Test zunächst rot: eine vor appl_init wartende Nachricht ging verloren.
- Minimaler Fix in pd::open: Flags ORen, Queue erhalten; neue Slots separat initialisiert.
- Integrationstest für TT- und Full-HD-Profil sowie Unit-Test für Flags/Queue/vollen Pool grün.
- `cargo run -p xtask -- test`: **429 bestanden**, 0 fehlgeschlagen; ein bestehender Doc-Test ignoriert.
- `cargo build -p tos-api --target aarch64-unknown-none-softfloat`: Exit 0.
- Produktionsänderung nicht deployed, kein Commit erzeugt.

Details, Quellbelege und nächste Hardware-Gegenprobe: [A25](A25.md).
