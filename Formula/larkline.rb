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
  version "0.15.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    else
      url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    url "https://github.com/TaylorFinklea/larkline/releases/download/v#{version}/lark-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "PLACEHOLDER"
  end

  def install
    bin.install "lark"
    # macOS bundle includes the EventKit helper for the Calendar plugin.
    # Linux tarball does not contain it; calendar plugin falls back to icalbuddy.
    bin.install "larkline-macos-helper" if OS.mac?
  end

  def caveats
    <<~EOS
      Run `lark plugin sync` to install the standard plugin library.
      On macOS, the Calendar plugin uses larkline-macos-helper for rich event
      data. macOS will prompt for Calendar access on first use; grant via
      System Settings → Privacy & Security → Calendars.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/lark --version")
  end
end
