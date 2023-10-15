#!/bin/bash

NAME=opds-server-init-arm64v8
VOLUME=opds-server:/opds_server

docker volume rm opds-server

docker run --privileged --rm tonistiigi/binfmt --install all

docker build -t ${NAME} --platform linux/arm64 .

docker container run -it --platform linux/arm64 --rm -v ${VOLUME} ${NAME}

