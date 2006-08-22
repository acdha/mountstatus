// Specify that we want to use POSIX to work around

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

/*
	TODO:
		- Command-line arguments to replace hard-coded values
*/

pid_t child;

int main (int argc, char const* argv[]) {
	if (getuid() > 0) {
		fprintf(stderr, "%s must be run as root\n", argv[0]);
		exit(EXIT_FAILURE);
	}
	
	if (argc > 1) {
		fprintf(stderr, "%s does not accept command-line arguments\n", argv[0]);
	}
	
	// Close all file handles:
	fflush(NULL);

	for (int fd = _POSIX_OPEN_MAX; fd >= 0; fd--) {
		close(fd);
	}

	if (fork() > 0)	exit(EXIT_SUCCESS);
	if (fork() > 0)	exit(EXIT_SUCCESS);

	setsid();

	openlog(basename((char*)argv[0]), LOG_PID | LOG_NDELAY | LOG_NOWAIT, LOG_DAEMON);

	while (getppid() != 1) {
		sleep(1);
	}

	if (chdir("/") != 0) {
		syslog(LOG_ERR, "Couldn't chdir(/): %m");
		exit(EXIT_FAILURE);
	}

	syslog(LOG_INFO, "%s started", argv[0]);

	struct sigaction sact;
	memset(&sact, 0, sizeof sact);
	sact.sa_handler = (void*) &kill_children;
	sigemptyset(&sact.sa_mask);	

	if (sigaction(SIGALRM, &sact, 0) < 0) {
		syslog(LOG_ERR, "Cannot install SIGALRM handler: %m");
		exit(EXIT_FAILURE);
	}

	int zombiestatus;

	while (1==1) {
		check_mounts();
		// We'll reap any zombies just in case:
      	while (waitpid(-1, &zombiestatus, WNOHANG) > 0);
		sleep(180);
	}
	
	closelog();
	return(0);
}

void check_mounts() {
	time_t startTime = time(NULL);
	
	int livemountcount = 0;
	int mountcount = 0;

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
		syslog(LOG_CRIT, "Error %d retrieving filesystem information: %m", errno);
	}

	for (int i = 0; i < mountcount; i++) {
		if (check_mount(mounts[i].f_mntonname)) {
			livemountcount++;
		}
	}
#endif

	if (mountcount != livemountcount) {
		syslog(LOG_INFO, "Checked %u mounts in %i seconds: %u dead", livemountcount, (int)(time(NULL) - startTime), mountcount - livemountcount);
	} else {
		syslog(LOG_INFO, "Checked %u mounts in %i seconds", livemountcount, (int)(time(NULL) - startTime));
	}
}
	

bool check_mount(const char* path) {
	child = fork();
	if (child < 0) {
		syslog(LOG_ERR, "Error %d attempting to fork mount checking child for %s: %m", errno, path);
	} else if (child == 0) {						
		struct stat mountstat;

		if (stat(path, &mountstat) != 0) {
			syslog(LOG_ERR, "Couldn't stat %s: %m", path);
			exit(EXIT_FAILURE);
		}

		// Change to the UID of the mount owner to handle mountpoints with restrictive permissions:
		if (setuid(mountstat.st_uid) != 0) {
			syslog(LOG_ERR, "Couldn't setuid(%d): %m", mountstat.st_uid);
			exit(EXIT_FAILURE);
		}
		if (setgid(mountstat.st_gid) != 0) {
			syslog(LOG_ERR, "Couldn't setgid(%d): %m", mountstat.st_gid);
			exit(EXIT_FAILURE);
		}
			
		DIR* mountpoint = opendir(path);

		if (!mountpoint) {
			syslog(LOG_ERR, "Couldn't open directory %s: %m", path);
			exit(EXIT_FAILURE);
		}
			
		int direntc = 0;
		struct dirent *dp;
		while ((dp = readdir(mountpoint)) != NULL) {
			direntc++;
		}
			
		if (closedir(mountpoint) != 0) {
			syslog(LOG_ERR, "Unable to close directory %s: %m", path);
			exit(EXIT_FAILURE);
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
				syslog(LOG_ERR, "Child process %i returned %i while checking %s!", child, WEXITSTATUS(status), path);
			} 
		} else if (WIFSIGNALED(status)) {
			syslog(LOG_ERR, "Child process %i terminated on signal %i while checking %s!", child, WTERMSIG(status), path);
		} else {
			syslog(LOG_ERR, "Child process %i terminated with status %i while checking %s!", child, WEXITSTATUS(status), path);
		}
	}

	return false;
}

void kill_children() {
	if (child > 0) {
		syslog(LOG_INFO, "Timed out waiting for child process %i: sending SIGKILL", child);
		int rc = kill(child, SIGKILL);
		if (rc != 0) {
			syslog(LOG_ERR, "Unable to kill child process %i: %m", child);
		}
	} else {
		syslog(LOG_INFO, "received an unexplained SIGALARM!");
	}
}
