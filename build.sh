#!/bin/bash
# Build script for all platforms
# Run on each target platform or use cross-compilation

set -e

echo "=== CC Proxy Build Script ==="
echo ""

# Check prerequisites
command -v pnpm >/dev/null 2>&1 || { echo "pnpm required"; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo "cargo required"; exit 1; }

# Install deps
pnpm install

# Build
echo "Building..."
pnpm tauri build

echo ""
echo "=== Build Complete ==="
echo "Output:"
find src-tauri/target/release/bundle -name "*.exe" -o -name "*.msi" -o -name "*.deb" -o -name "*.AppImage" -o -name "*.dmg" 2>/dev/null
