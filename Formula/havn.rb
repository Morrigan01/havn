class Havn < Formula
  desc "Map local ports to project directories — CLI, dashboard, and MCP server"
  homepage "https://github.com/Morrigan01/havn"
  version "0.3.0"
  license "BSL-1.1"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Morrigan01/havn/releases/download/v0.3.0/havn-aarch64-apple-darwin"
      sha256 "REPLACE_WITH_SHA256"
    else
      url "https://github.com/Morrigan01/havn/releases/download/v0.3.0/havn-x86_64-apple-darwin"
      sha256 "REPLACE_WITH_SHA256"
    end
  end

  on_linux do
    url "https://github.com/Morrigan01/havn/releases/download/v0.3.0/havn-x86_64-unknown-linux-gnu"
    sha256 "REPLACE_WITH_SHA256"
  end

  def install
    binary = Dir["havn-*"].first || "havn"
    bin.install binary => "havn"
  end

  def caveats
    <<~EOS
      To use havn as an MCP server (e.g. with Claude Code), add it to your
      MCP configuration:

        {
          "mcpServers": {
            "havn": {
              "command": "#{opt_bin}/havn",
              "args": ["mcp"]
            }
          }
        }

      Start the dashboard with:
        havn

      See https://github.com/Morrigan01/havn for full documentation.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/havn --version")
  end
end
