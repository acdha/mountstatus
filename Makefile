CC=gcc
CFLAGS=-Wall -Wextra -Os --std=c99

all:
	$(CC) $(CFLAGS) -o build/MountStatusMonitor main.c

clean:
	/bin/rm build/*
