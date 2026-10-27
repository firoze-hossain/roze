# 🌹 Roze v1.0.0 Installation Guide

Welcome to the Roze Programming Language official installation guide. Roze supports Linux, macOS, and Windows with precompiled native binaries, package manager integrations, and standalone installers.

---

## 1. Quickstart: One-Line Installers

### Linux & macOS
Run the universal installation script in your terminal:
```bash
curl -fsSL https://roze-lang.org/install.sh | bash
```
Then reload your shell:
```bash
source ~/.bashrc   # or source ~/.zshrc
```

### Windows (PowerShell)
Run the PowerShell installer in Windows Terminal:
```powershell
iwr -useb https://roze-lang.org/install.ps1 | iex
```

---

## 2. Package Managers

### Debian / Ubuntu (`.deb`)
Download and install the native Debian package:
```bash
# Download latest v1.0.0 .deb
curl -LO https://roze-lang.org/dist/roze_1.0.0_amd64.deb

# Install package
sudo dpkg -i roze_1.0.0_amd64.deb

# Verify
roze --version
```

### Fedora / RHEL / CentOS (`.rpm`)
Download and install the RPM package:
```bash
sudo rpm -ivh https://roze-lang.org/dist/roze-1.0.0-1.x86_64.rpm
```

### macOS (Homebrew)
Install Roze via Homebrew tap:
```bash
brew tap firoze/roze
brew install roze
```

### Windows (MSI & Setup Wizard)
- **Installer Wizard (`.exe`)**: Download [Roze-Setup-1.0.0.exe](https://roze-lang.org/dist/Roze-Setup-1.0.0.exe) and follow the GUI installation wizard.
- **Enterprise MSI (`.msi`)**: Deploy silently via msiexec:
  ```cmd
  msiexec /i roze-1.0.0-x64.msi /quiet
  ```

---

## 3. Building From Source

Prerequisites:
- Rust toolchain (`rustc` >= 1.75.0, `cargo`)
- C compiler (`gcc` or `clang`)
- Java Development Kit (JDK 17+) for JVM backend target

```bash
# Clone the repository
git clone https://github.com/firoze/roze.git
cd roze

# Build all release binaries
cargo build --release --workspace

# The binaries are located at target/release/:
#   - target/release/roze         (Main compiler)
#   - target/release/roze-pkg     (Package manager & dependency resolver)
#   - target/release/roze-lsp     (Language Server Protocol daemon)
#   - target/release/roze-build   (Build orchestrator)
```

---

## 4. Verifying Your Installation

Confirm that all toolchain components are available on your `PATH`:

```bash
$ roze --version
🌹 Roze Compiler v1.0.0

$ roze-pkg --version
roze-pkg 1.0.0

$ roze-lsp --version
roze-lsp 1.0.0
```

Compile and run your first Roze program:
```bash
# Create hello.roze
cat << 'EOF' > hello.roze
func main() {
    println("Hello from Roze v1.0.0! 🌹");
}
EOF

# Compile to native standalone executable:
roze build hello.roze --target native

# Run native executable:
./hello
Hello from Roze v1.0.0! 🌹
```

---

## 5. VS Code Extension

Install the official **Roze Language** extension from the Visual Studio Marketplace or via `.vsix`:
```bash
code --install-extension ide/vscode/roze-language-1.0.0.vsix
```
Features include auto-formatting on save, syntax highlighting for all types/keywords, error squiggles from `roze-lsp`, and `Ctrl+Shift+B` project builds.
