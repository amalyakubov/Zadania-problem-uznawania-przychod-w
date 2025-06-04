FROM rust:latest AS build

# create a new empty shell project
RUN USER=root cargo new --bin Untergang
WORKDIR /Untergang

# copy over your manifests
COPY ./Untergang/Cargo.lock ./Cargo.lock
COPY ./Untergang/Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./Untergang/src ./src

# build for release
RUN rm ./target/release/deps/Untergang*
RUN cargo build --release

# our final base - use a smaller image for production
FROM debian:bookworm-slim

# Install runtime dependencies if needed
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# copy the build artifact from the build stage
COPY --from=build /Untergang/target/release/Untergang .

# Expose the port the app will run on
EXPOSE 3000

# set the startup command to run your binary
CMD ["./Untergang"]
