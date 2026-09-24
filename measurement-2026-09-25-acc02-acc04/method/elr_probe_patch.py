#!/usr/bin/env python3
"""ACC diagnosis (scratch copy only): print the EL0 return address of every
GEM trap, so an AES call can be attributed to a *code* location and not only to
a process identity. The op-19 flood in the first acc2 run was traced to
identity 0 (an accessory) calling `appl_exit`, which its source does not do --
the open question is which code is executing there."""
import sys
p = sys.argv[1] + '/kernel/src/arch/aarch64/exceptions.rs'; s = open(p).read()
a = "        TrapClass::Gem => {\n            let abi = tos_api::bdos::process::current_abi();"
assert s.count(a) == 1
s = s.replace(a, "        TrapClass::Gem => {\n"
                 "            if crate::host_env::h02_inject::ELR_PROBE.load(core::sync::atomic::Ordering::Relaxed)\n"
                 "                && (frame.spsr & 0xF) == 0 {\n"
                 "                crate::kprintln!(\"[elr] gem trap elr=0x{:x} sp=0x{:x} x0={} x1=0x{:x}\",\n"
                 "                    frame.elr, frame.sp_el0, frame.gpr[0], frame.gpr[1]);\n"
                 "            }\n"
                 "            let abi = tos_api::bdos::process::current_abi();", 1)
open(p, 'w').write(s)
p = sys.argv[1] + '/kernel/src/host_env.rs'; s = open(p).read()
a = "    static ARMED_BY_EL0: AtomicU32 = AtomicU32::new(0); // A78-TEMP: 1 = EL0 program, 2 = desktop\n"
assert s.count(a) == 1
s = s.replace(a, a + "    pub static ELR_PROBE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false); // ACC-TEMP\n", 1)
a = "                Step::Arm => { tos_api::aes::H02_TRACE.store(true, Ordering::Relaxed); }"
assert s.count(a) == 1
s = s.replace(a, "                Step::Arm => { tos_api::aes::H02_TRACE.store(true, Ordering::Relaxed); ELR_PROBE.store(true, Ordering::Relaxed); }", 1)
open(p, 'w').write(s)
print("patched", p)
