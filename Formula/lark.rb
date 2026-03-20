# Homebrew formula for lark.
#
# This file is maintained in the main repo for version control.
# Copy it to the tap repo (github.com/tfinklea/homebrew-tap) at Formula/lark.rb.
#
# After tagging a release and downloading the tarballs, update the sha256 values:
#   shasum -a 256 lark-v<VERSION>-<TARGET>.tar.gz
class Lark < Formula
  desc "The line to all your tools — a keyboard-driven terminal command palette"
  homepage "https://github.com/tfinklea/larkline"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/tfinklea/larkline/releases/download/v#{version}/lark-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "7fda8691e9ac0c8683041b60ad6f80cf6e2b6ec58fc3f6c7377f527712ab6e66"
    else
      url "https://github.com/tfinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "6d932914a417c7e45ecd259adbd9c4fe5ed23f8e4c029d466e3c9b981900cafc"
    end
  end

  on_linux do
    url "https://github.com/tfinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "c31703971ece6fe9220b7fe3f02d4ede65770972391510dcbe6713b340f99938"
  end

  def install
    bin.install "lark"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/lark --version")
  end
end
