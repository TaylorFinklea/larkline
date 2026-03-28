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
  version "0.2.0"
  license "GPL-3.0-only"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/tfinklea/larkline/releases/download/v#{version}/lark-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "38482cd5666cf4aeb920d04fbfbcd4afed5bd6b9a0af32b2006445b96e40bfe7"
    else
      url "https://github.com/tfinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "b3c4f3e6c417f18ac06b73c58a50621edcd8c58b14a2921d95f24c86f84bae94"
    end
  end

  on_linux do
    url "https://github.com/tfinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "1fc0ed7fa838a910f9328d34eda8e2a3bb76fa5ddeec8e080bc43656d2f64453"
  end

  def install
    bin.install "lark"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/lark --version")
  end
end
