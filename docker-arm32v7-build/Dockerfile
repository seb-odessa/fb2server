FROM arm32v7/rust:slim

WORKDIR /volume

RUN apt-get update && apt-get install git -y && git clone https://github.com/seb-odessa/opds_server.git

# Переход в каталог проекта
WORKDIR /volume/opds_server

# Команда по умолчанию
CMD ["cargo", "build", "--release", "-j", "4"]
