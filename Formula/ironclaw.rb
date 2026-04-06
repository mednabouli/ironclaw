# typed: false
# frozen_string_literal: true

# Homebrew formula for IronClaw — ultra-lightweight AI agent framework.
#
# Install:
#   brew tap mednabouli/ironclaw https://github.com/mednabouli/ironclaw
#   brew install ironclaw
#
# Or directly:
#   brew install mednabouli/ironclaw/ironclaw
class Ironclaw < Formula
  desc "Ultra-lightweight AI agent framework — single binary, <5ms startup, WASM plugins"
  homepage "https://github.com/mednabouli/ironclaw"
  version "0.1.1"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mednabouli/ironclaw/releases/download/v#{version}/ironclaw-macos-aarch64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_MACOS_AARCH64"
    else
      url "https://github.com/mednabouli/ironclaw/releases/download/v#{version}/ironclaw-macos-x86_64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_MACOS_X86_64"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      if Hardware::CPU.is_64_bit?
        url "https://github.com/mednabouli/ironclaw/releases/download/v#{version}/ironclaw-linux-aarch64.tar.gz"
        sha256 "PLACEHOLDER_SHA256_LINUX_AARCH64"
      else
        url "https://github.com/mednabouli/ironclaw/releases/download/v#{version}/ironclaw-linux-armv7.tar.gz"
        sha256 "PLACEHOLDER_SHA256_LINUX_ARMV7"
      end
    else
      url "https://github.com/mednabouli/ironclaw/releases/download/v#{version}/ironclaw-linux-x86_64.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_X86_64"
    end
  end

  def install
    bin.install "ironclaw"
  end

  def post_install
    # Generate shell completions
    (bash_completion/"ironclaw").write Utils.safe_popen_read(bin/"ironclaw", "completions", "bash")
    (zsh_completion/"_ironclaw").write Utils.safe_popen_read(bin/"ironclaw", "completions", "zsh")
    (fish_completion/"ironclaw.fish").write Utils.safe_popen_read(bin/"ironclaw", "completions", "fish")
  end

  test do
    assert_match "ironclaw", shell_output("#{bin}/ironclaw --version")
    assert_match "ok", shell_output("#{bin}/ironclaw health 2>&1", 1)
  end
end
