PACKAGE_VERSION = $(shell cargo metadata --format-version=1 | jq -r '.packages[] | select(.name == "mount_status_monitor") | .version')

all:
	install -d packages
	cargo clean
	cargo generate-lockfile
	docker build -f Dockerfile.release -t mountstatus:release --build-arg PACKAGE_VERSION=${PACKAGE_VERSION} .
	docker run --rm -v $(realpath packages):/host-packages-volume mountstatus:release