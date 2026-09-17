/* H03 scratch: mt_evnt_timer with an endian/width-independent word split
 * (the same form a_evnt_multi.c already uses: interval, interval >> 16). */
#include "gem_aesP.h"
short
mt_evnt_timer (unsigned long interval, short *global_aes)
{
	AES_PARAMS(24,2,1,0,0);
	aes_intin[0] = (short)(interval & 0xFFFF);
	aes_intin[1] = (short)((interval >> 16) & 0xFFFF);
	AES_TRAP(aes_params);
	return (aes_intout[0]);
}
