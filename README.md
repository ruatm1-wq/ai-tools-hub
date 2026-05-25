<p align="center">
  <img src="./public/icons.svg" width="80" height="80" alt="AI Tools Hub" />
</p>

<h1 align="center">AI Tools Hub</h1>

<p align="center">
  <strong>多 AI 工具统一管理桌面平台</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.83+-orange?logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Tauri-2.0+-purple?logo=tauri" alt="Tauri" />
  <img src="https://img.shields.io/badge/React-19+-blue?logo=react" alt="React" />
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License" />
</p>

---

## Overview

AI Tools Hub is a **lightweight AI agent desktop platform** that unifies multiple AI tools into a single interface. Instead of building AI from scratch, it manages existing AIs — API models (DeepSeek, OpenAI, GLM) and CLI agents (Reasonix Code, Hermes Agent, OpenCode).

### What it does

- **Unified chat** — Talk to any AI tool in one window
- **PTY process management** — CLI tools run persistently via pseudo-terminal
- **Memory engine** — SQLite-backed fact memory with cross-session search
- **Knowledge base** — Auto-index Markdown files from your vault
- **Web search** — Three-tier fallback: Chrome CDP → DuckDuckGo → Google
- **Skill system** — Install/attach reusable skill bundles
- **Agent orchestration** — Sub-agent creation and context relay
- **Security sandbox** — PathGuard access control system
- **Hub service** — HTTP/WebSocket backend on port 27125
- **Telegram bridge** — Chat with AI via Telegram Bot

## Architecture

```
┌──────────────────────────────────────────────────┐
│              AI Tools Hub Desktop                │
├──────────────────────────────────────────────────┤
│  🖥️  Frontend (React + TailwindCSS)              │
│  ┌─────────┬──────────────────┬──────────┐       │
│  │ Sidebar │    Chat Area     │  Status  │       │
│  │ Tools   │    Streaming     │  Memory  │       │
│  │ History │    Memory Inject │  Preview │       │
│  │ Skills  │    Search Inject │          │       │
│  │ Terminal│                  │          │       │
│  └─────────┴──────────────────┴──────────┘       │
├──────────────────────────────────────────────────┤
│  🦀  Backend (Rust)                              │
│  ┌──────┬──────┬──────┬──────┬──────┬──────┐    │
│  │Memory│ PTY  │ Hub  │ Know-│Skill │Sand- │    │
│  │Engine│Mgr   │HTTP  │ledge │Bundle│box   │    │
│  ├──────┼──────┼──────┼──────┼──────┼──────┤    │
│  │Search│Chrome│Sub-  │Cron  │Person│CLI   │    │
│  │      │CDP   │Agent │Sched │ality │Mode  │    │
│  └──────┴──────┴──────┴──────┴──────┴──────┘    │
├──────────────────────────────────────────────────┤
│  🌐  Hub Service (port 27125)                     │
│  └── /health · /api/chat · /webhook              │
└──────────────────────────────────────────────────┘
```

## Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Desktop | **Tauri 2.0** | Cross-platform desktop shell |
| Backend | **Rust** | Core logic, API calls, process mgmt |
| Frontend | **React 19 + TypeScript** | User interface |
| Styling | **TailwindCSS v4** | UI framework |
| Icons | **Lucide React** | Icon library |
| HTTP | **reqwest** | API client |
| Database | **rusqlite (SQLite)** | Memory storage |
| PTY | **portable-pty** | Pseudo-terminal for CLI tools |
| Async | **tokio** | Async runtime |
| Build | **Vite** | Frontend bundler |

## Quick Start

### Development

```bash
# Prerequisites
# - Rust 1.83+
# - Node.js 18+
# - pnpm (or npm)

# Install frontend dependencies
pnpm install

# Run in development mode (hot reload)
pnpm tauri dev
```

### Build

```bash
pnpm tauri build
# Output:
#   src-tauri/target/release/ai-tools-hub.exe
#   src-tauri/target/release/bundle/nsis/...setup.exe
```

### CLI Mode

```bash
# Chat without GUI
ai-tools-hub.exe --cli
# Requires DEEPSEEK_API_KEY environment variable
```

## Configuration

Configuration files are stored at `%APPDATA%/ai-tools-hub/`:

| File | Description |
|------|-------------|
| `tools.json` | Tool definitions (API keys, commands) |
| `hub.json` | Hub settings (Telegram token, vault path) |
| `sessions/` | Chat session history |
| `knowledge/` | Indexed knowledge documents |
| `memory/memory.db` | SQLite memory database |
| `skills/` | Installed skill bundles |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DEEPSEEK_API_KEY` | — | DeepSeek API key |
| `DEEPSEEK_API_BASE` | `https://api.deepseek.com` | API base URL |
| `AI_HUB_VAULT` | `D:\我的工作台` | Vault directory for knowledge base |

## Adding Tools

### API Tools (DeepSeek / OpenAI / GLM)

| Field | Example |
|-------|---------|
| Name | `DeepSeek` |
| Type | `API` |
| API URL | `https://api.deepseek.com` |
| Model | `deepseek-chat` |
| API Key | `sk-...` |

### CLI Tools (Reasonix / Hermes / OpenCode)

| Field | Example |
|-------|---------|
| Name | `小拉` |
| Type | `CLI` |
| Command | `cmd.exe` |
| Args | `/c, chcp 65001 >nul && npx reasonix run` |

Or use the built-in **Terminal** to enter commands directly — the app starts a PTY session and lets you test before saving.

## Features

### Streaming
Real-time SSE streaming from API models, with per-token display.

### Memory
Conversations are automatically saved as facts to SQLite. Related memories are injected into prompts for context.

### Knowledge Base
Recursively indexes `D:\我的工作台` (configurable) for all `.md` files. Matching documents are injected into AI prompts during chat.

### Web Search
Three-tier fallback search:
1. Chrome CDP (requires `--remote-debugging-port=9222`)
2. DuckDuckGo Instant Answer
3. Google HTML scraping

### Skills
JSON-based skill bundles that attach reusable prompts. Skills can be installed, listed, and attached per session.

### Hub HTTP Service
Runs on port 27125 (auto-fallback 27126, 27127). Provides REST API for chat and health checks.

## Security

- PathGuard sandbox restricts file operations to allowed directories
- API keys are stored in `%APPDATA%` (outside the repo)
- Filename sanitization prevents path traversal in desk notes
- CLI tools run with `CREATE_NO_WINDOW` flag on Windows

## Development

```bash
# Install dependencies
pnpm install

# Dev mode (hot reload)
pnpm tauri dev

# Build release
pnpm tauri build

# Type check
pnpm tsc --noEmit

# Lint
pnpm lint
```

## License

MIT
