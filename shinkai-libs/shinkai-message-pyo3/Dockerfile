# Use CentOS 7 as base image
FROM centos:7

# Install necessary development tools and libraries
RUN yum groupinstall -y "Development Tools"

# Install necessary libraries for building Python with custom SSL support
RUN yum install -y wget zlib-devel libffi-devel gcc make epel-release patchelf

# Build and install OpenSSL from source
RUN wget --no-check-certificate https://www.openssl.org/source/openssl-1.1.1l.tar.gz && \
    tar -xzf openssl-1.1.1l.tar.gz && \
    cd openssl-1.1.1l && \
    ./config --prefix=/usr/local/ssl --openssldir=/usr/local/ssl && \
    make && make install && \
    cd .. && \
    rm -rf openssl-1.1.1l.tar.gz openssl-1.1.1l

# Set environment variables for building Python
ENV LDFLAGS="-L/usr/local/ssl/lib"
ENV LD_LIBRARY_PATH="/usr/local/ssl/lib:/usr/local/lib"
ENV CPPFLAGS="-I/usr/local/ssl/include"
ENV OPENSSL_DIR=/usr/local/ssl
ENV OPENSSL_LIB_DIR=/usr/local/ssl/lib
ENV OPENSSL_INCLUDE_DIR=/usr/local/ssl/include

# Download and install Python 3.10 with custom SSL support
RUN wget https://www.python.org/ftp/python/3.10.0/Python-3.10.0.tgz && \
    tar -xzf Python-3.10.0.tgz && \
    cd Python-3.10.0 && \
    ./configure --enable-optimizations --with-ensurepip=install --with-openssl=/usr/local/ssl --enable-shared && \
    make altinstall && \
    cd .. && \
    rm -rf Python-3.10.0.tgz Python-3.10.0

# Set python3.10 and pip3.10 as the default python and pip versions
RUN ln -sf /usr/local/bin/python3.10 /usr/bin/python && \
    ln -sf /usr/local/bin/pip3.10 /usr/bin/pip && \
    ln -s /usr/local/lib/libpython3.10.so /usr/lib/ && \
    ln -s /usr/local/lib/libpython3.10.so.1.0 /usr/lib/

# Install Rust and Cargo (using Rust version 1.72.0)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain 1.72.0
ENV PATH="/root/.cargo/bin:${PATH}"
ENV PYO3_PYTHON=“/usr/local/lib/libpython3.10.so”

# Install maturin
RUN pip install maturin

# Check and ensure SSL for Python
RUN python --version && \
    python -c "import ssl; print(ssl.OPENSSL_VERSION)"

# Set the working directory
WORKDIR /project

RUN pip install patchelf

# Entrypoint
CMD ["/bin/bash"]

