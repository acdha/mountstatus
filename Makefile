CC=gcc
CFLAGS=-Wall -Wextra -Os --std=c99

all:
	mkdir -p build
	$(CC) $(CFLAGS) -o build/MountStatusMonitor main.c

clean:
	/bin/rm -rf build
