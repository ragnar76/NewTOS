#include <stdio.h>
#include <stdlib.h>
#include <mint/osbind.h>
int main(void)
{
	char *end;
	double d = strtod("3.14159", &end);
	Cconws("itest: start\r\n");
	printf("itest: %%ld 1234567890123 -> [%ld]\r\n", 1234567890123L);
	printf("itest: %%lld -9876543210987 -> [%lld]\r\n", -9876543210987LL);
	printf("itest: %%llu max -> [%llu]\r\n", 18446744073709551615ULL);
	printf("itest: %%lx 0x123456789abcdef -> [%lx]\r\n", 0x123456789abcdefUL);
	printf("itest: %%lo 01234567012345670 -> [%lo]\r\n", 01234567012345670UL);
	printf("itest: strtod(3.14159)*1e5 as long -> [%ld]\r\n", (long)(d * 100000.0 + 0.5));
	fflush(stdout);
	Cconws("itest: end\r\n");
	return 0;
}
