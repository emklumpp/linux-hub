# syntax=docker/dockerfile:1

# ---- stage 1: compile the Rust fetcher ---------------------------------------
# Full (non-slim) rust image: `ring` needs a C toolchain to build.
FROM rust:1-bookworm AS fetcher-build
WORKDIR /build
# Compile dependencies against a stub main first so that editing src/main.rs
# doesn't invalidate the (slow) dependency layer.
COPY fetcher/Cargo.toml fetcher/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src
COPY fetcher/src ./src
# cargo keys off mtime; the stub above is newer than the real source we just copied.
RUN touch src/main.rs && cargo build --release

# ---- stage 2: production node_modules ----------------------------------------
FROM node:22-bookworm-slim AS node-deps
WORKDIR /app
COPY server/package.json server/package-lock.json ./
RUN npm ci --omit=dev

# ---- stage 3: runtime --------------------------------------------------------
# Same Debian release as the builder, so the fetcher's glibc linkage matches.
FROM node:22-bookworm-slim
ENV NODE_ENV=production \
    PORT=3000 \
    DATA_FILE=/data/cameras.json \
    FETCHER_BIN=/usr/local/bin/flock-fetch

WORKDIR /app
COPY --from=node-deps /app/node_modules ./node_modules
COPY server/package.json server/server.js ./
COPY server/public ./public
COPY --from=fetcher-build /build/target/release/flock-fetch /usr/local/bin/flock-fetch

# TLS roots are compiled into the fetcher via webpki-roots, so no ca-certificates
# package is required here.
RUN mkdir -p /data && chown -R node:node /data
USER node
VOLUME ["/data"]
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||3000)+'/api/status').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"

CMD ["node", "server.js"]
