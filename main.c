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

#include "main.h"

extern int errno;

/*
	TODO:
		- Command-line arguments to replace hard-coded values
*/

pid_t child;

int main (int argc, char const* argv[]) {
	openlog(argv[0], LOG_PID | LOG_CONS, LOG_DAEMON);

	if (fork() > 0)	exit(EXIT_SUCCESS);
	if (fork() > 0)	exit(EXIT_SUCCESS);
	
	syslog(LOG_INFO, "%s started", argv[0]);

	struct sigaction sact;
	struct sigaction * osact = NULL;
	sact.sa_handler = &kill_children;
	// Probably unnecessary: sact.sa_flags = SA_RESTART;
	sigemptyset(&sact.sa_mask);	

	if (sigaction(SIGALRM, &sact, osact) < 0) {
		syslog(LOG_ERR, "Cannot install SIGALRM handler: %m");
		exit(EXIT_FAILURE);
	}

	while (1==1) {
		check_mounts();
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
		if (check_mount(entry->mnt_dir, entry->mnt_fsname)) {
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
		if (check_mount(mounts[i].f_mntonname, mounts[i].f_mntfromname)) {
			livemountcount++;
		}
	}
#endif

	syslog(LOG_INFO, "Checked %u mounts in %i seconds: %u live", livemountcount, (int)(time(NULL) - startTime), mountcount);
}
	

bool check_mount(const char* path, const char* source) {
	child = fork();
	if (child < 0) {
		syslog(LOG_ERR, "Error %d attempting to fork mount checking child for %s: %m", errno, path);
	} else if (child == 0) {						
		syslog(LOG_DEBUG, "Checking %s (%s)", path, source);
			
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
			
		syslog(LOG_DEBUG, "%s contains %i files", path, direntc);
		exit(EXIT_SUCCESS);
	} else {
		int status = 0;
			
		alarm(15);
		waitpid(child, &status, 0);
		alarm(0);	

		if (WIFEXITED(status)) {
			if (WEXITSTATUS(status) == EXIT_SUCCESS) {
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

void kill_children(int sig) {
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
