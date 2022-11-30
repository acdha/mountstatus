PACKAGE_VERSION = $(shell cargo metadata --format-version=1 | jq -r '.packages[] | select(.name == "mount_status_monitor") | .version')

all: local deb el7

local:
	install -d packages
	cargo clean
	cargo generate-lockfile

deb: local
	docker build -f Dockerfile.release-deb -t mountstatus:release-deb --build-arg PACKAGE_VERSION=${PACKAGE_VERSION} .
	docker run --rm -v $(realpath packages):/host-packages-volume mountstatus:release-deb

el7: local
	docker build -f Dockerfile.release-el7 -t mountstatus:release-el7 --build-arg PACKAGE_VERSION=${PACKAGE_VERSION} .
	docker run --rm -v $(realpath packages):/host-packages-volume mountstatus:release-el7
