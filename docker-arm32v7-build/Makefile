.PHONY: init build clean

IMAGE = opds-server-arm32v7
PLATFORM = linux/arm/v7
VOLUME_NAME = opds-server-volume
VOLUME_MOUNT_POINT = /volume
BINARY_PATH = $(VOLUME_MOUNT_POINT)/opds_server/target/release/opds_server
VOLUME = $(VOLUME_NAME):$(VOLUME_MOUNT_POINT)

init:
	docker run --privileged --rm tonistiigi/binfmt --install all
	docker build --platform $(PLATFORM) -t $(IMAGE) .

build:
	docker run --platform $(PLATFORM) --rm -v $(VOLUME) $(IMAGE) git pull
	docker run --platform $(PLATFORM) --rm -v $(VOLUME) $(IMAGE) cargo build --release
	docker run --platform $(PLATFORM) --rm -v $(VOLUME) -v  $(CURDIR):/destination $(IMAGE) cp $(BINARY_PATH) /destination

clean:
	docker image rm $(IMAGE)
	docker volume rm $(VOLUME_NAME)
