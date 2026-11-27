# BUILDING.md — building and packaging the executable

There is one Cargo project. Every operating system builds the same source into a different
executable; no per-OS crate is required. Native builds are the recommended path.

The project requires **Rust 1.99 or newer** (`rust-version = "1.99"` in `Cargo.toml`). It uses
`rustls`-free, pure-Rust compression through `zlib-rs`; no OpenSSL or other system library is
needed.

## Targets

| Platform | Rust target | Artifact |
|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | `mcp-chinese-dict-search.exe` |
| Linux x64, glibc | `x86_64-unknown-linux-gnu` | `mcp-chinese-dict-search` |
| Linux x64, static | `x86_64-unknown-linux-musl` | `mcp-chinese-dict-search` |
| Linux arm64 | `aarch64-unknown-linux-gnu` | `mcp-chinese-dict-search` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `mcp-chinese-dict-search` |
| macOS Intel | `x86_64-apple-darwin` | `mcp-chinese-dict-search` |

Check the local toolchain:

```sh
rustc --version
cargo --version
rustup target list --installed
```

## Windows x64

Install the **Visual Studio Build Tools** with the “Desktop development with C++” workload, then:

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
```

Output: `target\x86_64-pc-windows-msvc\release\mcp-chinese-dict-search.exe`.
A plain `cargo build --release` on a Windows x64 host produces the same binary under
`target\release\`.

## Linux

```sh
sudo apt-get update
sudo apt-get install -y build-essential
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu
```

For a binary that does not depend on the build machine's glibc, use musl:

```sh
sudo apt-get install -y musl-tools
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

A musl build is statically linked; `ldd` reports “not a dynamic executable”.

## macOS

Install the Xcode Command Line Tools, then build for the host:

```sh
xcode-select --install
cargo build --release
```

For a universal binary:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo -create -output mcp-chinese-dict-search \
  target/aarch64-apple-darwin/release/mcp-chinese-dict-search \
  target/x86_64-apple-darwin/release/mcp-chinese-dict-search
```

macOS targets cannot practically be cross-built from Windows or Linux because Apple's SDK is
required. Build on a Mac or a hosted macOS runner.

## Cross-compilation

`cross` runs builds inside Docker images with the target linker and libc preinstalled:

```sh
cargo install cross --git https://github.com/cross-rs/cross
rustup target add x86_64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-gnu
cross build --release --target x86_64-unknown-linux-gnu
cross build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-gnu
```

Docker Desktop or Docker Engine must be running. From Windows without `cross`, use WSL and the
Linux native instructions.

## CI matrix

```yaml
name: build-binaries
on:
  workflow_dispatch:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: mcp-chinese-dict-search.exe
            bundle: windows-x64
          - os: ubuntu-latest
            target: x86_64-unknown-linux-musl
            artifact: mcp-chinese-dict-search
            bundle: linux-x64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: mcp-chinese-dict-search
            bundle: macos-arm64
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: mcp-chinese-dict-search
            bundle: macos-x64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust and target
        run: |
          rustup toolchain install 1.99.0 --profile minimal
          rustup default 1.99.0
          rustup target add ${{ matrix.target }}
      - name: Install musl tools
        if: matrix.target == 'x86_64-unknown-linux-musl'
        run: |
          sudo apt-get update
          sudo apt-get install -y musl-tools
      - name: Build
        run: cargo build --release --target ${{ matrix.target }}
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.bundle }}
          path: target/${{ matrix.target }}/release/${{ matrix.artifact }}
          if-no-files-found: error
