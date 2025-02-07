FROM messense/rust-musl-cross:x86_64-musl AS build
COPY ./ /home/rust/src
# Uncomment if building behind proxy with a custom CA certificate.
#COPY cacert.gitignore.crt /usr/local/share/ca-certificates/proxyca.crt
#RUN update-ca-certificates
RUN --mount=type=cache,target=/home/rust/src/target \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo build --release && \
    mv /home/rust/src/target/x86_64-unknown-linux-musl/release/embed-server /embed-server

FROM scratch
USER 1001:1001
COPY --from=build /embed-server /embed-server
COPY ./embed-server/config.prod.toml /config.toml
ENV EMBED_BIND_ADDRESS="0.0.0.0:8050"
EXPOSE 8050/tcp
ENTRYPOINT ["/embed-server"]