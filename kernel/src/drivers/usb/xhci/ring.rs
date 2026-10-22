//! Producer side of an xHCI TRB ring (Command and Transfer Rings).
//!
//! Pure index / cycle-bit arithmetic with no register or DMA access, so the
//! host test suite (`tools/xtask`, which includes this file by path) runs its
//! tests; `xhci/mod.rs` owns the ring memory and does the writes.
//!
//! Source: xHCI 1.2 (Intel, May 2019; `references/xhci/xhci-1.2.pdf`):
//! * §4.9.2 Transfer Ring Management — the producer writes each TRB's Cycle
//!   bit from its Producer Cycle State (PCS); a Link TRB with Toggle Cycle
//!   makes the producer toggle PCS and the consumer toggle its CCS.
//! * §4.9.3 Command Ring Management — "identical to Transfer Rings" in this
//!   respect: software "toggl[es] the Cycle bit each pass through the Command
//!   Ring".
//! * §4.11.5.1 Link TRB — "A Link TRB shall be the last TRB of each Transfer
//!   or Command Ring Segment"; in a single-segment ring it points to the
//!   segment's first TRB; "The Toggle Cycle flag should be set in at least one
//!   Link TRB of a ring".
//! * §6.4.4.1, Figure 6-38 / Table 6-86 — Link TRB layout: dwords 0/1 = Ring
//!   Segment Pointer (16-byte aligned), dword 3 bit 0 = Cycle, bit 1 = Toggle
//!   Cycle, bits 15:10 = TRB Type (6 = Link, Table 6-91).
//!
//! The Event Ring is the other direction (the controller produces, software
//! consumes) and has **no** Link TRB: its segments come from the ERST and the
//! consumer toggles CCS when its dequeue index wraps (§4.9.4). That side lives
//! in `Xhci::poll_event`.

/// TRB Type of a Link TRB (xHCI 1.2 Table 6-91).
pub const TRB_TYPE_LINK: u32 = 6;
/// Link TRB dword 3, bit 1: Toggle Cycle (xHCI 1.2 Table 6-86).
pub const LINK_TOGGLE_CYCLE: u32 = 1 << 1;
/// TRB dword 3, bit 0: Cycle (every TRB type; Table 6-86 for the Link TRB).
pub const TRB_CYCLE: u32 = 1;

/// `data` with its Cycle bit set to the producer cycle state `pcs`.
pub fn stamp_cycle(mut data: [u32; 4], pcs: bool) -> [u32; 4] {
    if pcs {
        data[3] |= TRB_CYCLE;
    } else {
        data[3] &= !TRB_CYCLE;
    }
    data
}

/// The Link TRB that closes a single-segment ring: points back at the
/// segment base `base` (the controller-visible DMA address), Toggle Cycle set,
/// Cycle bit = `pcs` so the consumer accepts it on this pass.
pub fn link_trb(base: u64, pcs: bool) -> [u32; 4] {
    let data = [
        // Ring Segment Pointer bits 3:0 are RsvdZ (16-byte aligned segment).
        (base as u32) & !0xf,
        (base >> 32) as u32,
        // Interrupter Target 0 (ignored on the Command Ring, Table 6-85).
        0,
        (TRB_TYPE_LINK << 10) | LINK_TOGGLE_CYCLE,
    ];
    stamp_cycle(data, pcs)
}