```

## Optional semantic feature

The default build links no HTTP client and is lexical-only. To build the gated Ollama fallback:

```sh
cargo build --release --features semantic
cargo clippy --all-targets --features semantic -- -D warnings
cargo test --all-targets --features semantic
```

The feature adds `ureq` and the `src/semantic/` modules. It still requires a local Ollama server
and a separately built index at runtime; no model weights enter the binary or the repository. The
default `cargo tree` must not contain `ureq`; check with `cargo tree | grep ureq`.

## Windows bundle

`dist/` is a self-contained Windows x64 bundle. Refresh it after a release build:

```powershell
$Bundle = "dist"
Copy-Item "target\release\mcp-chinese-dict-search.exe" $Bundle -Force
Copy-Item LICENSE, dist\INSTALL.md, dist\install.ps1, dist\mcp-config.example.json $Bundle -Force

Get-FileHash "$Bundle\mcp-chinese-dict-search.exe", "$Bundle\install.ps1", `
  "$Bundle\INSTALL.md", "$Bundle\mcp-config.example.json", "$Bundle\LICENSE" `
  -Algorithm SHA256 |
  ForEach-Object { "$($_.Hash.ToLower())  $(Split-Path -Leaf $_.Path)" } |
  Set-Content "$Bundle\SHA256SUMS.txt"
```

Never copy the dictionary archive, extracted Markdown, images, an EPUB, or any credential into
`dist/`. `tests/dist_bundle.rs` enforces the version line and the absence of dictionary data.

## Linux/macOS bundle

```sh
VERSION=0.6.0
TARGET=x86_64-unknown-linux-musl
PLATFORM=linux-x64
BUNDLE="dist/$PLATFORM"

mkdir -p "$BUNDLE"
cp "target/$TARGET/release/mcp-chinese-dict-search" "$BUNDLE/"
cp LICENSE "$BUNDLE/"
chmod +x "$BUNDLE/mcp-chinese-dict-search"
( cd "$BUNDLE" && sha256sum mcp-chinese-dict-search LICENSE > SHA256SUMS.txt )
tar -czf "dist/mcp-chinese-dict-search-${VERSION}-${PLATFORM}.tar.gz" -C dist "$PLATFORM"
```

Add a platform-specific `INSTALL.md` and, if desired, an `install.sh`. It should copy the binary
to `~/.local/bin` and print the MCP client entry; it must not copy dictionary data.

## Verify a build

```sh
./mcp-chinese-dict-search --version
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}' \
  | ./mcp-chinese-dict-search --data "/absolute/path/to/dictionary.zip"
```

Expected: the version, then a JSON-RPC result containing `"serverInfo"` and
`"protocolVersion":"2025-11-25"`. Run the handshake and one `tools/call` on each target OS, or in a
matching CI job:

```sh
cargo test --all-targets
```

To check the lookup indices and the cache without a client, run
`./mcp-chinese-dict-search --build-indices --data <archive>`: it rebuilds them, writes the cache,
prints the key counts to stderr, and exits.

To build the linked data copy, run
`./mcp-chinese-dict-search --data <archive-or-directory> --build-linked-data <copy-directory>`:
it copies the Markdown, inserts the index anchors, unescapes the conversion's literal brackets,
verifies the entry text and the fragment links, prints the counts, and exits. The server is then
pointed at the copy with `--data`; the source is untouched.

When the crate version changes, update `**Version:**` in `dist/INSTALL.md`; `tests/dist_bundle.rs`
fails if it drifts from `CARGO_PKG_VERSION`.

## Platform notes

- **Linux glibc floor.** A glibc build runs only on systems with a glibc at least as new as the
  build machine's. Build on the oldest distribution you support, or ship the musl binary.
- **macOS Gatekeeper.** Verify the SHA-256 of a downloaded unsigned binary, then remove the
  quarantine attribute if your policy allows: `xattr -d com.apple.quarantine mcp-chinese-dict-search`.
- **Windows SmartScreen.** An unsigned executable may warn on first run. Code-sign it for
  enterprise distribution.
- **Data paths are OS-specific.** Use absolute paths in MCP client configuration; the archive
  filename contains spaces, so quote it in shells.
- **No network.** Proxy variables are irrelevant; the server reads local files only.
