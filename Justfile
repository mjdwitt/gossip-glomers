set shell := ["bash", "-euo", "pipefail", "-c"]

maelstrom_version := "v0.2.4"
maelstrom_dir     := justfile_directory() / "maelstrom"
maelstrom_bin     := maelstrom_dir / "maelstrom"
rust_dir          := justfile_directory() / "rust"
go_dir            := justfile_directory() / "go"
verify            := "python3 " + justfile_directory() / "scripts/verify_run.py"

default:
    @just --list

# Download Maelstrom into ./maelstrom/. Idempotent.
install:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -x "{{maelstrom_bin}}" ]]; then
      echo "Maelstrom already installed at {{maelstrom_dir}}"
      exit 0
    fi
    url="https://github.com/jepsen-io/maelstrom/releases/download/{{maelstrom_version}}/maelstrom.tar.bz2"
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    echo "Downloading $url"
    curl -fL "$url" -o "$tmp/maelstrom.tar.bz2"
    tar -xjf "$tmp/maelstrom.tar.bz2" -C "{{justfile_directory()}}"
    test -x "{{maelstrom_bin}}"
    echo "Installed Maelstrom {{maelstrom_version}} at {{maelstrom_dir}}"

# Build the binary for (lang, crate) and run `maelstrom test` with the
# given args. `crate` is the Rust crate name; the Go binary is
# `maelstrom-<crate>`.
_run lang crate maelstrom_args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{lang}}" in
      rust)
        (cd "{{rust_dir}}" && cargo build --release -p {{crate}})
        bin="{{rust_dir}}/target/release/{{crate}}"
        ;;
      go)
        mkdir -p "{{go_dir}}/bin"
        (cd "{{go_dir}}" && go build -o "bin/maelstrom-{{crate}}" "./cmd/maelstrom-{{crate}}")
        bin="{{go_dir}}/bin/maelstrom-{{crate}}"
        ;;
      *)
        echo "unknown lang: {{lang}} (expected rust|go)" >&2
        exit 1
        ;;
    esac
    cd "{{maelstrom_dir}}"
    ./maelstrom test --bin "$bin" {{maelstrom_args}}

# --- challenges ---

# 1. Echo
challenge-1 lang="rust":
    @just _run {{lang}} echo "-w echo --node-count 1 --time-limit 10"
    {{verify}} --workload echo

# 2. Unique ID generation
challenge-2 lang="rust":
    @just _run {{lang}} unique-ids "-w unique-ids --time-limit 30 --rate 1000 --node-count 3 --availability total --nemesis partition"
    {{verify}} --workload unique-ids

# 3a. Single-node broadcast
challenge-3a lang="rust":
    @just _run {{lang}} broadcast "-w broadcast --node-count 1 --time-limit 20 --rate 10"
    {{verify}} --workload broadcast

# 3b. Multi-node broadcast
challenge-3b lang="rust":
    @just _run {{lang}} broadcast "-w broadcast --node-count 5 --time-limit 20 --rate 10"
    {{verify}} --workload broadcast

# 3c. Fault-tolerant broadcast
challenge-3c lang="rust":
    @just _run {{lang}} broadcast "-w broadcast --node-count 5 --time-limit 20 --rate 10 --nemesis partition"
    {{verify}} --workload broadcast

# 3d. Efficient broadcast I — msgs/op < 30, median latency < 400ms, max < 600ms
challenge-3d lang="rust":
    @just _run {{lang}} broadcast "-w broadcast --node-count 25 --time-limit 20 --rate 100 --latency 100"
    {{verify}} --workload broadcast --max-msgs-per-op 30 --max-median-latency-ms 400 --max-latency-ms 600

# 3e. Efficient broadcast II — msgs/op < 20, median < 1000ms, max < 2000ms
challenge-3e lang="rust":
    @just _run {{lang}} broadcast "-w broadcast --node-count 25 --time-limit 20 --rate 100 --latency 100"
    {{verify}} --workload broadcast --max-msgs-per-op 20 --max-median-latency-ms 1000 --max-latency-ms 2000
