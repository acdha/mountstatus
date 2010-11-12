Background
==========

Our environment is designed around a NAS model with a few very large servers
providing terabytes of storage to several hundred machines using NFS and
cfengine to synchronize our filesystem configuration. This greatly simplifies
our infrastructure by giving our users a single, consistent filesystem view no
matter which machine they're using, avoids the need for us to manage large
quantities of local storage and has allowed us to treat servers and
workstations as interchangeable parts (automated installs mean a failed system
can be replaced in as little as 5 minutes).

As you'd expect, this doesn't always work so well as NFS was designed to
handle failures smoothly: something goes wrong client simply hangs until the
server starts responding again. Unfortunately, most desktop operating systems
assume that networks and servers are perfectly reliable, and handle problems
particularly poorly by freezing because key parts of the UI use config files
in a user's home directory and so the GUI freezes along with everything else.

OS X, FreeBSD and Linux have all had serious NFS client bugs which
significantly amplified the damage from even a momentary failure or overload.

What makes this worse is that most of these failures are never logged in any
form, making it hard for system administrators to take any sort of prompt
action.

What it does
============

MountStatusMonitor provides the missing notification piece and is a handy tool
for anyone who uses imperfect storage. It's a simple daemon which periodically
checks every mounted filesystem for failures and, unlike most other monitoring
tools, MountStatusMonitor robustly handles the various failure modes which
result in a hang because it works by fork()ing a child process which attempts
to list the contents of the mountpoint and will trigger an error if the child 
never returns.

Normally it syslogs a message like this after running:

    MountStatusMonitor[2659]: Checked 42 mounts in 0 seconds

When something fails it logs a summary message like this:

    MountStatusMonitor[21900]: Checked 37 mounts in 60 seconds: 1 dead

Other information about the failed mount depends on the type of failure. Many
NFS failures will simply cause a process to hang when it accesses the mount
and so MountStatusMonitor works by forking a worker process which performs the
actual check, allowing the main process to record an error if the check takes
too long:

    MountStatusMonitor[21900]: Timed out waiting for child process 30686: sending SIGKILL

The worker process uses setuid to run as the owner of the mountpoint but it's
still possible to encounter permissions errors and those will be logged like
this:

    MountStatusMonitor[18038]: Couldn't check mountpoint /example: mode 4000 does not allow access


Installation
============

The simple process is "make install". The binary needs to run as root -
installation could be as simple as installing it in /usr/local/sbin and adding
a simple cron @reboot entry to start it automatically.

Future Directions
=================

A long term experiment is having MountStatusMonitor actually attempt to do a
umount -f (or -l on Linux) to reduce the number of applications which hit a
dead mountpoint. This is somewhat risky and I haven't released any code
publicly. If you have any suggestions, email chris@improbable.org.