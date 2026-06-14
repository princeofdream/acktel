#!/bin/bash
# Build acktel for AckShell (Linux/macOS)
# Pure Rust project — uses cargo, no CMake required
set -e

ARCH=""
BUILD_DIR=""
OUTPUT_DIR=""
TOOLCHAIN_ROOT=""
CLEAN=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        -a|--arch) ARCH="$2"; shift 2 ;;
        -b|--build) BUILD_DIR="$2"; shift 2 ;;
        -o|--output) OUTPUT_DIR="$2"; shift 2 ;;
        -t|--toolchain) TOOLCHAIN_ROOT="$2"; shift 2 ;;
        -c|--clean) CLEAN=1; shift ;;
        -h|--help)
            echo "Usage: $0 -a <arch> [-t <toolchain_root>] [-o <output>] [-c]"
            echo ""
            echo "Arguments:"
            echo "  -a, --arch <arch>       Architecture: x86_64, aarch64, armv7"
            echo "  -t, --toolchain <path>  Rust target triple (e.g. aarch64-unknown-linux-gnu)"
            echo "  -o, --output <dir>      Install directory (binary copied to <dir>/bin/)"
            echo "  -c, --clean             Clean before build"
            echo "  -h, --help              Show help"
            echo ""
            echo "Dependencies:"
            echo "  - Rust toolchain (rustc + cargo)"
            echo "  - For cross-compilation: rustup target add <triple>"
            exit 0
            ;;
        x86_64|x86|aarch64|armv7) ARCH="$1"; shift ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

ARCH="${ARCH:-x86_64}"

OS="$(uname -s)"
case "$OS" in
    Linux)  PLATFORM_OS="linux" ;;
    Darwin) PLATFORM_OS="macos" ;;
    *) echo "Unsupported: $OS"; exit 1 ;;
esac

if [[ "$PLATFORM_OS" == "macos" && "$ARCH" == "armv7" ]]; then
    echo "Error: armv7 is not supported on macOS"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

MODULE_NAME="acktel"
DEFAULT_OUTPUT_DIR="$REPO_ROOT/output/target/$PLATFORM_OS/$ARCH"

OUTPUT_DIR="${OUTPUT_DIR:-$DEFAULT_OUTPUT_DIR}"

echo ""
echo "Building $MODULE_NAME ($PLATFORM_OS/$ARCH)"
echo ""

# ---------------------------------------------------------------------------
# Check Rust toolchain
# ---------------------------------------------------------------------------
for tool in rustc cargo; do
    if ! command -v "$tool" &> /dev/null; then
        echo "Error: $tool not found. Install Rust: https://rustup.rs"
        exit 1
    fi
done

echo "rustc : $(rustc --version)"
echo "cargo : $(cargo --version)"
echo ""

# ---------------------------------------------------------------------------
# Determine Rust target triple
# ---------------------------------------------------------------------------
NATIVE_ARCH="$(uname -m)"
NATIVE_ARCH="${NATIVE_ARCH/x86_64/x86_64}"
NATIVE_ARCH="${NATIVE_ARCH/aarch64/aarch64}"
NATIVE_ARCH="${NATIVE_ARCH/arm64/aarch64}"

TARGET_TRIPLE="${TOOLCHAIN_ROOT:-}"
if [[ -z "$TARGET_TRIPLE" ]]; then
    case "$PLATFORM_OS" in
        linux)
            case "$ARCH" in
                x86_64)  TARGET_TRIPLE="x86_64-unknown-linux-gnu" ;;
                aarch64) TARGET_TRIPLE="aarch64-unknown-linux-gnu" ;;
                armv7)   TARGET_TRIPLE="armv7-unknown-linux-gnueabihf" ;;
                *)       echo "Error: unsupported arch $ARCH for Linux"; exit 1 ;;
            esac
            ;;
        macos)
            case "$ARCH" in
                x86_64)  TARGET_TRIPLE="x86_64-apple-darwin" ;;
                aarch64) TARGET_TRIPLE="aarch64-apple-darwin" ;;
                *)       echo "Error: unsupported arch $ARCH for macOS"; exit 1 ;;
            esac
            ;;
    esac
fi

IS_CROSS=0
if [[ "$ARCH" != "$NATIVE_ARCH" ]]; then
    IS_CROSS=1
fi

CARGO_ARGS=("--manifest-path" "$SCRIPT_DIR/Cargo.toml" "--release")

if [[ $IS_CROSS -eq 1 ]]; then
    echo "Cross-compiling: $NATIVE_ARCH -> $ARCH ($TARGET_TRIPLE)"
    if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET_TRIPLE"; then
        echo "Installing Rust target: $TARGET_TRIPLE"
        rustup target add "$TARGET_TRIPLE"
    fi
    CARGO_ARGS+=("--target" "$TARGET_TRIPLE")
else
    echo "Compiling native: $ARCH"
fi

# ---------------------------------------------------------------------------
# Clean
# ---------------------------------------------------------------------------
if [[ "$CLEAN" == "1" ]]; then
    echo "Cleaning..."
    cargo clean --manifest-path "$SCRIPT_DIR/Cargo.toml"
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
echo ""
echo "Building release binary..."

cargo build "${CARGO_ARGS[@]}"

# ---------------------------------------------------------------------------
# Install binary + library
# ---------------------------------------------------------------------------
echo ""
echo "Installing to $OUTPUT_DIR/bin/"

if [[ $IS_CROSS -eq 1 ]]; then
    SRC_BINARY="$SCRIPT_DIR/target/$TARGET_TRIPLE/release/acktel"
    SRC_LIB="$SCRIPT_DIR/target/$TARGET_TRIPLE/release/libacktel.rlib"
else
    SRC_BINARY="$SCRIPT_DIR/target/release/acktel"
    SRC_LIB="$SCRIPT_DIR/target/release/libacktel.rlib"
fi

mkdir -p "$OUTPUT_DIR/bin"
cp "$SRC_BINARY" "$OUTPUT_DIR/bin/acktel"
echo "  -> $OUTPUT_DIR/bin/acktel"

# Copy library for nshell integration
mkdir -p "$OUTPUT_DIR/lib"
cp "$SRC_LIB" "$OUTPUT_DIR/lib/libacktel.rlib"
echo "  -> $OUTPUT_DIR/lib/libacktel.rlib"

echo ""
echo "Done!"
