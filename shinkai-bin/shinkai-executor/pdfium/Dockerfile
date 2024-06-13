FROM ubuntu:22.04
RUN apt-get update && apt-get install -y curl git python3 libclang-dev cmake pkg-config g++

WORKDIR /app

COPY args.gn .
COPY build.sh .

RUN chmod 755 build.sh
RUN mkdir linux-x64

CMD ["./build.sh", "linux", "x64", "no-install"]
