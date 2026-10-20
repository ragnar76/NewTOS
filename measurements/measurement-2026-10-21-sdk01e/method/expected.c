/* expected.c -- the expected OpenLibm bit patterns of rtp.c: built for the HOST against OpenLibm 0.8.8 compiled for the host from the same sources (sdk/compiler-rt/tests/rt-host.sh builds that archive; here: cc -O2 -I<openlibm>/include -I<openlibm> -I<openlibm>/src expected.c <host-openlibm.a>). Output: expected.txt. */
#include <openlibm_complex.h>
#include <openlibm_math.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
static uint64_t D(double d){uint64_t u;memcpy(&u,&d,8);return u;}
static uint32_t F(float f){uint32_t u;memcpy(&u,&f,4);return u;}
#define PC(n,z) printf("%-10s 0x%016llxULL, 0x%016llxULL\n", n, (unsigned long long)D(creal(z)), (unsigned long long)D(cimag(z)))
#define PCF(n,z) printf("%-10s 0x%08xU, 0x%08xU\n", n, F(crealf(z)), F(cimagf(z)))
int main(void){
 double complex z = CMPLX(0.5, 0.5); float complex zf = CMPLXF(0.5f, 0.5f);
 PC("casin", casin(z)); PC("cacos", cacos(z)); PC("casinh", casinh(z)); PC("cacosh", cacosh(z)); PC("catanh", catanh(z));
 PCF("casinf", casinf(zf)); PCF("cacosf", cacosf(zf)); PCF("casinhf", casinhf(zf)); PCF("cacoshf", cacoshf(zf)); PCF("catanhf", catanhf(zf));
 PCF("cpowf", cpowf(CMPLXF(1.0f,1.0f), CMPLXF(2.0f,0.0f)));
 PCF("cpowf2", cpowf(CMPLXF(0.5f,-1.5f), CMPLXF(-0.5f,1.0f)));
 printf("powf(10,3.5) 0x%08x  powf(2,.5) 0x%08x  powf(-2,3) 0x%08x  powf(0.3,-7.25) 0x%08x\n", F(powf(10.0f,3.5f)), F(powf(2.0f,0.5f)), F(powf(-2.0f,3.0f)), F(powf(0.3f,-7.25f)));
 printf("remainder(1e300, 3.7) 0x%016llx remainderf(1e30f,3.7f) 0x%08x\n", (unsigned long long)D(remainder(1e300,3.7)), F(remainderf(1e30f,3.7f)));
}
