#!/bin/bash

NAME=opds-server-update-arm64v8

VOLUME=opds-server:/opds_server

docker build -t ${NAME} --platform linux/arm64 .

docker container run --platform linux/arm64 -v ${VOLUME} --name ${NAME} ${NAME}

docker cp  ${NAME}:/opds_server/target/release/opds_server .

docker container rm ${NAME}

