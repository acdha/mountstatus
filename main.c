// Specify that we want to use POSIX to work around an oddity in the Linux mntent code:
#ifdef __linux__
#define __USE_POSIX
#define _POSIX_C_SOURCE 200112L
#include <sys/vfs.h>
#include <mntent.h>
#else
#include <sys/param.h>
#include <sys/ucred.h>
#include <sys/mount.h>
#endif

#include <sys/resource.h>
#include <sys/types.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <sys/stat.h>

#include <dirent.h>
#include <errno.h>
#include <signal.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <syslog.h>
#include <time.h>
#include <unistd.h>
#include <fcntl.h>
#include <limits.h>
#include <libgen.h>
#include <string.h>

#include "main.h"

extern int errno;

pid_t child;

#define _GNU_SOURCE
#include <getopt.h>

void output(int err, char * msg) 
{
	syslog(err, msg);
	if (err != LOG_INFO)
		fprintf(stdout, "%s\n", msg);
}
void output_and_exit(int err, char *msg, int exitcode)
{
	output(err, msg);
	exit(exitcode);
}

int main (int argc, char const* argv[]) {

	static int nodaemon = 0;
	static int print = 0;
	int c;
	char msg[255];
	
	if (getuid() > 0) {
		fprintf(stderr, "%s must be run as root\n", argv[0]);
		exit(EXIT_FAILURE);
	}

	while (1)
	{
		static struct option long_options[] =
		{
			{"print", 	no_argument, &print, 	1},
			{"nodaemon",	no_argument, &nodaemon, 1},
			{0, 0, 0, 0}
		};
		/* getopt_long stores the option index here. */
		int option_index = 0;
	
		c = getopt_long (argc, argv, "pn",
			long_options, &option_index);

		/* Detect the end of the options. */
		if (c == -1)
			break;

		switch (c)
		{
			case 'p':
				print = 1;
				break;
			case 'n':
				nodaemon = 1;
				break;
		}

	}

	if (optind < argc)
	{
		output(1,  "Invalid command line arguments\n");

		fprintf(stderr, "Invalid command line arguments\n");
		exit(1);

		// Close all file handles:
		fflush(NULL);
	}


	if (nodaemon == 0)
	{
		for (int fd = _POSIX_OPEN_MAX; fd >= 0; fd--) {
			close(fd);
		}

		if (fork() > 0) exit(EXIT_SUCCESS);
		if (fork() > 0) exit(EXIT_SUCCESS);
	}

	setsid();

	openlog(basename((char*)argv[0]), LOG_PID | LOG_NDELAY | LOG_NOWAIT, LOG_DAEMON);

	if (nodaemon == 0)
	{
		while (getppid() != 1) {
			sleep(1);
		}
	}

	if (chdir("/") != 0) {
		//syslog(LOG_ERR, "Couldn't chdir(/): errno %d: %m", errno);
		sprintf(msg, "Couldn't chdir(/): errno %d: %m", errno);
		output_and_exit(LOG_ERR, msg, EXIT_FAILURE);
		//exit(EXIT_FAILURE);
	}

	struct sigaction sact;
	memset(&sact, 0, sizeof sact);
	sact.sa_handler = (void*) &kill_children;
	sigemptyset(&sact.sa_mask);

	if (sigaction(SIGALRM, &sact, 0) < 0) {
		sprintf(msg, "Cannot install SIGALRM handler: errno %d: %m", errno);
		output_and_exit(LOG_ERR, msg, EXIT_FAILURE);
	}

	int zombiestatus;

	sprintf(msg, "%s started", argv[0]);
	output(LOG_INFO, msg);

	while (1==1) {
		check_mounts();
		// We'll reap any zombies just in case:
		while (waitpid(-1, &zombiestatus, WNOHANG) > 0);
		if (nodaemon == 1)
			break;
		sleep(180);
	}

	closelog();
	return(0);
}


