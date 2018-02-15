FROM ubuntu:16.04

MAINTAINER Steffen Härtlein <steffen.haertlein@campus.tu-berlin.de>

WORKDIR /root

# install libsnark dependencies and build it

RUN apt-get update && \
    apt-get install -y \
    wget unzip curl git build-essential libssl-dev libgmp3-dev pkg-config

# install libsodium
ARG libsodium_version=LATEST

RUN wget https://download.libsodium.org/libsodium/releases/${libsodium_version}.tar.gz
RUN tar -xf ${libsodium_version}.tar.gz \
  && cd libsodium-stable \
  && ./configure \
  && make && make check \
  && make install

# install rust

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

ENV PATH=/root/.cargo/bin:$PATH

# install npm and truffle

RUN curl -sL https://deb.nodesource.com/setup_6.x | bash - \
  && apt-get install -y nodejs \
  && npm i -g truffle

# build library from git, use for production

ARG git_user
ARG git_pw
ARG git_branch=rust-to-blockchain
RUN git clone https://${git_user}:${git_pw}@github.com/steffen93/dist-mpc --branch ${git_branch}

ENV DIST_MPC_HOST=localhost

RUN cd dist-mpc/blockchain && truffle compile \
  && cd ../mpc && cargo build --bin player