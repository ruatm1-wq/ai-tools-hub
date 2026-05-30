# 🦾 Universal Skill Converter — 万能 AI Agent Skill 格式转换器

> **跨 14 种 AI Agent 格式的智能 Skill 生命周期管理引擎**  
> **Cross-platform skill format converter & lifecycle manager for 14 AI agents**

[![Python](https://img.shields.io/badge/Python-3.8+-blue?logo=python)](https://python.org)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Agents](https://img.shields.io/badge/agents-14-orange)](#supported-agents)

---

## 📖 简介 / Introduction

**中文**  
Universal Skill Converter 是一个**零依赖、单文件**的 Python 工具，能让你的 AI Agent skill 在 14 种主流格式之间自由转换。无论你是 Cursor 用户、Windsurf 开发者、Cline 爱好者还是 Reasonix 重度用户——同一个 skill 文件，一键同步到所有 Agent。

**English**  
Universal Skill Converter is a **zero-dependency, single-file** Python tool that freely converts AI Agent skills across 14 mainstream formats. Whether you use Cursor, Windsurf, Cline, Claude Code, or Reasonix — one skill file, one command, every agent.

---

## ✨ 核心能力 / Core Features

| Feature | 中文 | English |
|:--------|:-----|:--------|
| **Format Conversion** | 14 种 Agent 格式互相转换 | Convert between 14 agent formats |
| **Auto-Detect** | 根据文件内容和路径自动识别格式 | Auto-detect format from content & path |
| **Dependency Scan** | 检测二进制、环境变量、包依赖 | Scan bins, env vars, package deps |
| **Compatibility Check** | A/C/F 评分 + 字段丢失分析 | A/C/F score + field-loss analysis |
| **Bidirectional Sync** | 双向批量同步多个 Agent | Bi-directional batch sync to multiple agents |
| **Auto Watch** | 监控目录变化自动同步 | Watch directory & auto-sync on changes |
| **Self-Install** | 把自己安装为当前 Agent 的 Skill | Self-install as a skill for any agent |
| **One-Click Bootstrap** | 下载后一条命令完成配置 | Single command setup after download |

---

## 🚀 快速开始 / Quick Start

### 下载 / Download

```bash
# 直接下载单文件
# Download the single file
curl -O https://raw.githubusercontent.com/ruatm1-wq/ai-tools-hub/main/skills/universal-skill-converter/universal-skill-converter.py

# 或从项目克隆
# Or clone the repo
git clone https://github.com/ruatm1-wq/ai-tools-hub.git
```

### 一键引导 / One-Click Bootstrap

```bash
python universal-skill-converter.py --bootstrap
```

引导流程：检测当前项目环境中的 Agent → 自安装为 Skill → 输出使用说明  
Bootstrap: detect agents in your project → self-install as a skill → show usage guide

### 基础用法 / Basic Usage

```bash
# 安装 skill（自动检测目标环境）
# Install skill (auto-detect target agents)
python universal-skill-converter.py install my-skill.md

# 检测兼容性和依赖
# Check compatibility & dependencies
python universal-skill-converter.py check my-skill.md

# 查看 skill 信息
# Inspect skill info
python universal-skill-converter.py inspect my-skill.md

# 列出已安装技能
# List installed skills
python universal-skill-converter.py list
```

---

## 📋 命令参考 / Command Reference

### 完整命令表 / All Commands

| Command | 中文说明 | English | Example |
|:--------|:---------|:--------|:--------|
| `install` | 安装 skill 到指定 Agent | Install skill to agents | `install foo.md --to cursor,cline` |
| `inspect` | 查看 skill 详细信息 | View skill details | `inspect foo.md` |
| `check` | 检测兼容性 + 依赖评分 | Check compatibility & deps | `check foo.md --json` |
| `list` | 列出已安装技能 | List installed skills | `list --agent windsurf` |
| `convert` | 导出为指定格式到 stdout | Export to format (stdout) | `convert foo.md --to openclaw` |
| `install-dir` | 批量安装整个目录 | Batch install directory | `install-dir ./skills/` |
| `sync` | 双向同步到多个 Agent | Bi-directional sync | `sync --from ./skills/ --to all` |
| `watch` | 监控目录自动同步 | Watch & auto-sync | `watch ./skills/ --to reasonix,cursor` |
| `config` | 配置管理 | Config management | `config show` |
| `--detect-agent` | 检测当前 Agent 环境 | Detect agent environment | `--detect-agent` |
| `--self-install` | 自安装为当前 Agent 的 skill | Self-install as skill | `--self-install` |
| `--bootstrap` | ⭐ 一键引导 | One-click bootstrap | `--bootstrap` |

### 通用参数 / Common Flags

| Flag | 中文说明 | English |
|:-----|:---------|:--------|
| `--to <a,b>` | 目标 Agent 列表 | Target agent list |
| `--dry-run`, `-n` | 预览不写入 | Preview without writing |
| `--backup`, `-b` | 写入前备份 | Backup before writing |
| `--json`, `-j` | JSON 机器可读输出 | Machine-readable JSON output |

---

## 🤖 支持的 Agent / Supported Agents

| ID | Agent Name | Format | File Location |
|:---|:-----------|:-------|:-------------|
| `reasonix` | Reasonix Code | 单文件 `.md` | `.reasonix/skills/<name>.md` |
| `hermes` | Hermes Agent | 子目录 + SKILL.md | `.hermes/skills/<name>/SKILL.md` |
| `opencode` | OpenCode | 子目录 + SKILL.md | `.config/opencode/skills/<name>/SKILL.md` |
| `claude-code` | Claude Code (Anthropic) | 子目录 + plugin.json + SKILL.md | `.claude/plugins/<name>/SKILL.md` |
| `openai-codex` | OpenAI Codex | 子目录 + plugin.json + SKILL.md | `.agents/plugins/<name>/SKILL.md` |
| `cursor` | Cursor | 单文件 `.mdc` | `.cursor/rules/<name>.mdc` |
| `windsurf` | Windsurf (Codeium) | 单文件 `.windsurfrules` | `.windsurfrules` |
| `cline` | Cline | 纯 markdown `.clinerules` | `.clinerules` |
| `roo-code` | Roo Code | 纯 markdown (同 Cline) | `.clinerules` + `.roomodes` |
| `github-copilot` | GitHub Copilot | 单文件 | `.github/copilot-instructions.md` |
| `continue` | Continue.dev | JSON 配置 | `~/.continue/config.json` |
| `aider` | Aider | CONVENTIONS.md | `CONVENTIONS.md` |
| `skills-cli` | Skills CLI | 子目录 + SKILL.md | `.skills/<name>/SKILL.md` |
| `openclaw` | OpenClaw | 子目录 + SKILL.md + metadata JSON | `~/.openclaw/workspace/skills/<name>/SKILL.md` |

---

## 💡 实战场景 / Use Cases

### 场景 1：从网上下载一个 skill，装到所有 Agent

```bash
# 下载 → 自动检测格式 → 批量安装
python universal-skill-converter.py install ~/Downloads/research-skill.md --to all
```

### 场景 2：把 Cursor 的规则共享给 Cline 和 Windsurf

```bash
python universal-skill-converter.py convert .cursor/rules/coding.mdc --to cline
# → 输出 Cline 格式的 .clinerules 内容
```

### 场景 3：同步整个技能库到多个 Agent

```bash
python universal-skill-converter.py sync --from ./my-skills/ --to reasonix,cursor,openclaw
```

### 场景 4：每天自动同步

```bash
# 后台运行监控
python universal-skill-converter.py watch ./skills/ --to all --daemon
```

### 场景 5：作为 Agent 的 Skill 使用（自安装后）

```
# 在 Reasonix Code 中：
run_skill("skill-converter", "install foo.md --to cursor,windsurf")

# 在 Cursor / Windsurf / Cline 中：
# 直接运行命令行
```

---

## ⚙️ 配置 / Configuration

配置文件位置：`~/.skill-converter/config.json`

Config file location: `~/.skill-converter/config.json`

```json
{
  "default_targets": ["reasonix"],
  "watch_dirs": [],
  "backup_enabled": true,
  "poll_interval": 2.0,
  "path_overrides": {}
}
```

可通过 `config` 子命令管理：

```bash
# 查看配置 / View config
python universal-skill-converter.py config show

# 设置默认目标 / Set default targets
python universal-skill-converter.py config set default_targets reasonix,cursor,windsurf

# 覆盖 Agent 路径 / Override agent path
python universal-skill-converter.py config path reasonix D:/my-custom-path/skills
```

---

## 🏗️ 架构 / Architecture

```
                    ┌─────────────────┐
                    │   Input File     │
                    │  (.md/.mdc/.json)│
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │  detect_format()  │ ← 文件名 / 扩展名 / Frontmatter 关键词
                    └────────┬─────────┘
                             │
                    ┌────────▼─────────┐
                    │   parse_skill()   │ → 统一中间字典
                    └────────┬─────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
     ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
     │check_compat │ │detect_dep   │ │ render_skill │
     │ ibility()   │ │endencies()  │ │   → 14 fmt  │
     │ A/C/F score │ │bins/env/pkg │ │   → write   │
     └─────────────┘ └─────────────┘ └─────────────┘
```

---

## 📦 零依赖 / Zero Dependencies

整个工具是 **单文件 Python 3（无第三方库）**。仅 `watch` 命令会尝试导入 `watchdog`（可选加速），不存在时自动降级为 polling 模式。

The entire tool is a **single Python 3 file (no third-party libraries)**. Only the `watch` command optionally imports `watchdog` for faster file monitoring, falling back to polling mode automatically.

---

## 🧪 开发 / Development

```bash
# 验证括号平衡
node -e "
const c = require('fs').readFileSync('universal-skill-converter.py','utf8');
let s = c.replace(/(['\"])(?:(?!\1|\\\\).|\\\\.)*\1/g,'').replace(/#.*$/gm,'');
console.log('{}:', (s.match(/\{/g)||[]).length - (s.match(/\}/g)||[]).length);
console.log('():', (s.match(/\(/g)||[]).length - (s.match(/\)/g)||[]).length);
"

# 测试（需要 Python 环境）
python -c "from universal_skill_converter import *; print('OK')"
```

---

## 📄 License

MIT License — 自由使用、修改、分发。  
MIT License — Free to use, modify, and distribute.

---

## 🌟 Star History

如果这个工具对你有帮助，请给个 ⭐！  
If this tool helps you, please give it a ⭐!

---

*Made with ❤️ for the AI Agent community*
