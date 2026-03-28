# havn Homebrew Formula

This directory contains the Homebrew formula for [havn](https://github.com/Morrigan01/havn).

## Installation

Once the Homebrew tap is set up, install with:

```
brew install Morrigan01/tap/havn
```

### Alternative installation methods

**Download a prebuilt binary** from [GitHub Releases](https://github.com/Morrigan01/havn/releases):

```
# macOS (Apple Silicon)
curl -L https://github.com/Morrigan01/havn/releases/latest/download/havn-aarch64-apple-darwin -o havn
chmod +x havn && sudo mv havn /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/Morrigan01/havn/releases/latest/download/havn-x86_64-apple-darwin -o havn
chmod +x havn && sudo mv havn /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/Morrigan01/havn/releases/latest/download/havn-x86_64-unknown-linux-gnu -o havn
chmod +x havn && sudo mv havn /usr/local/bin/
```

**Install from source** via Cargo:

```
cargo install --git https://github.com/Morrigan01/havn
```

## Setting up the Homebrew tap

To publish this formula via `brew install Morrigan01/tap/havn`, create a repository named `homebrew-tap` under the `Morrigan01` GitHub account and copy `havn.rb` into its root or `Formula/` directory. Homebrew resolves `Morrigan01/tap` to `https://github.com/Morrigan01/homebrew-tap` automatically.

## Updating the formula

After creating a new release:

1. Build binaries for all three targets.
2. Compute SHA-256 checksums: `shasum -a 256 havn-*`
3. Update the `sha256` values and `version` in `havn.rb`.
