#!/usr/bin/env bash
# Setup script for cigen development environment

set -e

echo "🚀 Setting up cigen development environment..."

# Check if Rust is installed
if ! command -v rustc &> /dev/null; then
    echo "❌ Rust is not installed. Installing:"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
fi

echo "✅ Rust is installed ($(rustc --version))"

# Install Lefthook
if ! command -v lefthook &> /dev/null; then
    echo "📦 Installing Lefthook..."
    
    # Check if homebrew is available
    if command -v brew &> /dev/null; then
        echo "  Using Homebrew..."
        brew install lefthook
    else
        echo "❌ Homebrew is not installed. Please install Homebrew first:"
        echo "   /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
        echo ""
        echo "Or install Lefthook manually:"
        echo "   npm install -g lefthook"
        echo "   or"
        echo "   curl -sSfL https://raw.githubusercontent.com/evilmartians/lefthook/master/install.sh | sh"
        exit 1
    fi
    
    echo "✅ Lefthook installed successfully"
else
    echo "✅ Lefthook is already installed ($(lefthook version))"
fi

# Install actionlint
if ! command -v actionlint &> /dev/null; then
    echo "📦 Installing actionlint (GitHub Actions linter)..."
    
    # Check if homebrew is available
    if command -v brew &> /dev/null; then
        echo "  Using Homebrew..."
        brew install actionlint
    elif command -v go &> /dev/null; then
        echo "  Using Go..."
        go install github.com/rhysd/actionlint/cmd/actionlint@latest
    else
        echo "❌ Neither Homebrew nor Go is available. Please install actionlint manually:"
        echo "   brew install actionlint"
        echo "   or"
        echo "   go install github.com/rhysd/actionlint/cmd/actionlint@latest"
        echo ""
        echo "Note: actionlint is optional but recommended for linting GitHub Actions workflows"
    fi
    
    if command -v actionlint &> /dev/null; then
        echo "✅ actionlint installed successfully"
    fi
else
    echo "✅ actionlint is already installed ($(actionlint -version 2>&1 | head -n1))"
fi

# Install Lefthook git hooks
echo "🔗 Installing git hooks..."
lefthook install

# Run initial checks
echo "🧪 Running initial checks..."
echo "  Checking code format..."
if ! cargo fmt --all -- --check; then
    echo "❌ Code formatting issues found. Run 'cargo fmt' to fix."
    exit 1
fi

echo "  Running clippy..."
if ! cargo clippy; then
    echo "❌ Clippy found issues."
    exit 1
fi

echo "  Running tests..."
if ! cargo test; then
    echo "❌ Tests failed."
    exit 1
fi

echo ""
echo "✨ Setup complete! You're ready to start developing."
echo ""
echo "📝 Git hooks are now active:"
echo "  - pre-commit: Runs fmt, clippy, and tests"
echo "  - pre-push: Runs full checks"
echo ""
echo "💡 To skip hooks temporarily, use: git commit --no-verify"
