# typed: false
# frozen_string_literal: true

class Roze < Formula
  desc "The Roze Programming Language Toolchain"
  homepage "https://roze-lang.org"
  url "https://github.com/firoze-hossain/roze/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  license "MIT"
  head "https://github.com/firoze-hossain/roze.git", branch: "master"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--workspace"

    bin.install "target/release/roze"
    bin.install "target/release/roze-build"
    bin.install "target/release/roze-pkg"
    bin.install "target/release/roze-lsp"

    (pkgshare/"stdlib").install Dir["stdlib/*"]
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
          println("Hello from Roze Homebrew!");
      }
    EOS

    system "#{bin}/roze", "build", "hello.roze", "--target", "native"
    assert_equal "Hello from Roze Homebrew!\n", shell_output("./hello")
  end
end
