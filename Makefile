PACKAGE_VERSION = $(shell cargo metadata --format-version=1 | jq -r '.packages[] | select(.name == "mount_status_monitor") | .version')

all: local deb el8

local:
	install -d packages
	cargo clean
	cargo generate-lockfile

deb: local
	podman build --arch amd64 -f Dockerfile.release-deb -t mountstatus:release-deb --build-arg PACKAGE_VERSION=${PACKAGE_VERSION} .
	podman run --arch amd64 --rm -v $(realpath packages):/host-packages-volume mountstatus:release-deb

el7: local
	podman build --arch amd64 -f Dockerfile.release-el7 -t mountstatus:release-el7 --build-arg PACKAGE_VERSION=${PACKAGE_VERSION} .
	podman run --arch amd64 --rm -v $(realpath packages):/host-packages-volume mountstatus:release-el7

el8: local
	podman build --arch amd64 -f Dockerfile.release-el8 -t mountstatus:release-el8 --build-arg PACKAGE_VERSION=${PACKAGE_VERSION} .
	podman run --arch amd64 --rm -v $(realpath packages):/host-packages-volume mountstatus:release-el8
