CC=gcc
CFLAGS=-Wall -Wextra -Os --std=c99

all:
	mkdir -p build
	$(CC) $(CFLAGS) -o build/MountStatusMonitor main.c

clean:
	rm -rf build

install: all
	install -m 555 -o root -g 0 build/MountStatusMonitor /usr/local/sbin/MountStatusMonitor 	
