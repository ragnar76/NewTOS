/*
 * m68k reference model of QED's set module.
 *
 * Every function body below is a LITERAL transliteration of
 * sdk/apps/qed/src/set.c (sha256 264acfcd...), with the only change being
 * that "unsigned long" word access is replaced by an explicit BIG-ENDIAN
 * 32-bit read/write over a 128-byte image (m_word / m_putword) and
 * "*set++" by "m_word(s, w++)".  Nothing else is re-derived: this is what
 * the pristine source DOES on a 32-bit big-endian machine.
 */
#include <string.h>
#include "m68kmodel.h"

static const unsigned char bits[] = {128, 64, 32, 16, 8, 4, 2, 1};

static uint32_t m_word(const unsigned char *s, int i)
{
	return ((uint32_t)s[4*i] << 24) | ((uint32_t)s[4*i+1] << 16)
	     | ((uint32_t)s[4*i+2] << 8) | (uint32_t)s[4*i+3];
}

static void m_putword(unsigned char *s, int i, uint32_t v)
{
	s[4*i]   = (unsigned char)(v >> 24);
	s[4*i+1] = (unsigned char)(v >> 16);
	s[4*i+2] = (unsigned char)(v >> 8);
	s[4*i+3] = (unsigned char)v;
}

short m_setfree(const unsigned char *s)
{
	short	i;
	uint32_t help, h;
	int	w = 0;

	h = 0xFFFFFFFFUL;
	for (i = 0; i < M_SETMAX; i += 32)
		if ((help = m_word(s, w++)) != h)
		{
			h = 0x80000000UL;
			while (help >= h)
			{
				i++;
				help <<= 1;
			}
			break;
		}
	return i;
}

short m_setmax(const unsigned char *s)
{
	short	i;
	uint32_t help;

	for (i = M_SETSIZE; (--i) >= 0; )
		if ((help = m_word(s, i)) != 0UL)
		{
			i *= 32;
			for (i += 31; (help & 1UL) == 0; i--, help >>= 1) ;
			return (i);
		}
	return -1;
}

short m_setmin(const unsigned char *s)
{
	short	i;
	uint32_t help, h;
	int	w = 0;

	for (i = 0; i < M_SETMAX; i += 32)
		if ((help = m_word(s, w++)) != 0UL)
		{
			h = 0x80000000UL;
			while (help < h)
			{
				i++; help <<= 1;
			}
			break;
		}
	return i;
}

void m_setcpy(unsigned char *s1, const unsigned char *s2)
{
	memcpy(s1, s2, M_SETSIZE * 4);
}

void m_setall(unsigned char *s)
{
	int i;
	for (i = 0; i < M_SETSIZE; i++)
		m_putword(s, i, 0xFFFFFFFFUL);
}

void m_setclr(unsigned char *s)
{
	int i;
	for (i = 0; i < M_SETSIZE; i++)
		m_putword(s, i, 0UL);
}

void m_setnot(unsigned char *s)
{
	int i;
	for (i = 0; i < M_SETSIZE; i++)
		m_putword(s, i, m_word(s, i) ^ 0xFFFFFFFFUL);
}

void m_setand(unsigned char *s1, const unsigned char *s2)
{
	int i;
	for (i = 0; i < M_SETSIZE; i++)
		m_putword(s1, i, m_word(s1, i) & m_word(s2, i));
}

void m_setor(unsigned char *s1, const unsigned char *s2)
{
	int i;
	for (i = 0; i < M_SETSIZE; i++)
		m_putword(s1, i, m_word(s1, i) | m_word(s2, i));
}

void m_setxor(unsigned char *s1, const unsigned char *s2)
{
	int i;
	for (i = 0; i < M_SETSIZE; i++)
		m_putword(s1, i, m_word(s1, i) ^ m_word(s2, i));
}

void m_setincl(unsigned char *s, short elt)
{
	if (elt >= 0 && elt <= M_SETMAX)
		*((char*)s + (elt >> 3)) |= bits[elt & 7];
}

void m_setexcl(unsigned char *s, short elt)
{
	if (elt >= 0 && elt <= M_SETMAX)
		*((char*)s + (elt >> 3)) &= ~bits[elt & 7];
}

void m_setchg(unsigned char *s, short elt)
{
	if (elt >= 0 && elt <= M_SETMAX)
		*((char*)s + (elt >> 3)) ^= bits[elt & 7];
}

bool m_setin(const unsigned char *s, short elt)
{
	if (elt >= 0 && elt <= M_SETMAX)
		return ((*((const char *)s + (elt >> 3)) & bits[elt & 7]) ? true : false);
	else
		return (false);
}

bool m_setcmp(const unsigned char *s1, const unsigned char *s2)
{
	short i;

	if (s2 == NULL)
		for (i = 0; i < M_SETSIZE && m_word(s1, i) == 0; i++) ;
	else
		for (i = 0; i < M_SETSIZE && m_word(s1, i) == m_word(s2, i); i++) ;
	return (i == M_SETSIZE);
}

short m_setcard(const unsigned char *s)
{
	short i, card, max;

	max = m_setmax(s);
	for (i = m_setmin(s), card = 0; i <= max; i++)
		if (m_setin(s, i)) card++;

	return (card);
}

void m_str2set(const char *str, unsigned char *s)
{
	short	i;

	m_setclr(s);
	i = 0;
	while (str[i])
	{
		if (str[i] == '-' && i > 0)
		{
			unsigned char c;			/* QED builds with -funsigned-char */

			i++;
			for (c = (unsigned char)str[i - 2]; c < (unsigned char)str[i]; c++)
				m_setincl(s, c);
		}
		else
			m_setincl(s, (unsigned char)str[i++]);
	}
}
