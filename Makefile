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
	fpm -s dir -t rpm -n MountStatusMonitor --rpm-dist el6 build/MountStatusMonitor=/usr/sbin/MountStatusMonitor upstart/MountStatusMonitor.conf=/etc/init/MountStatusMonitor.conf
	fpm -s dir -t rpm -n MountStatusMonitor --rpm-dist el7 build/MountStatusMonitor=/usr/sbin/MountStatusMonitor MountStatusMonitor.service=/etc/systemd/system/MountStatusMonitor.service

deb: all
	fpm -s dir -t deb -n MountStatusMonitor build/MountStatusMonitor=/usr/sbin/MountStatusMonitor upstart/MountStatusMonitor.conf=/etc/init/MountStatusMonitor.conf
