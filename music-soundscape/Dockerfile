# syntax=docker/dockerfile:1

# ---- Stage 1: compile the Rust synth and render the album -------------------
FROM rust:1-slim AS synth
WORKDIR /build
COPY synth/Cargo.toml synth/Cargo.lock ./
COPY synth/src ./src
RUN cargo build --release \
 && ./target/release/soundscape /build/out

# ---- Stage 2: tiny Node runtime serving the dashboard + audio ---------------
FROM node:22-alpine
ENV NODE_ENV=production \
    PORT=3000 \
    AUDIO_DIR=/app/audio
WORKDIR /app
COPY server/package.json server/server.js ./
COPY server/public ./public
COPY --from=synth /build/out ./audio
USER node
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
  CMD wget -qO- http://127.0.0.1:3000/api/health || exit 1
CMD ["node", "server.js"]
