#ifndef M68KMODEL_H
#define M68KMODEL_H
#include <stdint.h>
#include <stdbool.h>

/* The m68k SET: 32 words of 32 bit = 128 bytes, read big-endian. */
#define M_SETSIZE 32
#define M_SETMAX  (M_SETSIZE * 32 - 1)
#define M_BYTES   (M_SETSIZE * 4)

typedef unsigned char MSET[M_BYTES];

short m_setfree(const unsigned char *s);
short m_setmax (const unsigned char *s);
short m_setmin (const unsigned char *s);
void  m_setcpy (unsigned char *s1, const unsigned char *s2);
void  m_setall (unsigned char *s);
void  m_setclr (unsigned char *s);
void  m_setnot (unsigned char *s);
void  m_setand (unsigned char *s1, const unsigned char *s2);
void  m_setor  (unsigned char *s1, const unsigned char *s2);
void  m_setxor (unsigned char *s1, const unsigned char *s2);
void  m_setincl(unsigned char *s, short elt);
void  m_setexcl(unsigned char *s, short elt);
void  m_setchg (unsigned char *s, short elt);
bool  m_setin  (const unsigned char *s, short elt);
bool  m_setcmp (const unsigned char *s1, const unsigned char *s2);
short m_setcard(const unsigned char *s);
void  m_str2set(const char *str, unsigned char *s);
#endif
