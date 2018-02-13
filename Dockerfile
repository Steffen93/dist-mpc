FROM ubuntu:16.04

MAINTAINER Steffen Härtlein <steffen.haertlein@campus.tu-berlin.de>

ARG libsodium_version=libsodium-stable-2018-02-13
ARG git_user
ARG git_pw

WORKDIR /root

# install libsnark dependencies and build it

RUN apt-get update && \
    apt-get install -y \
    wget unzip curl \
    build-essential cmake git libgmp3-dev libprocps4-dev python-markdown libboost-all-dev libssl-dev pkg-config

RUN git clone https://github.com/scipr-lab/libsnark \
  && cd libsnark \
  && git submodule init \
  && git submodule update \
  && mkdir build && cd build \
  && cmake ..

# install libsodium
RUN wget https://download.libsodium.org/libsodium/releases/${libsodium_version}.tar.gz
RUN tar -xf ${libsodium_version}.tar.gz \
  && cd libsodium-stable \
  && ./configure \
  && make && make check \
  && make install

# install rust

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

ENV PATH=/root/.cargo/bin:$PATH

RUN cd libsnark/build \
  && make \
  && DESTDIR=/usr/local/lib make install

ENV LD_LIBRARY_PATH $LD_LIBRARY_PATH:/usr/local/lib

# install npm and truffle

RUN curl -sL https://deb.nodesource.com/setup_6.x | bash - \
  && apt-get install -y nodejs \
  && npm i -g truffle

# build library

RUN git clone https://${git_user}:${git_pw}@github.com/steffen93/dist-mpc --branch rust-to-blockchain \
    && cd dist-mpc/blockchain && truffle compile \
    && cd ../mpc && cargo build --bin player