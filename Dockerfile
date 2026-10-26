# syntax=docker/dockerfile:1
# Paint, version web en conteneur : le moteur Rust est compilé en WebAssembly,
# puis l'interface est servie par nginx (sans root, port 8080 dans le conteneur).
#
#   docker compose up -d                              → http://localhost:8088/
#   docker build -t paint . && docker run --rm -p 127.0.0.1:8088:8080 paint

FROM rust:1.99-slim-trixie AS build
ARG TARGETARCH
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl \
 && rm -rf /var/lib/apt/lists/*
# wasm-pack 0.13.1 (même version que ./run), archive vérifiée par somme SHA-256.
RUN set -eux; \
    case "${TARGETARCH:-amd64}" in \
      amd64) arch=x86_64;  sum=c539d91ccab2591a7e975bcf82c82e1911b03335c80aa83d67ad25ed2ad06539 ;; \
      arm64) arch=aarch64; sum=2e65038769f8bbaa5fc237ad4bb523e692df99458cbd3e3d92525b89d8762379 ;; \
      *) echo "Architecture non prise en charge : $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    name="wasm-pack-v0.13.1-$arch-unknown-linux-musl"; \
    curl -fsSL -o /tmp/wasm-pack.tgz "https://github.com/rustwasm/wasm-pack/releases/download/v0.13.1/$name.tar.gz"; \
    echo "$sum  /tmp/wasm-pack.tgz" | sha256sum -c -; \
    tar -xzf /tmp/wasm-pack.tgz -C /usr/local/bin --strip-components=1 "$name/wasm-pack"; \
    rm /tmp/wasm-pack.tgz; \
    rustup target add wasm32-unknown-unknown

WORKDIR /src
# src-tauri n'est pas compilé ici, mais son manifeste fait partie de l'espace de travail.
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY src-tauri src-tauri
COPY ui ui
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    --mount=type=cache,target=/root/.cache/.wasm-pack \
    wasm-pack build crates/paint-wasm --target web --release --out-dir ../../ui/pkg --no-typescript -- --locked \
 && rm -f ui/pkg/.gitignore ui/pkg/package.json

FROM nginxinc/nginx-unprivileged:1.30-alpine-slim
LABEL org.opencontainers.image.title="Paint" \
      org.opencontainers.image.description="Éditeur d'images façon Paint Windows 11 (Rust + WebAssembly), version web"
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/ui /usr/share/nginx/html
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
  CMD wget -q --spider http://127.0.0.1:8080/ || exit 1
