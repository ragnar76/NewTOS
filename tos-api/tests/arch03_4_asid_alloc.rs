//! ARCH03 stage 4 — ASID bookkeeping (`bdos::process::asid`), pure logic:
//! the per-slot allocator with generations (hand-out, reuse, retirement,
//! rollover) and the queue of owed TLB invalidations with its `Xt` operand
//! layout (ARM ARM `IZVNJG`/`ILHKCP`, D8-7863/7864). Plus: with ASIDs **off**
//! (this binary never switches them on) the user leaf descriptors are
//! exactly the pre-stage-4 ones — no `nG` bit.

use tos_api::bdos::process::asid::{Allocator, Inval, Queue, NO_ASID, QUEUE_CAP};
use tos_api::bdos::process::mmu::{self, PageFlags};
use tos_api::bdos::process::MAX_PROCESSES;

#[test]
fn each_slot_gets_its_own_asid_and_keeps_it() {
    let mut a = Allocator::new();
    a.configure(1 << 16);
    let mut seen = Vec::new();
    for slot in 0..MAX_PROCESSES {
        let (asid, rolled) = a.assign(slot);
        assert!(!rolled);
        assert_ne!(asid, NO_ASID, "ASID 0 belongs to the empty root");
        assert!(!seen.contains(&asid), "distinct per slot");
        seen.push(asid);
    }
    for slot in 0..MAX_PROCESSES {
        assert_eq!(a.assign(slot), (seen[slot], false), "an installed slot keeps its ASID");
        assert_eq!(a.current(slot), Some(seen[slot]));
    }
    assert_eq!(a.assigned(), MAX_PROCESSES as u64);
}

#[test]
fn a_retired_asid_is_not_handed_out_again_in_its_generation() {
    let mut a = Allocator::new();
    a.configure(256);
    let (first, _) = a.assign(3);
    assert_eq!(a.retire(3), Some(first));
    assert_eq!(a.current(3), None, "a new tenant has no ASID until installed");
    assert_eq!(a.retire(3), None, "retiring twice is harmless");
    let mut handed = vec![first];
    // The new tenant and many more tenancies of other slots: never `first`
    // again while the generation lasts.
    for round in 0..100usize {
        let slot = round % MAX_PROCESSES;
        let (asid, rolled) = a.assign(slot);
        assert!(!rolled);
        assert!(!handed.contains(&asid), "round {round}: {asid} handed out twice");
        handed.push(asid);
        a.retire(slot);
    }
    assert_eq!(a.generation(), 1);
}

#[test]
fn running_out_starts_a_new_generation_and_stales_every_slot() {
    let mut a = Allocator::new();
    a.configure(4); // asid=tiny: ASIDs 1..3
    assert_eq!(a.assign(0), (1, false));
    assert_eq!(a.assign(1), (2, false));
    assert_eq!(a.assign(2), (3, false));
    // Fourth root: the numbers are used up -> new generation, caller must
    // drop the whole TLB before ASID 1 goes live again.
    assert_eq!(a.assign(3), (1, true));
    assert_eq!(a.generation(), 2);
    assert_eq!(a.rollovers(), 1);
    // Every slot of the old generation is stale now; its next install gets a
    // number of the new generation (and none of them is current meanwhile).
    for slot in 0..3 {
        assert_eq!(a.current(slot), None, "slot {slot} must not keep a stale ASID");
    }
    assert_eq!(a.current(3), Some(1));
    assert_eq!(a.assign(0), (2, false));
    assert_eq!(a.assign(1), (3, false));
    assert_eq!(a.assign(2), (1, true), "the next rollover");
    assert_eq!(a.rollovers(), 2);
    assert_eq!(a.current(3), None);
}

#[test]
fn many_rollovers_never_hand_out_asid_zero_or_out_of_range() {
    for limit in [2u32, 4, 256, 1 << 16] {
        let mut a = Allocator::new();
        a.configure(limit);
        let mut rollovers = 0;
        for i in 0..(3 * limit as usize + 50) {
            let slot = (i * 7) % MAX_PROCESSES;
            a.retire(slot);
            let (asid, rolled) = a.assign(slot);
            rollovers += rolled as u32;
            assert!(asid >= 1 && (asid as u32) < limit, "limit {limit}: asid {asid}");
        }
        assert_eq!(a.rollovers(), rollovers);
        assert!(rollovers >= 2, "limit {limit}: the test must cross generations");
    }
}

#[test]
fn unconfigured_and_out_of_range_give_no_asid() {
    let mut a = Allocator::new();
    assert_eq!(a.assign(0), (NO_ASID, false));
    a.configure(256);
    assert_eq!(a.assign(MAX_PROCESSES), (NO_ASID, false));
    assert_eq!(a.current(MAX_PROCESSES), None);
    assert_eq!(a.retire(MAX_PROCESSES), None);
}

#[test]
fn operand_carries_asid_and_page_number() {
    // ASID in Xt[63:48], VA[55:12] in Xt[43:0], TTL [47:44] = 0.
    let op = Inval::Asid { asid: 0xBEEF, va: 0x0123_4567_8000 }.operand();
    assert_eq!(op >> 48, 0xBEEF);
    assert_eq!(op & ((1 << 44) - 1), 0x0123_4567_8000 >> 12);
    assert_eq!((op >> 44) & 0xF, 0);
    // VAAE1: no ASID field.
    let op = Inval::AllAsids { va: 0x3AE0_1000 }.operand();
    assert_eq!(op, 0x3AE01);
    // An 8-bit ASID leaves Xt[63:56] zero (ILHKCP).
    assert_eq!(Inval::Asid { asid: 0xFF, va: 0 }.operand() >> 56, 0);
}

#[test]
fn queue_dedups_and_overflows_into_a_full_flush() {
    let mut q = Queue::new();
    assert!(q.is_empty());
    q.push(Inval::Asid { asid: 1, va: 0x40_0000 });
    q.push(Inval::Asid { asid: 1, va: 0x40_0000 });
    q.push(Inval::AllAsids { va: 0x40_0000 });
    let (ops, overflow) = q.take();
    assert!(!overflow);
    assert_eq!(ops.as_slice().len(), 2);
    assert!(q.is_empty(), "take empties the queue");

    for i in 0..=QUEUE_CAP as u64 {
        q.push(Inval::AllAsids { va: 0x40_0000 + i * 0x1000 });
    }
    assert!(!q.is_empty());
    let (_, overflow) = q.take();
    assert!(overflow, "more than the capacity -> the flush must drop everything");
    let (ops, overflow) = q.take();
    assert!(!overflow && ops.as_slice().is_empty());
}

#[test]
fn with_asids_off_the_user_leaves_are_global_as_before() {
    assert!(!mmu::asids_enabled());
    for f in [
        PageFlags::UserRwExec,
        PageFlags::UserRoExec,
        PageFlags::UserRwNx,
        PageFlags::UserRoNx,
        PageFlags::UserRwNxUncached,
        PageFlags::UserRoNxUncached,
    ] {
        assert_eq!(mmu::page_bits(f) & (1 << 11), 0, "{f:?}: nG must stay clear with ASIDs off");
    }
    // The pre-stage-4 value of the pool preset (V0a A1 bit positions):
    // VALID | PAGE | AttrIdx 0 | AP=01 | SH=11 | AF | PXN.
    assert_eq!(
        mmu::page_bits(PageFlags::UserRwExec),
        1 | (1 << 1) | (0b01 << 6) | (0b11 << 8) | (1 << 10) | (1 << 53)
    );
}
