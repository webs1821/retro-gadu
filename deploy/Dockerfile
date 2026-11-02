# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 webs1821
# Official images resolved 2026-11-01. Build through tools/build_container.py.
FROM rust:1.99.1-slim-bookworm@sha256:24707e70a4f1e23d9a7b25202ec43126d28a52294b96aad5c670df1fd1f1d400 AS build
ARG SOURCE_COMMIT
ARG VERSION
ENV CARGO_TARGET_DIR=/build-target CARGO_BUILD_JOBS=2
WORKDIR /src
COPY . .
RUN mkdir -p .cargo \
    && cargo vendor --locked --versioned-dirs vendor > .cargo/config.toml \
    && cargo build --release --workspace --frozen \
    && test "$(/build-target/release/rgg-server --version)" = "Retro Gadu (rGG) ${VERSION}" \
    && printf '%s\n' "$SOURCE_COMMIT" > SOURCE-COMMIT \
    && tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner -cf /source.tar . \
    && gzip -n /source.tar

FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3 AS runtime
ARG SOURCE_COMMIT
ARG VERSION
LABEL org.opencontainers.image.title="Retro Gadu" \
      org.opencontainers.image.source="https://github.com/webs1821/retro-gadu" \
      org.opencontainers.image.revision="${SOURCE_COMMIT}" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"
COPY --from=build /build-target/release/rgg-server /build-target/release/rgg-admin /usr/local/bin/
COPY --from=build /source.tar.gz /usr/share/rgg/source.tar.gz
COPY LICENSE THIRD_PARTY.toml /usr/share/rgg/
COPY config/rgg.container.toml /etc/rgg/rgg.toml
RUN install -d -m 0700 -o 10001 -g 10001 /data \
    && install -d -m 0755 -o 10001 -g 10001 /app
USER 10001:10001
WORKDIR /app
STOPSIGNAL SIGTERM
CMD ["/usr/local/bin/rgg-server", "--config", "/etc/rgg/rgg.toml"]

# Optional test runner; the runtime image itself does not contain Python.
FROM python:3.13-slim-bookworm@sha256:281d39ff02e3b389401586134ff9e6572954687822a58055ec07a60ded4e0b7f AS verification
# The local TLS fixture creates a disposable certificate; never part of the runtime stage.
RUN openssl version
ARG SOURCE_COMMIT
ARG VERSION
LABEL org.opencontainers.image.revision="${SOURCE_COMMIT}" org.opencontainers.image.version="${VERSION}"
COPY --from=runtime /usr/local/bin/rgg-server /usr/local/bin/rgg-admin /usr/local/bin/
COPY --from=runtime /usr/share/rgg /usr/share/rgg
COPY tests /src/tests
COPY tools /src/tools
ENV PYTHONDONTWRITEBYTECODE=1
USER 10001:10001
WORKDIR /src
CMD ["python", "tests/lifecycle.py", "--binary", "/usr/local/bin/rgg-server"]

FROM runtime AS final
