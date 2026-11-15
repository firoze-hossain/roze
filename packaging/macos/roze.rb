# typed: false
# frozen_string_literal: true

class Roze < Formula
  desc "The Roze Programming Language Toolchain (v2.0.0 'Prometheus')"
  homepage "https://roze-lang.org"
  url "https://github.com/firoze-hossain/roze/archive/refs/tags/v2.0.0.tar.gz"
  sha256 "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  license "MIT"
  head "https://github.com/firoze-hossain/roze.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--workspace"

    bin.install "target/release/roze"
    bin.install "target/release/bsr"
    bin.install "target/release/roze-build"
    bin.install "target/release/roze-pkg"
    bin.install "target/release/roze-lsp"
    bin.install "target/release/roze-bindgen"
    bin.install "target/release/roze-svd"
    bin.install "target/release/rozedoc"

    if File.directory?("stdlib/src")
      (pkgshare/"stdlib").install Dir["stdlib/src/*"]
    else
      (pkgshare/"stdlib").install Dir["stdlib/*"]
    end
  end

  def caveats
    <<~EOS
      Set the ROZE_STDLIB_PATH environment variable in your shell profile:
        export ROZE_STDLIB_PATH="#{opt_pkgshare}/stdlib"
    EOS
  end

  test do
    (testpath/"hello.roze").write <<~EOS
      func main() {
          println("Hello from Roze v2.0 'Prometheus' Homebrew!");
      }
    EOS

    system "#{bin}/roze", "build", "hello.roze", "--target", "native"
    assert_equal "Hello from Roze v2.0 'Prometheus' Homebrew!\n", shell_output("./hello")
  end
end
