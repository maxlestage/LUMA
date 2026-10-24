# Le site de LÜMA, une app Paquets (pages rendues en Rust, îlots et animations en
# WebAssembly). Heroku le construit avec le buildpack emk/rust (RustConfig, Procfile) ;
# cette image est l'autre voie : la stack container (heroku.yml), ou tout hôte Docker.
#
#   docker build --build-arg REPOS_TOKEN=… -t luma . && docker run --rm -p 8080:8080 luma

FROM rust:1-slim-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates git \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown
# Paquets et ses paquets (Vitesse, Securite, Memoire, Active) sont des dépôts git
# privés : un jeton GitHub qui peut les lire, passé à git sans apparaître dans une URL.
ARG REPOS_TOKEN=""
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
WORKDIR /src
COPY . .
WORKDIR /src/site
RUN if [ -n "$REPOS_TOKEN" ]; then \
      git config --global credential.https://github.com.helper \
        '!f() { test "$1" = get && echo username=x-access-token && echo "password='"$REPOS_TOKEN"'"; }; f'; \
    fi \
    && cargo build --lib --target wasm32-unknown-unknown --profile wasm-release \
    && version=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p') \
    && (curl -sSfL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${version}/wasm-bindgen-${version}-x86_64-unknown-linux-musl.tar.gz" \
            | tar -xz --strip-components=1 -C /usr/local/bin \
        || cargo install wasm-bindgen-cli --version "$version" --locked) \
    && wasm-bindgen --target web --no-typescript --out-dir pkg \
        target/wasm32-unknown-unknown/wasm-release/luma_site.wasm \
    && cargo build --release --bin luma-site && cp target/release/luma-site /server \
    && git config --global --unset-all credential.https://github.com.helper || true

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 luma
WORKDIR /app
COPY --from=build /server /usr/local/bin/server
COPY --from=build /src/site/public ./public
COPY --from=build /src/site/pkg ./pkg
USER luma
ENV PORT=8080
EXPOSE 8080
CMD ["server"]
