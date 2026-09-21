//! V11 / V12 (VDI-OVL-001): `v_clrwk` → `v_get_pixel` must report the
//! Falcon truecolor word `0xFFFF`, not the pen-0 word `0xFFDF`.
//!
//! Separate test binary on purpose: it installs a host whose raster backend
//! is the *kernel's* device-pixel model (`vdi_reference/overlay_bit.rs`),
//! and `tos_api::host::install_host` takes one host per process — the
//! `vdi_reference` binary already owns its own. See the module header for
//! the EmuTOS contract, the measured reference and what has to change.
//!
//! Named `vdi_reference_…` so that it sorts **after** `vdi_raster_reference`
//! and `vdi_reference` among the `tos-api` test binaries: `cargo xtask test`
//! runs fail-fast, and a red binary earlier in the alphabet would leave the
//! two reference suites unrun. Red on purpose until VDI-OVL-001 is fixed.

#[path = "vdi_reference/overlay_bit.rs"]
mod overlay_bit;
