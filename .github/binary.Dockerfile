FROM ubuntu:24.10 AS downloader
 RUN apt-get update && apt-get install -y curl unzip
 ARG SHINKAI_NODE_VERSION
 RUN curl -L -o shinkai-node.zip https://download.shinkai.com/shinkai-node/binaries/production/x86_64-unknown-linux-gnu/${SHINKAI_NODE_VERSION:-latest}.zip
 RUN FILE_SIZE=$(stat -c %s /shinkai-node.zip) && \
    if [ $FILE_SIZE -lt 26214400 ]; then \
        echo "Error: shinkai-node file is less than 25MB" && \
        exit 1; \
    fi
 RUN unzip shinkai-node.zip    
 RUN chmod +x /shinkai-node

 FROM ubuntu:24.10
 RUN apt-get update && apt-get install -y openssl ca-certificates
 WORKDIR /app
 COPY --from=downloader /shinkai-node ./shinkai-node

 EXPOSE 9550
 ENTRYPOINT ["/bin/sh", "-c", "/app/shinkai-node"]