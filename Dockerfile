FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin bpfquery



# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/bpfquery /usr/local/bin
COPY --from=builder /app/static ./static
COPY --from=builder /app/fly_linux_kernel_definitions.db linux_kernel_definitions.db
COPY --from=builder /app/bpftrace_machine bpftrace_machine
RUN ssh-add bpftrace_machine
RUN apt-get update 
RUN apt-get install -y bpftrace 
RUN apt-get install -y linux-headers-generic
ENTRYPOINT ["/usr/local/bin/bpfquery", "localhost"]
