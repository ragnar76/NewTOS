#include <stdio.h>
#include <mint/osbind.h>
static void t(const char *what, double v)
{
	Cconws("fptest: "); Cconws(what); Cconws(" -> ");
	printf("[%f] [%.3f] [%e] [%g]\r\n", v, v, v, v);
	fflush(stdout);
}
int main(void)
{
	Cconws("fptest: start\r\n");
	t("1.5", 1.5);
	t("0.1", 0.1);
	t("3.14159", 3.14159);
	t("79.123", 79.123);
	t("1e10", 1e10);
	t("1e-5", 1e-5);
	t("123456789.0", 123456789.0);
	t("1e100", 1e100);
	Cconws("fptest: end\r\n");
	return 0;
}