void check_mounts() {
	int rc = 0;
	time_t startTime = time(NULL);

	int livemountcount = 0;
	int mountcount = 0;

	char msg[255];
#ifdef __linux__
    FILE *fp;
    struct mntent *entry;

    fp = setmntent( _PATH_MOUNTED, "r" );

    while ((entry = getmntent(fp)) != NULL ) {
        if (check_mount(entry->mnt_dir)) {
            livemountcount++;
        }
        mountcount++;
    }

    endmntent( fp );
#else
	struct statfs *mounts;
	mountcount = getmntinfo(&mounts, MNT_NOWAIT);

	if (mountcount < 0) {
		sprintf(msg, "Couldn't retrieve filesystem information: errno %d: %m", errno);
		output(LOG_CRIT, msg);
	}

	for (int i = 0; i < mountcount; i++) {
		if (check_mount(mounts[i].f_mntonname)) {
			livemountcount++;
		}
	}
#endif

	if (mountcount != livemountcount) {
		rc = LOG_ERR;
		sprintf(msg, "Checked %u mounts in %i seconds: %u dead", livemountcount, (int)(time(NULL) - startTime), mountcount - livemountcount);
	} else {
		rc = LOG_INFO;
		sprintf(msg, "Checked %u mounts in %i seconds", livemountcount, (int)(time(NULL) - startTime));
	}
	output(rc, msg);
}


bool check_mount(const char* path) {
	char msg[255];
	child = fork();
	if (child < 0) {
        	sprintf(msg, "Couldn't fork a child to check mountpoint %s: errno %d: %m", path, errno);
		output(LOG_ERR, msg);
	} else if (child == 0) {
		struct stat mountstat;

		if (stat(path, &mountstat) != 0) {
			sprintf(msg, "Couldn't stat mountpoint %s: errno %d: %m", path, errno);
			output_and_exit(LOG_ERR, msg,42);
		}

		if ((mountstat.st_mode & 0x111) == 0) {
			sprintf(msg, "Couldn't check mountpoint %s: mode %3x does not allow access", path, mountstat.st_mode);
			output_and_exit(LOG_ERR, msg,42);
		}

		// Change to the UID of the mount owner to handle mountpoints with restrictive permissions:
		if (setgid(mountstat.st_gid) != 0) {
			sprintf(msg, "Couldn't setgid(%d): errno %d: %m", mountstat.st_gid, errno);
			output_and_exit(LOG_ERR, msg, EXIT_FAILURE);
		}
		if (setuid(mountstat.st_uid) != 0) {
			sprintf(msg, "Couldn't setuid(%d): errno %d: %m", mountstat.st_uid, errno);
			output_and_exit(LOG_ERR, msg, EXIT_FAILURE);
		}

		DIR* mountpoint = opendir(path);

		if (!mountpoint) {
			sprintf(msg, "Couldn't open directory %s: errno %d: %m", path, errno);

			if (errno == EACCES) {
				output_and_exit(LOG_INFO, msg,42);
			} else {
				output_and_exit(LOG_ERR, msg, EXIT_FAILURE);
			}
		}

		int direntc = 0;
		struct dirent *dp;
		while ((dp = readdir(mountpoint)) != NULL) {
			direntc++;
		}

		if (closedir(mountpoint) != 0) {
			sprintf(msg, "Couldn't close directory %s: errno %d: %m", path, errno);
			output_and_exit(LOG_ERR, msg, EXIT_FAILURE);
		}

		exit(42);
	} else {
		int status = 0;

		alarm(60);
		waitpid(child, &status, 0);
		alarm(0);

        if (WIFEXITED(status)) {
            if (WEXITSTATUS(status) == 42) {
                return true;
            } else {
                sprintf(msg, "Child process %i returned %i while checking %s!", child, WEXITSTATUS(status), path);
		output(LOG_ERR, msg);
            }
        } else if (WIFSIGNALED(status)) {
            sprintf(msg, "Child process %i terminated on signal %i while checking %s!", child, WTERMSIG(status), path);
		output(LOG_ERR, msg);
        } else {
            sprintf(msg, "Child process %i terminated with status %i while checking %s!", child, WEXITSTATUS(status), path);
		output(LOG_ERR, msg);
        }
	}

	return false;
}

void kill_children() {
	char msg[255];
	if (child > 0) {
		sprintf(msg, "Timed out waiting for child process %i: sending SIGKILL", child);
		output(LOG_ERR, msg);
		int rc = kill(child, SIGKILL);
		if (rc != 0) {
			sprintf(msg, "Couldn't kill child process %i: errno %d: %m", child, errno);
			output(LOG_ERR, msg);
		}
	} else {
		sprintf(msg, "Received an unexpected SIGALARM!");
		output(LOG_ERR, msg);
	}
}
