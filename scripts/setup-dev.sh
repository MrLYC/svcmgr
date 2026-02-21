#!/bin/bash
# Setup script for svcmgr development environment
# Run this after cloning the repository

set -e

echo "🚀 Setting up svcmgr development environment..."

# 1. Install pre-commit hook
echo "📌 Installing pre-commit hook..."
if [ -f .git/hooks/pre-commit ]; then
    echo "⚠️  Pre-commit hook already exists. Backing up to .git/hooks/pre-commit.backup"
    mv .git/hooks/pre-commit .git/hooks/pre-commit.backup
fi

ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
echo "✅ Pre-commit hook installed"

# 2. Check Rust toolchain
echo "🦀 Checking Rust toolchain..."
if ! command -v rustc &> /dev/null; then
    echo "❌ Rust is not installed!"
    echo "💡 Install from: https://rustup.rs/"
    exit 1
fi
echo "✅ Rust version: $(rustc --version)"

# 3. Check required components
echo "🔧 Checking required components..."
if ! rustup component list --installed | grep -q rustfmt; then
    echo "📦 Installing rustfmt..."
    rustup component add rustfmt
fi
if ! rustup component list --installed | grep -q clippy; then
    echo "📦 Installing clippy..."
    rustup component add clippy
fi
echo "✅ Required components installed"

# 4. Build project
echo "🔨 Building project..."
cargo build
echo "✅ Build successful"

# 5. Run tests
echo "🧪 Running tests..."
cargo test --lib
echo "✅ Tests passed"

echo ""
echo "✨ Setup complete! You're ready to develop."
echo ""
echo "📚 Quick commands:"
echo "  cargo fmt --all          # Format code"
echo "  cargo clippy             # Check code quality"
echo "  cargo test               # Run tests"
echo "  cargo build              # Build project"
echo ""
echo "💡 Pre-commit hook will automatically run checks before each commit."
