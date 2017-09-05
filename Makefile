all:
	install -d packages
	docker build -f Dockerfile.release -t mountstatus:release .
	docker run --rm -v $(realpath packages):/host-packages-volume mountstatus:release
