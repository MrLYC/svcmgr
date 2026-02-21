#!/bin/bash
# Pre-commit hook for svcmgr
# Ensures code quality before commits

set -e

echo "🔍 Running pre-commit checks..."

# 1. Check code formatting
echo "📝 Checking code formatting (cargo fmt)..."
if ! cargo fmt --all -- --check; then
    echo "❌ Code formatting check failed!"
    echo "💡 Run 'cargo fmt --all' to fix formatting issues"
    exit 1
fi
echo "✅ Code formatting check passed"

# 2. Run clippy
echo "🔧 Running clippy..."
if ! cargo clippy --all-targets --all-features -- -A dead_code -A unused_imports -A clippy::single_component_path_imports -D warnings 2>&1; then
    echo "❌ Clippy check failed!"
    echo "💡 Fix the warnings above before committing"
    exit 1
fi
echo "✅ Clippy check passed"

# 3. Run tests
echo "🧪 Running tests..."
if ! cargo test --lib --quiet 2>&1; then
    echo "❌ Tests failed!"
    echo "💡 Fix the failing tests before committing"
    exit 1
fi
echo "✅ All tests passed"

echo "✨ All pre-commit checks passed! Proceeding with commit..."
