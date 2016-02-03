CC=gcc
CFLAGS=-Wall -Wextra -Os --std=c99

all:
	mkdir -p build
	$(CC) $(CFLAGS) -o build/MountStatusMonitor main.c

clean:
	rm -rf build *.deb *.rpm

install: all
	install -m 555 -o root -g 0 build/MountStatusMonitor /usr/local/sbin/MountStatusMonitor 	

rpm: all
	fpm -s dir -t rpm -n MountStatusMonitor build/MountStatusMonitor=/usr/sbin/MountStatusMonitor upstart/MountStatusMonitor.conf=/etc/init/MountStatusMonitor.conf

deb: all
	fpm -s dir -t deb -n MountStatusMonitor build/MountStatusMonitor=/usr/sbin/MountStatusMonitor upstart/MountStatusMonitor.conf=/etc/init/MountStatusMonitor.conf
