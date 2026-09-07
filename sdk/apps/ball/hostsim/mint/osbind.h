/* host shim for the TOS calls ball.c / ball-tex.c make */
#ifndef HOST_OSBIND_H
#define HOST_OSBIND_H
#include <stddef.h>
long host_hz200(void);
void host_cconws(const char *s);
int  host_bconstat(int d);
long host_bconin(int d);
long host_fopen(const char *p, int m);
long host_fcreate(const char *p, int a);
long host_fclose(int fd);
long host_fread(int fd, long n, void *b);
long host_fwrite(int fd, long n, const void *b);
long host_fseek(long off, int fd, int mode);
void *host_malloc(long n);
void host_free(void *p);
void host_vsync(void);

#define Cconws(s)          host_cconws(s)
#define Bconstat(d)        host_bconstat(d)
#define Bconin(d)          host_bconin(d)
#define Bconout(d,c)       ((void)(d),(void)(c),0L)
#define Fopen(p,m)         host_fopen((p),(m))
#define Fcreate(p,a)       host_fcreate((p),(a))
#define Fclose(f)          host_fclose((int)(f))
#define Fread(f,n,b)       host_fread((int)(f),(long)(n),(b))
#define Fwrite(f,n,b)      host_fwrite((int)(f),(long)(n),(b))
#define Fseek(o,f,m)       host_fseek((long)(o),(int)(f),(int)(m))
#define Malloc(n)          ((long)(size_t)host_malloc((long)(n)))
#define Mxalloc(n,m)       ((long)(size_t)host_malloc((long)(n)))
#define Mfree(p)           (host_free((void *)(p)),0L)
#define Vsync()            host_vsync()
#endif
