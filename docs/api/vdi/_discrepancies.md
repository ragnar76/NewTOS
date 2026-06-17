# VDI source-vs-doc discrepancies (Phase 00b)

> Mirrors `docs/api/gemdos/_discrepancies.md`. Logged when the EmuTOS
> source disagrees with the secondary references (Compendium /
> Profibuch) or is internally surprising. **EmuTOS is authoritative**
> (CLAUDE.md Rule 1); the spec follows the source and the discrepancy is
> recorded here for traceability.

| # | Function | Source behaviour (EmuTOS `1a7484ab`) | Note |
|---|----------|--------------------------------------|------|
| V-1 | `vqm_attributes` (36) | Returns `INTOUT[0] = vwk->mark_index` **0-based**, while `vql_attributes`/`vqf_attributes` return their index `+1` and `vsm_type` stores `type-1`. So a round-trip `vsm_type(n)` → `vqm_attributes` reports `n-1`, not `n`. | Internal asymmetry in EmuTOS. Preserved verbatim as the source behaviour. **TODO:** cross-check the intended value against the Compendium `vqm_attributes` entry before Phase-12 implementation. See [`inquiry/_attributes.md`](inquiry/_attributes.md). |
