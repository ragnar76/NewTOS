/*
 * p9par.c -- TASK/PORT09 case suite, PARENT (#Z autorun). Checks its own
 * start-up first (a #Z program gets a copy of the AES environment
 * "PATH=\0C:\\\0", which mintlib's parseargs joins to "PATH=C:\\"; no
 * ARGV=, so argv[0] is ""), then starts C:\P9KID.NAP five times with
 * Pexec(0, ...) -- explicit environments in the shapes the launchers use
 * (TeraDesk's ARGV launch with 14 arguments, ARGV=NULL:, no ARGV, an empty
 * block) and env == NULL (inherit). The kid answers 0x42 when its
 * expectations for the case hold. Verdict line "P9PAR: PASS|FAIL".
 */
#include <stdlib.h>
#include <string.h>
#include "common.h"

extern char **environ;

/* TeraDesk's single-task launch (startprg.c:543-567, :650-655): the PORT09
   probe's 14 arguments, 196 characters joined, one with blanks. */
static const char env14[] =
	"PATH=C:\\\0"
	"ARGV=\0"
	"C:\\P9KID.NAP\0"
	"alpha-one\0bravo-two\0charlie-three\0delta-four\0echo-five\0"
	"foxtrot-six\0golf-seven\0hotel-eight\0india-nine\0juliet-ten\0"
	"kilo-eleven\0lima-twelve\0mike thirteen has blanks\0"
	"november-fourteen-is-the-last-argument\0"
	"\0";
static char tail14[128] =
	"\177alpha-one bravo-two charlie-three delta-four echo-five "
	"foxtrot-six golf-seven hotel-eight india-nine juliet-ten kilo-el";

/* ARGV-blank.txt: an empty argument travels as " " plus its index. */
static const char envnull[] =
	"ARGV=NULL:2\0C:\\P9KID.NAP\0case-null\0 \0last\0\0";
static char tailnull[128] = "\177case-null '' last";

static const char envtail[] = "FOO=bar\0P9=yes\0\0";
static char tailtail[128] = "\015case-tail x y";

static const char envempty[] = "\0\0";
static char tailempty[128] = "\012case-empty";

static char tailinh[128] = "\014case-inherit";

static void run(const char *what, char *tail, const char *env)
{
	long rc;

	Cconws("[P9PAR] case ");
	Cconws(what);
	Cconws("\r\n");
	rc = Pexec(0, "C:\\P9KID.NAP", tail, env);
	checkhex(what, (unsigned long)rc, 0x42);
}

int main(int argc, char **argv)
{
	const char *pe = getenv("PATH");

	Cconws("[P9PAR] own start-up\r\n");
	check("parent argc 1, argv[0] \"\"", argc == 1 && argv[0][0] == '\0');
	check("parent environ {PATH=C:\\} (AES PATH=\\0C:\\\\\\0 joined)",
	      environ && environ[0] && strcmp(environ[0], "PATH=C:\\") == 0
	      && environ[1] == NULL);
	check("parent getenv(PATH) == C:\\", pe && strcmp(pe, "C:\\") == 0);

	run("argv14", tail14, env14);
	run("null", tailnull, envnull);
	run("tail", tailtail, envtail);
	run("empty", tailempty, envempty);
	run("inherit", tailinh, 0);

	Cconws(fails ? "P9PAR: FAIL\r\n" : "P9PAR: PASS\r\n");
	return fails;
}
