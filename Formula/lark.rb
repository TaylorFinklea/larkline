# Homebrew formula for lark.
#
# This file is maintained in the main repo for version control.
# Copy it to the tap repo (github.com/TaylorFinklea/homebrew-tap) at Formula/lark.rb.
#
# After tagging a release and downloading the tarballs, update the sha256 values:
#   shasum -a 256 lark-v<VERSION>-<TARGET>.tar.gz
class Lark < Formula
  desc "The line to all your tools — a keyboard-driven terminal command palette"
  homepage "https://github.com/TaylorFinklea/larkline"
  version "0.3.0"
  license "GPL-3.0-only"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "8e4ae52f4ecb5b96314e5a077d3be55fcaaca80830e00b34243321e793e730a6"
    else
      url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0d4ff0423ce712b69bc49a10e20ea4383b14faced246bbbbbaa01b7c09c3762c"
    end
  end

  on_linux do
    url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "b5f7b17c530dd606dd8708ac3d52a585554fd3e11f4cb9b5d798d445795b6e3b"
  end

  def install
    bin.install "lark"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/lark --version")
  end
end
