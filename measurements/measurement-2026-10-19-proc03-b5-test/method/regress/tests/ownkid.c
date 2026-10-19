/*
 * ownkid.c -- ARCH03 stage 2d probe child. Command tail: 8 hex digits = a
 * block of its parent. Started with Pexec 0 (its own process): it must not
 * free or shrink that block (E15 -> EIMBA). Started with Pexec 3+4 by its
 * parent (tail "K" style: 8 zero digits) it is a co-owner of its basepage
 * and environment and shares its parent's memory space: Mshrink(own
 * basepage) succeeds, Mfree(own env) drops its link, a second one the
 * parent's -- the block is freed.
 * Exit code: 0x40 + number of failed checks.
 */
#include "common.h"

extern unsigned long _PgmSize;

int main(void)
{
	unsigned long blk = tail_value(_base->p_cmdlin);
	if (blk) {
		checkhex("kid: Mfree(parent's block) = EIMBA", (unsigned long)Mfree((void *)blk),
			 (unsigned long)-40L);
		checkhex("kid: Mshrink(parent's block) = EIMBA", (unsigned long)Mshrink((void *)blk, 16),
			 (unsigned long)-40L);
	} else {
		/* Its own size: the owner check is all that is tested. Since
		   ARCH03 E9 the start-up code has already shrunk the TPA to
		   _PgmSize (mintlib _crtinit), so that is "its own size" now --
		   p_hitpa - p_lowtpa would be a grow (EGSBF), on EmuTOS too. */
		checkhex("kid (Pexec 4): Mshrink(own basepage) as co-owner",
			 (unsigned long)Mshrink(_base, _PgmSize), 0);
		/* Its own link first; a Pexec-4 child shares its caller's
		   memory space (plan E6, FreeMiNT FORK_SHAREVM), so a second
		   Mfree drops the caller's link too -- and frees the block
		   (FreeMiNT detaches both attachments the same way). */
		checkhex("kid (Pexec 4): Mfree(own env) drops its own link",
			 (unsigned long)Mfree((void *)(unsigned long)_base->p_env), 0);
		checkhex("kid (Pexec 4): a second Mfree(env) drops the caller's (shared space)",
			 (unsigned long)Mfree((void *)(unsigned long)_base->p_env), 0);
		checkhex("kid (Pexec 4): a third Mfree(env) = EIMBA (freed)",
			 (unsigned long)Mfree((void *)(unsigned long)_base->p_env), (unsigned long)-40L);
	}
	return 0x40 + fails;
}
