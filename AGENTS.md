# Agent Guide: os_dev_specs

This document provides a high-level guide for AI agents and developers on the project's architecture, development workflow, and core principles.

## 1. Project Purpose
`os_dev_specs` is a data-processing tool designed to manage a library of OS development specifications. It parses a Markdown-formatted input file containing specification metadata and generates structured HTML indexes and a synchronized local PDF library.

## 2. Input/Output Model
- **Input**: A Markdown file (default: `data.md`) containing specification IDs, source URLs (PDF or ZIP), and specific page references.
- **Output Artifacts**:
    - **HTML Indexes**: `index.html` (local use) and `docs/index.html` (public use).
    - **Download Script**: `download_entries.generated.sh` for legacy compatibility.
    - **PDF Library**: A synchronized `spec/` directory containing the processed PDFs.

## 3. CLI Architecture
All operations are centralized in a single Rust binary using a subcommand-driven interface:
- **Build (Default)**: `cargo run [PATH]`. Parses the input data and generates all HTML and script artifacts.
- **Download**: `cargo run -- download`. Synchronizes the local `spec/` directory with remote sources defined in the input data.
- **Watch**: `cargo run -- watch`. Monitors the input file for changes and automatically triggers a rebuild.

## 4. Development Principles
- **Tooling Standards**:
    - **Stable Rust**: Use only stable Rust features.
    - **System Integration**: Prefer invoking proven system tools (`wget`, `unzip`, `sha1sum`, `diff`) via `std::process::Command` for networking and IO to keep the binary lightweight.
- **Robustness**:
    - **Resilient Downloads**: Individual download failures should not stop the batch process; report failures as a summary at the end.
    - **Loop Prevention**: Implement content-checking and cooldown periods when watching files that the program also modifies.
    - **Atomic Save Support**: Monitor the parent directory of the input file to ensure compatibility with editors using atomic save strategies.
- **Code Hygiene**:
    - Maintain clean git diffs by avoiding grouped imports in `use` statements.
    - Regularly run `cargo clippy`, `cargo fmt`, and `cargo test` to ensure code quality.

## 5. Instructions for Future Agents
- Focus changes on improving the processing logic, CLI interface, or the robustness of the I/O operations.
- Ensure any new functionality is reflected in the subcommand structure.
- Always verify that changes do not break the "Build -> Download -> Watch" workflow.
