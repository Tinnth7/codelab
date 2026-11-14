<p align="center">
<img width="240" height="240" alt="icon" src="https://github.com/user-attachments/assets/1dd6d14a-6375-4504-b781-aef23f1a2b51" />
</p>

<h1 align="center">CodeLab</h1>

<p align="center">
  <b>Code smarter, not harder with the IDE that groups your code.</b>
</p>

CodeLab parses your source code into logical blocks (functions, classes, imports, etc.) using Tree-sitter, allowing you to **navigate, group**, and edit code at a structural level rather than just scrolling through a wall of text.

## Features

- **Block-Based Editing**: Navigate your code by logical structures grouped in a collapsible sidebar.
- **Multi-Language Support**: Full AST parsing powered by Tree-sitter for Python, C, C++, JavaScript, TypeScript, and Rust.
- **KDL / Niri Config Support**: Specialized parsing for `.kdl` files, including a custom Key Capture dialog for Niri window manager binds.
- **Dual Editor Modes**: Toggle seamlessly between a **Block** view (focused editing) and a **Raw** view (full file).
- **Smart Comment Hiding**: Hide full-line comments to compress your view. The editor physically removes the lines from the view but preserves the original line numbers in the gutter.
- **Auto-Save & Backups**: Automatically creates deduplicated snapshots of your file every 5 minutes (up to 50 backups), on save, and on close.
- **Modern UI**: Built with `egui`, featuring a native file explorer, Lucide icon fonts, and Dark/Light mode.

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+S` | Save File |
| `Ctrl+Shift+S` | Save As... |
| `Ctrl+O` | Open File |
| `Ctrl+N` | Create New Block (captures keys if in KDL Binds) |
| `Ctrl+D` | Delete Selected Block |
| `Ctrl+E` | Toggle Disable (comments out the block) |
| `Ctrl+F` | Find / Search Blocks |
| `Ctrl+K` | Capture Key Combo (KDL Binds only) |

## Installation

We support both Windows and Linux!
You can download the latest version from the [Releases page](https://www.github.com/Tinnth7/codelab/releases) or the [Actions page](https://github.com/Tinnth7/codelab/actions) (Make sure the lastest one has a green check on it)

The installer will:
1. Install CodeLab to your `Program Files` directory.
2. Silently install the required Visual C++ Redistributable (if not already present).
3. Add the `codelab` command to your System `PATH`, allowing you to open the editor from any terminal.

## Building from Source

### Prerequisites
- [Rust & Cargo](https://rustup.rs/) (Stable toolchain)
- **Windows Only**: Visual Studio C++ Build Tools (Desktop development with C++)

### Build Steps
```bash
# Clone the repository
git clone https://github.com/Tinnth7/codelab.git

# Build the release binary
cargo build --release
---
