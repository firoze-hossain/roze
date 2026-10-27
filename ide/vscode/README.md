# Roze Language Support for Visual Studio Code 🌹

Official VS Code marketplace extension for the **Roze Programming Language** (v1.0.0 Stable).

---

## Features

- 🔤 **Syntax Highlighting**: Full grammar coverage for expressions, control flow, ADT enums, traits, generics, unsafe memory, and built-ins.
- ⚡ **Language Server Protocol (LSP)**: Integrated diagnostic reports, error squiggles, syntax validation, and code hover via `roze-lsp`.
- 📦 **Code Snippets**: Quick templates for `func`, `class`, `enum`, `trait`, `impl`, `match`, `unsafe`, and `extern`.
- 🛠️ **Build & Run Integration**: 
  - `Ctrl+Shift+B`: Trigger `roze-pkg build`
  - `Ctrl+Shift+R`: Trigger `roze-pkg run`
- 🎨 **Format on Save**: Automatic code formatting and whitespace trimming.

---

## Getting Started

1. Install the Roze Toolchain:
   ```bash
   curl -fsSL https://roze-lang.org/install.sh | bash
   ```
2. Open any `.roze` file in VS Code.
3. The extension automatically detects `roze-lsp` from your system PATH and activates language services.

---

## Extension Settings

This extension contributes the following settings:

- `roze.compilerPath`: Absolute path to the Roze compiler binary (`roze`).
- `roze.lspEnabled`: Enable/disable Language Server Protocol support (default: `true`).
- `roze.formatOnSave`: Enable/disable formatting on save (default: `true`).

---

## License

MIT License. Copyright (c) 2026 Firoze.
