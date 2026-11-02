FROM ubuntu:22.04@sha256:076e7bf82319619f28dd0aa9c02ab34b055ad37100395b4f49a2ffba04fb5e11

# Bootstrap HTTPS trust without disabling certificate or APT signature checks.
ADD --chmod=644 --checksum=sha256:796f1389bfea13c0ed78f663b765baaff54a508e5483786bd6ece73c5e3e3cbe https://curl.se/ca/cacert-2026-10-20.pem /etc/ssl/certs/ca-certificates.crt

# Signed, immutable Ubuntu package indexes freeze the full native dependency
# closure, rather than pinning only the top-level development packages.
RUN chmod 755 /etc/ssl /etc/ssl/certs \
 && chmod 644 /etc/ssl/certs/ca-certificates.crt \
 && printf '%s\n' \
    'deb [check-valid-until=no] https://snapshot.ubuntu.com/ubuntu/20261101T000000Z jammy main universe' \
    'deb [check-valid-until=no] https://snapshot.ubuntu.com/ubuntu/20261101T000000Z jammy-updates main universe' \
    'deb [check-valid-until=no] https://snapshot.ubuntu.com/ubuntu/20261101T000000Z jammy-security main universe' \
    > /etc/apt/sources.list \
 && apt-get -o Acquire::https::CaInfo=/etc/ssl/certs/ca-certificates.crt update \
 && DEBIAN_FRONTEND=noninteractive apt-get -o Acquire::https::CaInfo=/etc/ssl/certs/ca-certificates.crt install -y --no-install-recommends \
    build-essential ca-certificates curl xz-utils git pkg-config python3 file wget \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    libssl-dev patchelf dbus-x11 gnome-keyring xvfb \
 && rm -rf /var/lib/apt/lists/*

RUN curl --fail --location --proto '=https' \
    https://static.rust-lang.org/dist/rust-1.98.1-x86_64-unknown-linux-gnu.tar.xz -o /tmp/rust.tar.xz \
 && echo '5326b36c53de11d148c8f8dab6553a3d1006c2cfd32123683073fad3c302605b  /tmp/rust.tar.xz' | sha256sum -c - \
 && tar -xJf /tmp/rust.tar.xz -C /tmp \
 && /tmp/rust-1.98.1-x86_64-unknown-linux-gnu/install.sh --prefix=/opt/rust --without=rust-docs --disable-ldconfig \
 && rm -rf /tmp/rust.tar.xz /tmp/rust-1.98.1-x86_64-unknown-linux-gnu

RUN curl --fail --location --proto '=https' \
    https://nodejs.org/dist/v24.15.0/node-v24.15.0-linux-x64.tar.xz -o /tmp/node.tar.xz \
 && echo '472655581fb851559730c48763e0c9d3bc25975c59d518003fc0849d3e4ba0f6  /tmp/node.tar.xz' | sha256sum -c - \
 && mkdir /opt/node \
 && tar -xJf /tmp/node.tar.xz --strip-components=1 -C /opt/node \
 && rm /tmp/node.tar.xz

ENV PATH="/opt/rust/bin:/opt/node/bin:${PATH}"
RUN curl --fail --location --proto '=https' https://registry.npmjs.org/pnpm/-/pnpm-10.29.3.tgz -o /tmp/pnpm.tgz \
 && echo 'a74f4dbd3f5a7ca874d004135271f0b697b683bf1fd2fc2133961161cd25b29c  /tmp/pnpm.tgz' | sha256sum -c - \
 && mkdir /opt/pnpm \
 && tar -xzf /tmp/pnpm.tgz --strip-components=1 -C /opt/pnpm \
 && ln -s /opt/pnpm/bin/pnpm.cjs /opt/node/bin/pnpm \
 && rm /tmp/pnpm.tgz
ENV CARGO_BUILD_JOBS=2
WORKDIR /workspace
