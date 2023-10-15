FROM arm64v8/rust:1.73.0

WORKDIR /opds_server

ENTRYPOINT cd /opds_server && git pull && cargo build --release -j 6

