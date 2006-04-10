#include <syslog.h>
#include <stdarg.h>
#include <sys/param.h>
#include <sys/mount.h>
#include <stdio.h>
#include <sys/types.h>
#include <dirent.h>
#include <sys/types.h>
#include <unistd.h>
#include <stdlib.h>
#include <sys/time.h>
#include <sys/resource.h>
#include <errno.h>
#include <signal.h>
#include "main.h"

extern int errno;

/*
	TODO:
		- Switch to using SIGARLM instead of polling
		- Command-line arguments to replace hard-coded values
*/

pid_t child;

int main (int argc, char const* argv[]) {

	if (fork() > 0)	exit(EXIT_SUCCESS);
	if (fork() > 0)	exit(EXIT_SUCCESS);
	
	openlog(argv[0], LOG_PERROR | LOG_PID, LOG_DAEMON);

	struct sigaction sact;
	struct sigaction * osact = NULL;
	sact.sa_handler = &kill_children;
	sact.sa_flags = SA_RESTART;
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
	
	struct statfs *mounts;
	int livemountcount = 0;
	int mountcount = getmntinfo(&mounts, MNT_NOWAIT);
	
	time_t childStartTime;
	
	if (mountcount < 0) {
		syslog(LOG_CRIT, "Error %d retrieving filesystem information: %m", errno);
	}
	
	for (int i = 0; i < mountcount; i++) {
		child = fork();
		if (child < 0) {
			syslog(LOG_ERR, "Error %d attempting to fork mount checking child for %s: %m", mounts[i].f_mntonname);
		} else if (child == 0) {						
			syslog(LOG_DEBUG, "Checking %s (%s)", mounts[i].f_mntonname, mounts[i].f_mntfromname);
			
			DIR* mountpoint = opendir(mounts[i].f_mntonname);

			if (!mountpoint) {
				syslog(LOG_ERR, "Couldn't open directory %s: %m", mounts[i].f_mntonname);
				exit(EXIT_FAILURE);
			}
			
			int direntc = 0;
			struct dirent *dp;
			while ((dp = readdir(mountpoint)) != NULL) {
				direntc++;
			}
			
			if (closedir(mountpoint) != 0) {
				syslog(LOG_ERR, "Unable to close directory %s: %m", mounts[i].f_mntonname);
				exit(EXIT_FAILURE);
			}
			
			syslog(LOG_DEBUG, "%s contains %i files", mounts[i].f_mntonname, direntc);
			exit(EXIT_SUCCESS);
		} else {
			childStartTime = time(NULL);
			int status = 0;
			
			alarm(15);
			waitpid(child, &status, 0);
			alarm(0);	
			
			if (WIFEXITED(status)) {
				if (WEXITSTATUS(status) == EXIT_SUCCESS) {
					livemountcount++;
				} else {
					syslog(LOG_ERR, "Child process %i returned %i while checking %s!", child, WEXITSTATUS(status), mounts[i].f_mntonname);
				} 
			} else if (WIFSIGNALED(status)) {
				syslog(LOG_ERR, "Child process %i terminated on signal %i while checking %s!", child, WTERMSIG(status), mounts[i].f_mntonname);
			} else {
				syslog(LOG_ERR, "Child process %i terminated with status %i while checking %s!", child, WEXITSTATUS(status), mounts[i].f_mntonname);
			}
		}
	}
	
	syslog(LOG_INFO, "Checked %u mounts in %u seconds: %u live", livemountcount, (time(NULL) - startTime), mountcount);
}

void kill_children(int sig) {
	if (child > 0) {
		syslog(LOG_INFO, "Timed out waiting for child process %i: sending SIGKILL", child);
		kill(child, SIGKILL);
	} else {
		syslog(LOG_INFO, "received an unexplained SIGALARM!");
	}
}
