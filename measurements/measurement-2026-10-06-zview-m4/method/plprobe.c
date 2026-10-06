/*
 * plprobe.c -- diagnostic probe of measurement-2026-10-06-zview-m4 (not
 * product code): repeats zView's plugins_init() steps (plugins.c:46-123 with
 * the .nld overlay) and prints every intermediate result to the console
 * (serial). Installed on a probe card as C:\ZVIEW\ZVIEW.NAP so it starts in
 * exactly zView's context (shel_write NORMAL_APP, own directory).
 */
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <dirent.h>
#include <mint/osbind.h>
#include <gem.h>
#include <ldg.h>

static void say(const char *s) { Cconws(s); }

int main(void)
{
	char path[1024], buf[1200], ext[4];
	DIR *dir;
	struct dirent *de;
	long r;
	int n = 0;

	(void)appl_init();
	path[0] = 'A' + Dgetdrv(); path[1] = ':';
	r = Dgetpath(path + 2, 0);
	sprintf(buf, "PLPROBE: Dgetdrv/Dgetpath rc=%ld zview_path=[%s]\r\n", r, path); say(buf);
	strcat(path, "\\codecs");
	errno = 0;
	r = chdir(path);
	sprintf(buf, "PLPROBE: chdir([%s]) = %ld errno=%d\r\n", path, r, errno); say(buf);
	errno = 0;
	dir = opendir(".");
	sprintf(buf, "PLPROBE: opendir(\".\") = %p errno=%d\r\n", (void *)dir, errno); say(buf);
	if (dir) {
		while ((de = readdir(dir)) != NULL) {
			size_t l = strlen(de->d_name);
			strcpy(ext, l >= 3 ? de->d_name + l - 3 : de->d_name);
			sprintf(buf, "PLPROBE: readdir [%s] ext=[%s]\r\n", de->d_name, ext); say(buf);
			if (++n > 40) break;
		}
		sprintf(buf, "PLPROBE: readdir end errno=%d (%d entries)\r\n", errno, n); say(buf);
		closedir(dir);
	}
	{
		LDG *l = ldg_open("BMP.NLD", aes_global);
		sprintf(buf, "PLPROBE: ldg_open(BMP.NLD) = %p\r\n", (void *)l); say(buf);
		if (l) {
			void *f = ldg_find("plugin_init", l);
			sprintf(buf, "PLPROBE: plugin_init = %p infos=[%s] user_ext=%ld\r\n", f, l->infos, (long)l->user_ext); say(buf);
		}
	}
	say("PLPROBE: done\r\n");
	appl_exit();
	return 0;
}