/// Enqueue one TRB on a single-segment producer ring of `len` TRBs whose last
/// slot (`len - 1`) is reserved for the Link TRB.
///
/// `enq` / `pcs` are the producer's enqueue index and cycle state. `write` is
/// called with `(index, trb)` for every TRB to store — first the work TRB,
/// then, when the enqueue index reaches the Link slot, the Link TRB. After
/// the Link TRB the producer continues at index 0 with PCS toggled, exactly as
/// the consumer will after following the Link TRB's Toggle Cycle (§4.9.2).
///
/// Returns the index the work TRB was written to.
pub fn push(
    enq: &mut usize,
    pcs: &mut bool,
    len: usize,
    base: u64,
    data: [u32; 4],
    mut write: impl FnMut(usize, [u32; 4]),
) -> usize {
    debug_assert!(len >= 2, "a ring needs one work slot plus the Link TRB");
    let at = *enq;
    write(at, stamp_cycle(data, *pcs));
    *enq += 1;
    if *enq == len - 1 {
        write(len - 1, link_trb(base, *pcs));
        *enq = 0;
        *pcs = !*pcs;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u64 = 0x1_2345_6780;

    /// A consumer modelled on xHCI 1.2 §4.9.2: it owns the TRB at its dequeue
    /// index while the TRB's Cycle bit equals its CCS; a Link TRB moves it to
    /// the Ring Segment Pointer and, with Toggle Cycle, toggles CCS.
    struct Consumer {
        deq: usize,
        ccs: bool,
    }

    impl Consumer {
        fn new() -> Self {
            // CRCR RCS = 1 / Endpoint Context DCS = 1 — the driver's setting.
            Consumer { deq: 0, ccs: true }
        }

        /// Fetch the next work TRB, following Link TRBs. `None` = ring empty
        /// (the controller stops and waits for the next doorbell).
        fn fetch(&mut self, ring: &[[u32; 4]]) -> Option<[u32; 4]> {
            for _ in 0..ring.len() {
                let t = ring[self.deq];
                if ((t[3] & TRB_CYCLE) != 0) != self.ccs {
                    return None;
                }
                if (t[3] >> 10) & 0x3f == TRB_TYPE_LINK {
                    let target = (t[0] as u64) | ((t[1] as u64) << 32);
                    assert_eq!(target, BASE, "Link TRB must point at the segment base");
                    self.deq = 0;
                    if t[3] & LINK_TOGGLE_CYCLE != 0 {
                        self.ccs = !self.ccs;
                    }
                    continue;
                }
                self.deq += 1;
                return Some(t);
            }
            panic!("consumer looped over the whole ring without a work TRB");
        }
    }

    fn work(n: u32) -> [u32; 4] {
        // A distinguishable payload in dword 0; TRB type 23 (No Op Command)
        // in dword 3, Cycle bit deliberately left set to prove `push` owns it.
        [n, 0, 0, (23 << 10) | TRB_CYCLE]
    }

    /// The driver's command pattern: push one TRB, ring the doorbell, wait
    /// for the controller to consume it. Every one of 100 commands — six
    /// passes over a 16-entry ring — must reach the consumer in order.
    #[test]
    fn every_command_is_consumed_across_many_wraps() {
        const LEN: usize = 16;
        let mut ring = [[0u32; 4]; LEN];
        let (mut enq, mut pcs) = (0usize, true);
        let mut hc = Consumer::new();
        for n in 0..100u32 {
            push(&mut enq, &mut pcs, LEN, BASE, work(n), |i, t| ring[i] = t);
            let got = hc.fetch(&ring).unwrap_or_else(|| panic!("command {} not fetched", n + 1));
            assert_eq!(got[0], n);
            assert_eq!(hc.fetch(&ring), None, "nothing past command {}", n + 1);
        }
    }

    /// Several TRBs queued before one doorbell (a control transfer's Setup,
    /// Data and Status stage) may straddle the Link TRB.
    #[test]
    fn a_batch_straddling_the_link_trb_is_consumed_in_order() {
        const LEN: usize = 16;
        let mut ring = [[0u32; 4]; LEN];
        let (mut enq, mut pcs) = (0usize, true);
        let mut hc = Consumer::new();
        let mut next = 0u32;
        for _ in 0..20 {
            for k in 0..3 {
                push(&mut enq, &mut pcs, LEN, BASE, work(next + k), |i, t| ring[i] = t);
            }
            for k in 0..3 {
                assert_eq!(hc.fetch(&ring).map(|t| t[0]), Some(next + k));
            }
            assert_eq!(hc.fetch(&ring), None);
            next += 3;
        }
    }

    /// The 15th TRB (index 14) is the last work slot: pushing it writes the
    /// Link TRB at index 15 with the current PCS and Toggle Cycle, and the
    /// producer continues at index 0 with PCS inverted.
    #[test]
    fn link_trb_is_written_at_the_last_slot_and_pcs_toggles() {
        const LEN: usize = 16;
        let mut ring = [[0u32; 4]; LEN];
        let (mut enq, mut pcs) = (0usize, true);
        for n in 0..14 {
            assert_eq!(push(&mut enq, &mut pcs, LEN, BASE, work(n), |i, t| ring[i] = t), n as usize);
        }
        assert_eq!(ring[LEN - 1], [0; 4], "Link slot untouched before the wrap");
        assert_eq!(push(&mut enq, &mut pcs, LEN, BASE, work(14), |i, t| ring[i] = t), 14);
        assert_eq!((enq, pcs), (0, false));
        let link = ring[LEN - 1];
        assert_eq!(link[0], BASE as u32);
        assert_eq!(link[1], (BASE >> 32) as u32);
        assert_eq!(link[2], 0);
        assert_eq!((link[3] >> 10) & 0x3f, TRB_TYPE_LINK);
        assert_ne!(link[3] & LINK_TOGGLE_CYCLE, 0);
        assert_eq!(link[3] & TRB_CYCLE, TRB_CYCLE, "Link TRB carries the old PCS (1)");
        // Second pass: TRBs carry Cycle 0, the next Link TRB too.
        assert_eq!(push(&mut enq, &mut pcs, LEN, BASE, work(15), |i, t| ring[i] = t), 0);
        assert_eq!(ring[0][3] & TRB_CYCLE, 0);
    }

    /// The defect F1 removes (`docs/api/usb/hid-mouse-protocols.md` §3.1): a
    /// producer that jumps from index 14 back to 0 with neither a Link TRB nor
    /// a PCS toggle. The consumer stops at the empty slot 15 and never sees
    /// the 16th command — the `cmd timeout` on a large device tree.
    #[test]
    fn wrapping_without_link_trb_loses_the_sixteenth_command() {
        const LEN: usize = 16;
        let mut ring = [[0u32; 4]; LEN];
        let mut enq = 0usize;
        let mut hc = Consumer::new();
        for n in 0..15u32 {
            ring[enq] = stamp_cycle(work(n), true);
            enq += 1;
            if enq >= LEN - 1 {
                enq = 0;
            }
            assert_eq!(hc.fetch(&ring).map(|t| t[0]), Some(n));
        }
        ring[enq] = stamp_cycle(work(15), true);
        assert_eq!(hc.fetch(&ring), None, "the old ring stalls on command 16");
    }

    #[test]
    fn stamp_cycle_sets_and_clears_only_bit_0() {
        assert_eq!(stamp_cycle([1, 2, 3, 0xfffe], true), [1, 2, 3, 0xffff]);
        assert_eq!(stamp_cycle([1, 2, 3, 0xffff], false), [1, 2, 3, 0xfffe]);
    }
}
