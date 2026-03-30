# Homebrew formula for larkline.
#
# This file is maintained in the main repo for version control.
# Copy it to the tap repo (github.com/TaylorFinklea/homebrew-tap) at Formula/larkline.rb.
#
# Install: brew install TaylorFinklea/tap/larkline
# Binary name: lark
class Larkline < Formula
  desc "The line to all your tools — a keyboard-driven terminal command palette"
  homepage "https://github.com/TaylorFinklea/larkline"
  version "0.3.1"
  license "GPL-3.0-only"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "1b6c1c275e33bb3c722422757232cefad5fc98d6281ada735f8d40b640ebc3f0"
    else
      url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "be538f6d272863accb7c666e25ab182c01537ee12d59d41a38c5ff93a4998c01"
    end
  end

  on_linux do
    url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "43992770965621fab1a3e4ccd44db5fece5ca20ae7980960aa5c58ccc4404906"
  end

  def install
    bin.install "lark"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/lark --version")
  end
end
