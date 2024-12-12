FROM ubuntu:22.04
RUN apt-get update && apt-get install -y curl git python3 libclang-dev cmake pkg-config g++ libglib2.0-dev build-essential libc6-dev lsb-release

WORKDIR /app
ENV CPU=x64
ENV LINKING=static

COPY args.gn .
COPY build.sh .

RUN chmod 755 build.sh
RUN mkdir linux-x64

CMD ["bash", "-c", "./build.sh linux ${CPU} ${LINKING} no-install"]
