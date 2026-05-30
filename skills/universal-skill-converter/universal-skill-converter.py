#!/usr/bin/env python3
"""
⭐ Universal Skill Converter v3.0 — AI Agent 智能 Skill 生命周期管理引擎
Agent 通过 run_command 调用：安装/检测/同步/监控/转换
"""

import os, re, json, sys, shutil, time, glob as globmod
from pathlib import Path
from datetime import datetime

VERSION = '3.5.0'

# ═══════════════════════════════════════════════════════════
# 格式注册表 — 所有支持的 AI Agent 格式
# ═══════════════════════════════════════════════════════════

AGENTS = {
    'reasonix': {
        'name': 'Reasonix Code',
        'format': '单文件 .md',
        'path': '.reasonix/skills/<name>.md',
        'dir_needed': False, 'file_name': '{name}.md',
        'fields': ['name', 'description', 'runAs', 'allowed-tools', 'model'],
        'defaults': {'runAs': 'inline', 'allowed-tools': '', 'model': ''},
    },
    'hermes': {
        'name': 'Hermes Agent',
        'format': '子目录 + SKILL.md',
        'path': '.hermes/skills/<name>/SKILL.md',
        'dir_needed': True, 'file_name': 'SKILL.md',
        'fields': ['name', 'description'],
        'defaults': {},
    },
    'opencode': {
        'name': 'OpenCode',
        'format': '子目录 + SKILL.md',
        'path': '.config/opencode/skills/<name>/SKILL.md',
        'dir_needed': True, 'file_name': 'SKILL.md',
        'fields': ['name', 'description'],
        'defaults': {},
    },
    'claude-code': {
        'name': 'Claude Code',
        'format': '子目录 + plugin.json + SKILL.md',
        'path': '.claude/plugins/<name>/SKILL.md',
        'dir_needed': True, 'file_name': 'SKILL.md',
        'fields': ['name', 'description'],
        'defaults': {},
        'extra_files': {
            'plugin.json': '{"name":"{name}","description":"{desc}","skills":["{name}"],"source":"./","strict":false}',
        },
    },
    'openai-codex': {
        'name': 'OpenAI Codex',
        'format': '子目录 + plugin.json + SKILL.md',
        'path': '.agents/plugins/<name>/SKILL.md',
        'dir_needed': True, 'file_name': 'SKILL.md',
        'fields': ['name', 'description'],
        'defaults': {},
        'extra_files': {
            'plugin.json': '{"name":"{name}","description":"{desc}","skills":["{name}"],"source":"./","strict":false}',
        },
    },
    'cursor': {
        'name': 'Cursor',
        'format': '单文件 .mdc',
        'path': '.cursor/rules/<name>.mdc',
        'dir_needed': False, 'file_name': '{name}.mdc',
        'fields': ['name', 'description'],
        'defaults': {},
        'template': '---\ndescription: {desc}\nglobs: **/*\n---\n\n{body}',
    },
    'github-copilot': {
        'name': 'GitHub Copilot',
        'format': '单文件 copilot-instructions.md',
        'path': '.github/copilot-instructions.md',
        'dir_needed': False, 'file_name': 'copilot-instructions.md',
        'fields': [],
        'defaults': {},
        'template': '# {name}\n\n{desc}\n\n{body}',
    },
    'continue': {
        'name': 'Continue.dev',
        'format': 'JSON 配置',
        'path': '~/.continue/config.json',
        'dir_needed': False, 'file_name': 'config.json',
        'fields': [],
        'defaults': {},
        'is_json': True,
    },
    'aider': {
        'name': 'Aider',
        'format': 'CONVENTIONS.md',
        'path': 'CONVENTIONS.md',
        'dir_needed': False, 'file_name': 'CONVENTIONS.md',
        'fields': [],
        'defaults': {},
        'template': '# {name}\n\n{desc}\n\n{body}',
    },
    'skills-cli': {
        'name': 'Skills CLI',
        'format': '子目录 + SKILL.md',
        'path': '.skills/<name>/SKILL.md',
        'dir_needed': True, 'file_name': 'SKILL.md',
        'fields': ['name', 'description'],
        'defaults': {},
    },
    'openclaw': {
        'name': 'OpenClaw',
        'format': '子目录 + SKILL.md (含 metadata JSON)',
        'path': 'workspace/skills/<name>/SKILL.md',
        'dir_needed': True, 'file_name': 'SKILL.md',
        'fields': ['name', 'description', 'homepage', 'user-invocable',
                   'disable-model-invocation', 'command-dispatch', 'command-tool',
                   'command-arg-mode', 'metadata'],
        'defaults': {},
        'extra_files': {},
    },
    'windsurf': {
        'name': 'Windsurf (Codeium)',
        'format': '单文件 .windsurfrules',
        'path': '.windsurfrules',
        'dir_needed': False, 'file_name': '.windsurfrules',
        'fields': ['description', 'globs', 'tags'],
        'defaults': {},
        'template': '---\ndescription: {desc}\nglobs: {globs}\n---\n\n{body}',
    },
    'cline': {
        'name': 'Cline',
        'format': '单文件 .clinerules 或目录',
        'path': '.clinerules',
        'dir_needed': False, 'file_name': '.clinerules',
        'fields': [],
        'defaults': {},
        'template': '# {name}\n\n{desc}\n\n{body}',
        'is_plain_md': True,
    },
    'roo-code': {
        'name': 'Roo Code',
        'format': '单文件 .clinerules (同 Cline)',
        'path': '.clinerules',
        'dir_needed': False, 'file_name': '.clinerules',
        'fields': [],
        'defaults': {},
        'template': '# {name}\n\n{desc}\n\n{body}',
        'is_plain_md': True,
    },
}

# ═══════════════════════════════════════════════════════════
# 持久化配置
# ═══════════════════════════════════════════════════════════

CONFIG_DIR = Path.home() / '.skill-converter'
CONFIG_PATH = CONFIG_DIR / 'config.json'
LOCK_PATH = CONFIG_DIR / 'watch.lock'

DEFAULT_CONFIG = {
    'default_targets': ['reasonix'],
    'watch_dirs': [],
    'path_overrides': {},
    'backup_enabled': True,
    'poll_interval': 2.0,
}


def load_config() -> dict:
    if CONFIG_PATH.exists():
        try:
            return {**DEFAULT_CONFIG, **json.loads(CONFIG_PATH.read_text('utf-8'))}
        except:
            pass
    return dict(DEFAULT_CONFIG)


def save_config(cfg: dict):
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    CONFIG_PATH.write_text(json.dumps(cfg, ensure_ascii=False, indent=2), 'utf-8')


# ═══════════════════════════════════════════════════════════
# 核心引擎
# ═══════════════════════════════════════════════════════════

def detect_format(text: str, source_path: str = '') -> str:
    """自动检测 skill 格式（增强版：支持目录结构 + 置信度）"""
    p = source_path.lower()

    # 1) 文件名/扩展名检测
    if '.windsurfrules' in p: return 'windsurf'
    if '.clinerules' in p:
        parent_dir = Path(source_path).parent if source_path else Path.cwd()
        if (parent_dir / '.roomodes').exists():
            return 'roo-code'
        return 'cline'
    if '.roomodes' in p: return 'roo-code'
    if '.mdc' in p: return 'cursor'
    if 'copilot-instructions' in p: return 'github-copilot'
    if 'config.json' in p: return 'continue'
    if 'CONVENTIONS.md' in p or 'aider' in p: return 'aider'

    # 2) 目录结构检测
    if p.endswith('/skill.md') or p.endswith('\\skill.md'):
        parent = Path(source_path).parent.name.lower()
        if parent and parent != 'skills':
            pass  # 可能是 openclaw/hermes/opencode/skills-cli，继续走 frontmatter 检测

    # 3) Frontmatter 关键词检测
    if text.startswith('---'):
        lines = text.split('\n')
        has_openclaw_meta = False
        has_reasonix = False
        has_cursor = False
        has_windsurf = False
        for line in lines[:25]:
            l = line.strip()
            if l.startswith('runAs:'): has_reasonix = True
            if l.startswith('allowed-tools:'): has_reasonix = True
            if l.startswith('model:'): has_reasonix = True
            if l.startswith('globs:'): has_cursor = True
            if l.startswith('tags:'): has_windsurf = True
            if l.startswith('metadata:'): has_openclaw_meta = True
            if 'openclaw' in l: has_openclaw_meta = True

        if has_openclaw_meta and not has_reasonix:
            return 'openclaw'
        if has_reasonix:
            return 'reasonix'
        if has_windsurf:
            return 'windsurf'
        if has_cursor:
            return 'cursor'
        return 'generic-yaml'

    return 'plain-markdown'


def parse_skill(text: str, source_format: str = 'auto', source_path: str = '') -> dict:
    """解析任意格式 skill → 统一中间字典（增强版：保留 metadata JSON）"""
    if source_format == 'auto':
        source_format = detect_format(text, source_path)

    skill = {'name': '', 'description': '', 'body': '', 'fields': {}}

    yaml_formats = ('reasonix', 'hermes', 'opencode', 'claude-code', 'openai-codex',
                    'skills-cli', 'generic-yaml', 'cursor', 'openclaw', 'windsurf')

    if source_format in yaml_formats:
        m = re.match(r'^---\s*\n(.*?)\n?---\s*\n(.*)', text, re.DOTALL)
        if m:
            yaml_text, body = m.group(1), m.group(2).strip()
            # 特殊处理 metadata 行（可能包含内联 JSON）
            metadata_raw = None
            meta_lines = []
            in_metadata = False
            meta_depth = 0

            for line in yaml_text.strip().split('\n'):
                stripped = line.strip()
                if stripped.startswith('metadata:'):
                    rest = stripped[len('metadata:'):].strip()
                    if rest:
                        # metadata: {...} 在同一行
                        metadata_raw = rest
                    else:
                        in_metadata = True
                        meta_depth = 1
                    continue
                if in_metadata:
                    meta_lines.append(line)
                    meta_depth += line.count('{') - line.count('}')
                    if meta_depth <= 0:
                        metadata_raw = '\n'.join(meta_lines)
                        in_metadata = False
                        meta_lines = []
                    continue

                if ':' in line:
                    k, _, v = line.partition(':')
                    k, v = k.strip(), v.strip()
                    if k == 'name': skill['name'] = v
                    elif k == 'description': skill['description'] = v
                    elif k != 'metadata':
                        skill['fields'][k] = v

            # 解析 metadata JSON
            if metadata_raw:
                try:
                    # 清理可能混入的后续 frontmatter 内容
                    meta_json = metadata_raw.strip()
                    # 只取第一个完整的 JSON 对象
                    depth = 0
                    for i, ch in enumerate(meta_json):
                        if ch == '{': depth += 1
                        elif ch == '}': depth -= 1
                        if depth == 0 and i > 0:
                            meta_json = meta_json[:i+1]
                            break
                    skill['fields']['metadata'] = json.loads(meta_json)
                except json.JSONDecodeError:
                    skill['fields']['metadata'] = metadata_raw

            skill['body'] = body
        else:
            skill['body'] = text.strip()

    elif source_format in ('github-copilot', 'aider', 'plain-markdown'):
        lines = text.strip().split('\n')
        body_lines = []
        for line in lines:
            if line.startswith('# ') and not skill['name']:
                skill['name'] = line[2:].strip()
            elif line.startswith('> ') and not skill['description']:
                skill['description'] = line[2:].strip()
            else:
                body_lines.append(line)
        skill['name'] = skill['name'] or 'unnamed-skill'
        skill['body'] = '\n'.join(body_lines).strip()

    elif source_format == 'continue':
        try:
            data = json.loads(text)
            skill['name'] = data.get('name', 'unnamed')
            skill['description'] = data.get('description', '')
            skill['body'] = data.get('systemPrompt', data.get('prompt', ''))
            skill['fields'] = data
        except:
            skill['body'] = str(text)

    skill['name'] = skill['name'] or 'unnamed-skill'
    return skill


def render_skill(skill: dict, target_format: str, install_path: Path = None) -> str:
    """中间字典 → 目标格式文本（支持 OpenClaw metadata 内联 JSON）"""
    agent = AGENTS.get(target_format)
    if not agent:
        raise ValueError(f'不支持的格式: {target_format}')

    name, desc, body, fields = skill['name'], skill['description'], skill['body'], skill['fields']

    # ── 带 YAML frontmatter 的格式 ──
    if target_format in ('reasonix', 'hermes', 'opencode', 'skills-cli', 'openclaw'):
        out_fields = {}
        for f in agent['fields']:
            if f == 'name': out_fields['name'] = name
            elif f == 'description': out_fields['description'] = desc
            elif f == 'metadata' and f in fields:
                # metadata 需要序列化为 JSON
                if isinstance(fields[f], dict):
                    out_fields['metadata'] = json.dumps(fields[f], ensure_ascii=False)
                else:
                    out_fields['metadata'] = str(fields[f])
            elif f in fields and fields[f]:
                out_fields[f] = str(fields[f])
            elif f in agent.get('defaults', {}) and agent['defaults'][f]:
                out_fields[f] = agent['defaults'][f]

        # 额外的字段传递（不丢失信息）
        for k, v in fields.items():
            if k not in out_fields and k not in ('name', 'description', 'metadata'):
                out_fields[k] = str(v)

        yaml_lines = ['---']
        for k, v in out_fields.items():
            if v is not None and v != '':
                if k == 'metadata' and isinstance(v, str) and v.startswith('{'):
                    yaml_lines.append(f'{k}: {v}')
                else:
                    yaml_lines.append(f'{k}: {v}')
        yaml_lines.append('---')
        return '\n'.join(yaml_lines) + '\n\n' + body

    # ── Claude Code / OpenAI Codex ──
    elif target_format in ('claude-code', 'openai-codex'):
        yaml = f'---\nname: {name}\ndescription: {desc}\n---'
        content = yaml + '\n\n' + body
        if install_path and 'extra_files' in agent:
            plugin = agent['extra_files']['plugin.json'].format(name=name, desc=desc)
            (install_path.parent / 'plugin.json').write_text(plugin, encoding='utf-8')
        return content

    # ── Cursor .mdc ──
    elif target_format == 'cursor':
        t = agent['template']
        globs = fields.get('globs', '**/*')
        return t.format(name=name, desc=desc, body=body, globs=globs)

    # ── GitHub Copilot / Aider ──
    elif target_format in ('github-copilot', 'aider'):
        return agent['template'].format(name=name, desc=desc, body=body)

    # ── Windsurf .windsurfrules ──
    elif target_format == 'windsurf':
        t = agent['template']
        globs = fields.get('globs', '**/*')
        tags = fields.get('tags', '')
        return t.format(desc=desc, body=body, globs=globs, tags=tags)

    # ── Cline / Roo Code (纯 markdown) ──
    elif target_format in ('cline', 'roo-code'):
        return agent['template'].format(name=name, desc=desc, body=body)

    # ── Continue.dev JSON ──
    elif target_format == 'continue':
        data = {'name': name, 'description': desc, 'systemPrompt': body}
        if isinstance(fields, dict):
            for k, v in fields.items():
                if k not in data: data[k] = v
        return json.dumps(data, ensure_ascii=False, indent=2)

    return body


def get_target_path(agent_id: str, name: str, base_dir: str = None) -> Path:
    """计算技能目标路径 — 优先项目级，支持路径覆盖"""
    agent = AGENTS[agent_id]
    home = Path.home()
    cwd = Path(base_dir).resolve() if base_dir else Path.cwd()

    # 检查配置中的路径覆盖
    config = load_config()
    path_key = f'{agent_id}.path'
    if path_key in config.get('path_overrides', {}):
        base = Path(config['path_overrides'][path_key])
        fname = agent['file_name'].format(name=name)
        return (base / name) / fname if agent['dir_needed'] else base / fname

    if agent_id == 'reasonix':
        proj = cwd / '.reasonix' / 'skills'
        base = proj if proj.exists() else home / '.reasonix' / 'skills'
    elif agent_id == 'hermes':
        base = cwd / '.hermes' / 'skills' if (cwd / '.hermes').exists() else home / '.hermes' / 'skills'
    elif agent_id == 'opencode':
        base = cwd / '.config' / 'opencode' / 'skills' if (cwd / '.config' / 'opencode').exists() else home / '.config' / 'opencode' / 'skills'
    elif agent_id == 'claude-code':
        base = cwd / '.claude' / 'plugins'
    elif agent_id == 'openai-codex':
        base = cwd / '.agents' / 'plugins'
    elif agent_id == 'cursor':
        base = cwd / '.cursor' / 'rules'
    elif agent_id == 'github-copilot':
        base = cwd / '.github'
    elif agent_id == 'continue':
        base = home / '.continue'
    elif agent_id == 'aider':
        base = cwd
    elif agent_id == 'skills-cli':
        base = cwd / '.skills'
    elif agent_id == 'openclaw':
        ws = cwd / '.openclaw' / 'workspace' / 'skills'
        managed = home / '.openclaw' / 'skills'
        base = ws if ws.exists() else managed
    elif agent_id == 'windsurf':
        base = cwd
    elif agent_id in ('cline', 'roo-code'):
        base = cwd
    else:
        base = cwd

    fname = agent['file_name'].format(name=name)
    if agent['dir_needed']:
        return (base / name) / fname
    return base / fname


def validate_yaml(text: str) -> list:
    """检查渲染后的 skill 文件基本语法正确性"""
    errors = []
    pairs = [('{', '}'), ('[', ']'), ('(', ')')]
    for o, c in pairs:
        if text.count(o) != text.count(c):
            errors.append(f'{o}/{c} 不匹配 ({text.count(o)} vs {text.count(c)})')
    return errors


# ═══════════════════════════════════════════════════════════
# 依赖检测引擎
# ═══════════════════════════════════════════════════════════

DEPENDENCY_PATTERNS = {
    'bins': [
        r'(?:^|\s)(ffmpeg|node|python3?|uv|git|curl|wget|docker|npx|npm|pnpm|yarn|bun|go|rustc|cargo|gemini|claude)(?:\s|$)',
        r'run_command\s*\(\s*[\'\"]([^\'\"]+?)(?:\s|$)',
    ],
    'env': [
        r'(?:os\.environ|process\.env)\s*\[\s*[\'\"]([\w_]+)[\'\"]\s*\]',
        r'(?:os\.getenv|os\.environ\.get)\s*\(\s*[\'\"]([\w_]+)[\'\"]\s*\)',
        r'requires\.env.*?[\'\"]([\w_]+)[\'\"]',
    ],
    'skills': [
        r'run_skill\([\'\"]([\w-]+)[\'\"]\)',
        r'/skill\s+([\w-]+)',
    ],
    'packages_pip': [
        r'(?:pip|pip3)\s+install\s+([\w-]+)',
        r'import\s+(\w+)',
    ],
    'packages_npm': [
        r'(?:npm|pnpm|yarn)\s+(?:install|add|i)\s+([\w@/-]+)',
        r'require\([\'\"]([\w@/-]+)[\'\"]\)',
        r'from\s+[\'\"]([\w@/-]+)[\'\"]',
    ],
}


def detect_dependencies(text: str, source_path: str = '') -> dict:
    """从 skill 文本中自动检测外部依赖"""
    deps = {'bins': set(), 'env': set(), 'skills': set(),
            'packages_pip': set(), 'packages_npm': set(), 'config': set()}

    # 1) 检查 metadata 中的显式声明
    if text.startswith('---'):
        m = re.match(r'^---\s*\n(.*?)\n?---', text, re.DOTALL)
        if m:
            yaml_text = m.group(1)
            # 提取 metadata JSON
            meta_match = re.search(r'metadata:\s*(\{.*\})', yaml_text, re.DOTALL)
            if meta_match:
                try:
                    meta = json.loads(meta_match.group(1))
                    oc = meta.get('openclaw', {})
                    requires = oc.get('requires', {})
                    if isinstance(requires, dict):
                        for b in requires.get('bins', []):
                            deps['bins'].add(b)
                        for e in requires.get('env', []):
                            deps['env'].add(e)
                        for c in requires.get('config', []):
                            deps['config'].add(c)
                except json.JSONDecodeError:
                    pass

    # 2) Body 扫描（启发式）
    body_match = re.search(r'---\s*\n(.*?)\n?---\s*\n(.*)', text, re.DOTALL)
    body = body_match.group(2) if body_match else text

    for dep_type, patterns in DEPENDENCY_PATTERNS.items():
        for pat in patterns:
            for match in re.finditer(pat, body, re.IGNORECASE):
                deps[dep_type].add(match.group(1).strip())

    # 3) 过滤常见误报
    ignore_bins = {'node', 'python', 'python3', 'git', 'curl', 'wget', 'docker', 'npx', 'npm', 'pip'}
    ignore_pips = {'os', 'sys', 'json', 're', 'pathlib', 'typing', 'datetime',
                   'shutil', 'time', 'glob', 'subprocess', 'math', 'random'}
    ignore_npms = {'fs', 'path', 'os', 'util', 'child_process', 'process'}

    deps['bins'] = {b for b in deps['bins'] if b.lower() not in ignore_bins}
    deps['packages_pip'] = {p for p in deps['packages_pip'] if p.lower() not in ignore_pips}
    deps['packages_npm'] = {n for n in deps['packages_npm'] if n.lower() not in ignore_npms}

    # 4) 检查当前环境
    present_bins = []
    missing_bins = []
    for b in sorted(deps['bins']):
        if shutil.which(b):
            present_bins.append(b)
        else:
            missing_bins.append(b)

    present_env = []
    missing_env = []
    for e in sorted(deps['env']):
        if e in os.environ:
            present_env.append(e)
        else:
            missing_env.append(e)

    return {
        'declared': {k: sorted(v) for k, v in deps.items() if v},
        'present_bins': present_bins,
        'missing_bins': missing_bins,
        'present_env': present_env,
        'missing_env': missing_env,
    }


# ═══════════════════════════════════════════════════════════
# 兼容性验证引擎
# ═══════════════════════════════════════════════════════════

def check_compatibility(skill: dict, target_format: str = None) -> dict:
    """检查 skill 与目标格式的兼容性，返回评分 + issues"""
    issues = []
    name = skill['name']
    desc = skill['description']
    body = skill['body']
    fields = skill['fields']

    # 总是检查语法
    rendered = None
    syntax_errors = validate_yaml(
        f'---\nname: {name}\ndescription: {desc}\n---\n\n{body}'
    )
    for err in syntax_errors:
        issues.append({'severity': 'error', 'type': 'syntax', 'msg': err})

    if target_format:
        agent = AGENTS.get(target_format)
        if not agent:
            issues.append({'severity': 'error', 'type': 'unknown_format', 'msg': f'不支持的格式: {target_format}'})
        else:
            # 字段丢失检查
            for f in agent['fields']:
                if f in ('name', 'description', 'metadata'):
                    continue
                if f not in fields and f not in agent.get('defaults', {}):
                    issues.append({
                        'severity': 'warning',
                        'type': 'field_loss',
                        'msg': f'字段 "{f}" 不支持目标格式 {agent["name"]}'
                    })
                    break  # 一个格式报一次即可

            # 无损渲染测试
            try:
                rendered = render_skill(skill, target_format)
                se = validate_yaml(rendered)
                for err in se:
                    issues.append({'severity': 'error', 'type': 'render_error', 'msg': f'渲染后语法错误: {err}'})
            except Exception as e:
                issues.append({'severity': 'error', 'type': 'render_failed', 'msg': f'渲染失败: {e}'})

    # 依赖检测
    deps = detect_dependencies(
        f'---\nname: {name}\ndescription: {desc}\n---\n\n{body}'
    )
    for b in deps.get('missing_bins', []):
        issues.append({'severity': 'warning', 'type': 'missing_bin', 'msg': f'缺少二进制: {b}'})
    for e in deps.get('missing_env', []):
        issues.append({'severity': 'warning', 'type': 'missing_env', 'msg': f'缺少环境变量: {e}'})

    # 评分
    has_error = any(i['severity'] == 'error' for i in issues)
    has_warning = any(i['severity'] == 'warning' for i in issues)

    if has_error:
        score = 'F'
    elif has_warning:
        score = 'C'
    else:
        score = 'A'

    return {
        'compatible': not has_error,
        'score': score,
        'issues': issues,
        'dependencies': deps,
    }


# ═══════════════════════════════════════════════════════════
# 命令: install  安装/转换 skill
# ═══════════════════════════════════════════════════════════

def cmd_install(source: str, to: list, dry_run: bool = False, backup: bool = False,
                base_dir: str = None, json_output: bool = False) -> dict:
    """安装 skill 到指定 Agent"""
    result = {'action': 'install', 'source': source, 'ok': True, 'agents': {}}

    # 读源文件
    if source.startswith(('http://', 'https://')):
        import urllib.request
        try:
            req = urllib.request.Request(source, headers={'User-Agent': 'Agent/1.0'})
            resp = urllib.request.urlopen(req, timeout=15)
            text = resp.read().decode('utf-8')
        except Exception as e:
            return {'ok': False, 'error': f'下载失败: {e}'}
        src_path = source
    else:
        p = Path(source)
        if not p.exists():
            # 可能是子目录中的 SKILL.md
            alt = Path(source) / 'SKILL.md'
            if alt.exists():
                p = alt
            else:
                return {'ok': False, 'error': f'文件不存在: {source}'}
        text = p.read_text(encoding='utf-8')
        src_path = str(p)

    # 解析
    fmt = detect_format(text, src_path)
    skill = parse_skill(text, fmt, src_path)
    result['name'] = skill['name']
    result['description'] = skill['description'][:80]
    result['detected_format'] = fmt
    result['body_chars'] = len(skill['body'])

    if not to:
        config = load_config()
        to = config.get('default_targets', ['reasonix'])

    for agent_id in to:
        agent = AGENTS.get(agent_id)
        if not agent:
            result['agents'][agent_id] = {'ok': False, 'error': f'不支持的格式'}
            continue

        # 兼容性检查
        compat = check_compatibility(skill, agent_id)
        if compat['score'] == 'F':
            result['agents'][agent_id] = {
                'ok': False, 'error': '兼容性检查失败',
                'issues': [i['msg'] for i in compat['issues'] if i['severity'] == 'error']
            }
            continue

        target_path = get_target_path(agent_id, skill['name'], base_dir)
        content = render_skill(skill, agent_id, target_path if not dry_run else None)

        errors = validate_yaml(content)
        if errors:
            result['agents'][agent_id] = {'ok': False, 'error': '语法验证失败', 'details': errors}
            continue

        agent_result = {
            'ok': True,
            'agent': agent['name'],
            'path': str(target_path),
            'chars': len(content),
            'action': 'dry-run' if dry_run else 'write',
            'compatibility': compat,
        }

        if not dry_run:
            target_path.parent.mkdir(parents=True, exist_ok=True)
            if backup and target_path.exists():
                bak = target_path.with_suffix(f'.bak.{int(time.time())}{target_path.suffix}')
                shutil.copy2(target_path, bak)
                agent_result['backup'] = str(bak)
            target_path.write_text(content, encoding='utf-8')

        result['agents'][agent_id] = agent_result

    result['ok'] = all(a.get('ok') for a in result['agents'].values())

    if json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        _print_install_result(result)

    return result


def _print_install_result(result: dict):
    if not result.get('ok'):
        print(f'❌ 安装失败: {result.get("error", "未知错误")}')
        for aid, ar in result.get('agents', {}).items():
            if not ar.get('ok'):
                print(f'  ❌ {aid}: {ar.get("error", "")}')
                for issue in ar.get('issues', []):
                    print(f'     └─ {issue}')
        return
    print(f'📦 {result["name"]}  ({result["body_chars"]} 字, 检测格式: {result["detected_format"]})')
    for aid, ar in result['agents'].items():
        if ar.get('ok'):
            tag = '🔍' if ar['action'] == 'dry-run' else '✅'
            compat_score = f' [{ar.get("compatibility", {}).get("score", "?")}]'
            backup = f' (备份: {ar["backup"]})' if 'backup' in ar else ''
            print(f'  {tag} {ar["agent"]:<20s} → {ar["path"]}{compat_score}{backup}')
            deps = ar.get('compatibility', {}).get('dependencies', {})
            if deps.get('missing_bins'):
                print(f'     ⚠️ 缺少: {", ".join(deps["missing_bins"])}')
        else:
            print(f'  ❌ {aid:<20s} {ar.get("error", "")}')
            for d in ar.get('details', []):
                print(f'     └─ {d}')
            for issue in ar.get('issues', []):
                print(f'     └─ {issue}')
    print(f'  共 {len(result["agents"])} 个目标')


# ═══════════════════════════════════════════════════════════
# 命令: inspect  查看 skill 文件信息（增强版：含依赖+兼容性）
# ═══════════════════════════════════════════════════════════

def cmd_inspect(source: str, json_output: bool = False):
    """查看 skill 文件的结构信息 + 依赖 + 兼容性"""
    p = Path(source)
    if not p.exists():
        alt = p / 'SKILL.md'
        if alt.exists():
            p = alt
        else:
            print(json.dumps({'ok': False, 'error': f'文件不存在: {source}'}) if json_output else f'❌ 文件不存在: {source}')
            return

    text = p.read_text(encoding='utf-8')
    fmt = detect_format(text, str(p))
    skill = parse_skill(text, fmt, str(p))
    deps = detect_dependencies(text, str(p))

    result = {
        'ok': True,
        'name': skill['name'],
        'description': skill['description'],
        'detected_format': fmt,
        'fields': dict(skill['fields']),
        'body_chars': len(skill['body']),
        'dependencies': deps,
        'agent_compatible': [],
    }

    # 检查兼容哪些 agent
    for aid, agent in AGENTS.items():
        compat = check_compatibility(skill, aid)
        result['agent_compatible'].append({
            'agent': aid,
            'name': agent['name'],
            'compatible': compat['compatible'],
            'score': compat['score'],
            'issues': compat['issues'],
        })

    if json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f'📋 {result["name"]}')
        print(f'  描述: {result["description"] or "(无)"}')
        print(f'  检测格式: {result["detected_format"]}')
        print(f'  正文: {result["body_chars"]} 字符')
        if result['fields']:
            print(f'  字段:')
            for k, v in result['fields'].items():
                val = json.dumps(v, ensure_ascii=False)[:60] if isinstance(v, (dict, list)) else str(v)[:60]
                print(f'    {k}: {val}')

        # 依赖
        dep = result['dependencies']
        if dep.get('declared'):
            print(f'\n  📦 依赖:')
            for k, v in dep['declared'].items():
                if v:
                    print(f'    {k}: {", ".join(v)}')
        if dep.get('missing_bins'):
            print(f'  ⚠️ 缺少二进制: {", ".join(dep["missing_bins"])}')
        if dep.get('missing_env'):
            print(f'  ⚠️ 缺少环境变量: {", ".join(dep["missing_env"])}')

        print(f'\n  兼容性:')
        for ac in result['agent_compatible']:
            icons = {'A': '✅', 'C': '⚠️', 'F': '❌'}
            icon = icons.get(ac['score'], '❓')
            extra = ''
            for i in ac.get('issues', []):
                if i['severity'] == 'warning':
                    extra = f' — {i["msg"]}'
                    break
            print(f'    {icon} {ac["name"]:<20s} [{ac["score"]}]{extra}')


# ═══════════════════════════════════════════════════════════
# 命令: list  列出已安装的 skills
# ═══════════════════════════════════════════════════════════

def cmd_list(agent_id: str = 'reasonix', base_dir: str = None, json_output: bool = False):
    agent = AGENTS.get(agent_id)
    if not agent:
        print(json.dumps({'ok': False, 'error': f'不支持的 agent: {agent_id}'}) if json_output else f'❌ 不支持的 agent: {agent_id}')
        return

    cwd = Path(base_dir).resolve() if base_dir else Path.cwd()
    home = Path.home()

    dir_map = {
        'reasonix': [cwd / '.reasonix' / 'skills', home / '.reasonix' / 'skills'],
        'hermes': [cwd / '.hermes' / 'skills', home / '.hermes' / 'skills'],
        'opencode': [cwd / '.config' / 'opencode' / 'skills', home / '.config' / 'opencode' / 'skills'],
        'claude-code': [cwd / '.claude' / 'plugins'],
        'openai-codex': [cwd / '.agents' / 'plugins'],
        'cursor': [cwd / '.cursor' / 'rules'],
        'github-copilot': [cwd / '.github'],
        'aider': [cwd],
        'skills-cli': [cwd / '.skills'],
        'windsurf': [cwd],
        'cline': [cwd],
        'roo-code': [cwd],
        'openclaw': [
            cwd / '.openclaw' / 'workspace' / 'skills',
            home / '.openclaw' / 'skills',
            home / '.agents' / 'skills',
        ],
    }
    dirs = dir_map.get(agent_id, [cwd])

    skills = []
    for d in dirs:
        if not d.exists():
            continue
        if agent['dir_needed']:
            for sub in d.iterdir():
                if sub.is_dir():
                    sf = sub / agent['file_name'].format(name=sub.name)
                    if sf.exists():
                        sz = sf.stat().st_size
                        mt = datetime.fromtimestamp(sf.stat().st_mtime).isoformat()
                        skills.append({'name': sub.name, 'path': str(sf), 'size': sz, 'mtime': mt, 'dir': str(d)})
        else:
            for f in d.iterdir():
                if f.is_file() and not f.name.startswith('.bak'):
                    sz = f.stat().st_size
                    mt = datetime.fromtimestamp(f.stat().st_mtime).isoformat()
                    skills.append({'name': f.stem, 'path': str(f), 'size': sz, 'mtime': mt, 'dir': str(d)})

    result = {'ok': True, 'agent': agent['name'], 'count': len(skills), 'skills': skills}
    if json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f'📋 {agent["name"]} — 已安装 {len(skills)} 个技能')
        for s in skills:
            print(f'  📄 {s["name"]:<25s} {s["path"]}')
            print(f'     大小: {s["size"]}B  修改: {s["mtime"][:19]}')


# ═══════════════════════════════════════════════════════════
# 命令: detect-agent  环境感知检测
# ═══════════════════════════════════════════════════════════

AGENT_SIGNATURES = {
    'reasonix': ['.reasonix/', '.reasonix/skills/'],
    'hermes': ['.hermes/', '.hermes/skills/'],
    'opencode': ['.config/opencode/', '.config/opencode/skills/'],
    'claude-code': ['.claude/', '.claude/plugins/'],
    'openai-codex': ['.agents/', '.agents/plugins/'],
    'cursor': ['.cursor/', '.cursor/rules/'],
    'github-copilot': ['.github/', '.github/copilot-instructions.md'],
    'aider': ['CONVENTIONS.md'],
    'skills-cli': ['.skills/', '.skills/'],
    'openclaw': ['.openclaw/', '.openclaw/workspace/skills/', '.openclaw/skills/'],
    'windsurf': ['.windsurf/', '.windsurfrules'],
    'cline': ['.clinerules', '.clinerules/'],
    'roo-code': ['.roomodes', '.clinerules'],
}


def detect_agent(cwd: str = None) -> dict:
    """扫描当前项目目录，检测在用哪些 AI Agent"""
    root = Path(cwd).resolve() if cwd else Path.cwd()
    detected = []
    all_formats = []

    for aid, signatures in AGENT_SIGNATURES.items():
        score = 0
        evidence = []
        for sig in signatures:
            path = root / sig
            if path.exists():
                score += 1
                evidence.append(str(path))
        if score > 0:
            detected.append({
                'id': aid,
                'name': AGENTS[aid]['name'],
                'score': score,
                'evidence': evidence,
            })
            all_formats.append(aid)

    # 按匹配度排序
    detected.sort(key=lambda x: -x['score'])

    return {
        'cwd': str(root),
        'detected': detected,
        'formats': all_formats,
        'primary': detected[0]['id'] if detected else 'reasonix',
    }


def cmd_detect_agent(json_output: bool = False):
    result = detect_agent()
    if json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f'📡 环境检测: {result["cwd"]}')
        if result['detected']:
            print(f'  检测到 {len(result["detected"])} 个 Agent:')
            for d in result['detected']:
                print(f'    ✅ {d["name"]:<20s} [{d["id"]}]')
                for e in d['evidence']:
                    print(f'       📁 {e}')
        else:
            print('  未检测到任何 Agent 环境')
        print(f'  推荐目标: {result["primary"]}')


# ═══════════════════════════════════════════════════════════
# 自安装 Skill 模板
# ═══════════════════════════════════════════════════════════

def _self_skill_content(for_agent: str) -> str:
    """生成转换器自身的 skill 文件内容"""
    name = 'skill-converter'
    desc = '万能 Skill 转换器 — 跨 Agent 格式转换/检测/同步/监控'

    if for_agent == 'reasonix':
        return f'''---
name: {name}
description: >-
  {desc}
runAs: subagent
allowed-tools: run_command
model: deepseek-v4-flash
---
# {name}

{desc}

## 用法
`run_skill("skill-converter", "<命令> [参数]")`

## 命令
- `install <file> --to <agents>` — 安装 skill 到指定 Agent
- `check <file>` — 检测兼容性/依赖
- `inspect <file>` — 查看 skill 信息
- `sync --from <dir> --to <agents>` — 双向同步
- `watch <dir> --to <agents>` — 自动监控
- `list --agent <id>` — 列出已安装技能

## 支持 {len(AGENTS)} 种格式
{", ".join(AGENTS.keys())}
'''
    elif for_agent in ('windsurf',):
        return f'''---
description: >-
  {desc}
globs: **/*
---

# {name}

{desc}

## Commands
- install <file> --to <agents> — Install skill to target agent
- check <file> — Check compatibility & dependencies
- inspect <file> — View skill info
- sync --from <dir> --to <agents> — Bidirectional sync
- watch <dir> --to <agents> — Auto-watch & sync
'''
    elif for_agent in ('cline', 'roo-code'):
        return f'# {name}\n\n> {desc}\n\n## Usage\nSee [converter docs]\n'
    else:
        return f'# {name}\n\n{desc}\n'


def cmd_self_install(json_output: bool = False):
    """将转换器自身安装为当前环境的 skill"""
    env = detect_agent()
    primary = env['primary']
    agent = AGENTS.get(primary)
    if not agent:
        print(json.dumps({'ok': False, 'error': f'未知 Agent: {primary}'}) if json_output else f'❌ 未知 Agent: {primary}')
        return

    content = _self_skill_content(primary)
    skill = parse_skill(content, primary)
    target_path = get_target_path(primary, skill['name'])

    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.write_text(content, encoding='utf-8')

    result = {
        'ok': True,
        'agent': primary,
        'name': agent['name'],
        'path': str(target_path),
        'note': f'转换器已安装为 {agent["name"]} 的 skill: run_skill("skill-converter", ...)',
    }

    if json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f'✅ 自安装完成')
        print(f'  Agent: {agent["name"]} [{primary}]')
        print(f'  路径: {target_path}')
        print(f'  用法: 现在你可以通过此 Agent 的 skill 机制调用转换器')


def cmd_bootstrap(json_output: bool = False):
    """一键引导：检测环境 → 自安装 → 报告"""
    print('🚀 万能 Skill 转换器 — 一键引导')
    print()

    # 1. 检测环境
    env = detect_agent()
    print(f'📡 当前目录: {env["cwd"]}')
    if env['detected']:
        print(f'  检测到 {len(env["detected"])} 个 Agent 环境:')
        for d in env['detected']:
            print(f'    ✅ {d["name"]:<20s}')
    else:
        print('  未检测到特定 Agent 环境，使用默认: reasonix')
    print()

    # 2. 自安装
    primary = env['primary']
    print(f'📦 正在安装为 {AGENTS[primary]["name"]} 的 skill...')
    cmd_self_install(json_output)
    print()

    # 3. 使用说明
    print('📖 使用说明:')
    print(f'  python universal-skill-converter.py --help          # 查看帮助')
    print(f'  python universal-skill-converter.py install foo.md   # 安装 skill')
    print(f'  python universal-skill-converter.py check foo.md     # 检测兼容性')
    print(f'  python universal-skill-converter.py --self-install   # 重新安装')
    print(f'  python universal-skill-converter.py --detect-agent   # 环境检测')
    print()
    print('✅ 引导完成!')


# ═══════════════════════════════════════════════════════════
# 命令: check  兼容性/依赖检测
# ═══════════════════════════════════════════════════════════

def cmd_check(source: str, to: list = None, json_output: bool = False):
    """检测 skill 文件：格式/依赖/兼容性"""
    p = Path(source)
    if not p.exists():
        alt = p / 'SKILL.md'
        if alt.exists():
            p = alt
        else:
            print(json.dumps({'ok': False, 'error': f'文件不存在: {source}'}) if json_output else f'❌ 文件不存在: {source}')
            return

    text = p.read_text(encoding='utf-8')
    fmt = detect_format(text, str(p))
    skill = parse_skill(text, fmt, str(p))
    deps = detect_dependencies(text, str(p))

    result = {
        'ok': True,
        'name': skill['name'],
        'description': skill['description'],
        'detected_format': fmt,
        'dependencies': deps,
        'targets': {},
    }

    targets = to or list(AGENTS.keys())
    for aid in targets:
        if aid in AGENTS:
            result['targets'][aid] = check_compatibility(skill, aid)

    if json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f'📋 兼容性报告: {result["name"]}')
        print(f'  格式: {result["detected_format"]}')
        dep = result['dependencies']
        if dep.get('declared'):
            for k, v in dep['declared'].items():
                if v:
                    print(f'  📦 {k}: {", ".join(v)}')
        if dep.get('missing_bins'):
            print(f'  ⚠️ 缺少二进制: {", ".join(dep["missing_bins"])}')
        if dep.get('missing_env'):
            print(f'  ⚠️ 缺少环境变量: {", ".join(dep["missing_env"])}')
        if not dep.get('missing_bins') and not dep.get('missing_env'):
            print(f'  ✅ 依赖全部就绪')

        print(f'\n  目标兼容性:')
        for aid, compat in result['targets'].items():
            icons = {'A': '✅', 'C': '⚠️', 'F': '❌'}
            icon = icons.get(compat['score'], '❓')
            warnings = [i['msg'] for i in compat.get('issues', []) if i['severity'] == 'warning']
            detail = f' — {warnings[0]}' if warnings else ''
            print(f'    {icon} {AGENTS[aid]["name"]:<20s} [{compat["score"]}]{detail}')


# ═══════════════════════════════════════════════════════════
# 命令: convert  导出为指定格式 (stdout)
# ═══════════════════════════════════════════════════════════

def cmd_convert(source: str, to: str, json_output: bool = False):
    p = Path(source)
    if not p.exists():
        alt = p / 'SKILL.md'
        if alt.exists():
            p = alt
        else:
            r = {'ok': False, 'error': f'文件不存在: {source}'}
            print(json.dumps(r) if json_output else f'❌ 文件不存在')
            return

    text = p.read_text(encoding='utf-8')
    fmt = detect_format(text, str(p))
    skill = parse_skill(text, fmt, str(p))

    try:
        content = render_skill(skill, to)
    except ValueError as e:
        r = {'ok': False, 'error': str(e)}
        print(json.dumps(r) if json_output else f'❌ {e}')
        return

    if json_output:
        print(json.dumps({'ok': True, 'name': skill['name'], 'format': to, 'content': content}, ensure_ascii=False, indent=2))
    else:
        print(content)


# ═══════════════════════════════════════════════════════════
# 命令: install-dir  批量安装目录
# ═══════════════════════════════════════════════════════════

def cmd_install_dir(skill_dir: str, to: list, dry_run: bool = False, backup: bool = False,
                    base_dir: str = None, json_output: bool = False):
    d = Path(skill_dir)
    if not d.is_dir():
        r = {'ok': False, 'error': f'目录不存在: {skill_dir}'}
        print(json.dumps(r) if json_output else f'❌ 目录不存在')
        return

    md_files = list(d.glob('*.md')) + list(d.glob('*.mdc'))
    # 也搜索子目录中的 SKILL.md
    for sub in d.iterdir():
        if sub.is_dir():
            sf = sub / 'SKILL.md'
            if sf.exists():
                md_files.append(sf)

    if not md_files:
        r = {'ok': False, 'error': f'{skill_dir} 中没有 skill 文件'}
        print(json.dumps(r) if json_output else f'❌ 目录中没有 skill 文件')
        return

    results = []
    for f in md_files:
        r = cmd_install(str(f), to, dry_run, backup, base_dir, json_output=True)
        results.append(r)

    summary = {
        'ok': all(r.get('ok') for r in results),
        'total': len(results),
        'succeeded': sum(1 for r in results if r.get('ok')),
        'failed': sum(1 for r in results if not r.get('ok')),
        'results': results,
    }

    if json_output:
        print(json.dumps(summary, ensure_ascii=False, indent=2))
    else:
        print(f'\n📊 批量安装完成: {summary["succeeded"]}/{summary["total"]} 成功')
        for r in results:
            if r.get('ok'):
                agents = ', '.join(r['agents'].keys())
                print(f'  ✅ {r["name"]:<25s} → {agents}')
            else:
                print(f'  ❌ {r.get("name", Path(r["source"]).name):<25s} {r.get("error", "")}')


# ═══════════════════════════════════════════════════════════
# 命令: sync  双向同步
# ═══════════════════════════════════════════════════════════

def cmd_sync(source_dir: str, to: list, dry_run: bool = False, backup: bool = False,
             filter_compatible: bool = False, json_output: bool = False):
    """将源目录下的所有 skills 同步到目标 Agent"""
    d = Path(source_dir)
    if not d.is_dir():
        print(json.dumps({'ok': False, 'error': f'目录不存在: {source_dir}'}) if json_output else f'❌ 目录不存在: {source_dir}')
        return

    if not to:
        config = load_config()
        to = config.get('default_targets', ['reasonix'])

    print(f'🔁 双向同步: {source_dir} → {", ".join(to)}')

    # 扫描源目录
    skill_files = []
    for f in d.glob('*.md'):
        skill_files.append(f)
    for f in d.glob('*.mdc'):
        skill_files.append(f)
    for sub in d.iterdir():
        if sub.is_dir():
            sf = sub / 'SKILL.md'
            if sf.exists():
                skill_files.append(sf)

    if not skill_files:
        print('❌ 源目录中没有 skill 文件')
        return

    results = []
    for sf in skill_files:
        text = sf.read_text('utf-8')
        fmt = detect_format(text, str(sf))
        skill = parse_skill(text, fmt, str(sf))

        entry = {'file': str(sf), 'name': skill['name'], 'format': fmt, 'targets': {}}

        for aid in to:
            agent = AGENTS.get(aid)
            if not agent:
                entry['targets'][aid] = {'ok': False, 'error': '不支持的格式'}
                continue

            # 可选：只同步兼容的
            if filter_compatible:
                compat = check_compatibility(skill, aid)
                if compat['score'] == 'F':
                    entry['targets'][aid] = {'ok': False, 'error': '不兼容', 'issues': compat['issues']}
                    continue

            target_path = get_target_path(aid, skill['name'])
            content = render_skill(skill, aid, target_path if not dry_run else None)

            errors = validate_yaml(content)
            if errors:
                entry['targets'][aid] = {'ok': False, 'error': '语法错误', 'details': errors}
                continue

            agent_result = {'ok': True, 'agent': agent['name'], 'path': str(target_path), 'action': 'dry-run' if dry_run else 'write'}

            if not dry_run:
                target_path.parent.mkdir(parents=True, exist_ok=True)
                if backup and target_path.exists():
                    bak = target_path.with_suffix(f'.bak.{int(time.time())}{target_path.suffix}')
                    shutil.copy2(target_path, bak)
                    agent_result['backup'] = str(bak)
                target_path.write_text(content, encoding='utf-8')

            entry['targets'][aid] = agent_result

        results.append(entry)

    summary = {
        'ok': True,
        'source_dir': source_dir,
        'targets': to,
        'total': len(results),
        'succeeded': sum(1 for r in results if all(t.get('ok') for t in r['targets'].values())),
        'partial': sum(1 for r in results if any(t.get('ok') for t in r['targets'].values()) and not all(t.get('ok') for t in r['targets'].values())),
        'failed': sum(1 for r in results if not any(t.get('ok') for t in r['targets'].values())),
        'results': results,
    }

    if json_output:
        print(json.dumps(summary, ensure_ascii=False, indent=2))
    else:
        print(f'\n📊 同步完成: {summary["succeeded"]} 完全成功 / {summary["partial"]} 部分成功 / {summary["failed"]} 失败')
        for r in results:
            ok_count = sum(1 for t in r['targets'].values() if t.get('ok'))
            total_count = len(r['targets'])
            status = '✅' if ok_count == total_count else '⚠️' if ok_count > 0 else '❌'
            print(f'  {status} {r["name"]:<25s} ({ok_count}/{total_count})')
            for aid, ar in r['targets'].items():
                if ar.get('ok'):
                    print(f'       ✅ {aid}: {ar["path"]}')
                else:
                    print(f'       ❌ {aid}: {ar.get("error", "")}')


# ═══════════════════════════════════════════════════════════
# 命令: watch  自动监控文件变化
# ═══════════════════════════════════════════════════════════

def cmd_watch(watch_dirs: list, to: list, backup: bool = False,
              daemon: bool = False, poll_interval: float = 2.0,
              json_output: bool = False):
    """监控目录变化，自动同步到目标 Agent"""

    if not to:
        config = load_config()
        to = config.get('default_targets', ['reasonix'])

    # 尝试 watchdog
    watchdog_available = False
    try:
        from watchdog.observers import Observer
        from watchdog.events import FileSystemEventHandler
        watchdog_available = True
    except ImportError:
        pass

    if daemon:
        # 后台模式：写入锁文件
        if LOCK_PATH.exists():
            print('❌ watcher 已在运行 (lock 文件存在)')
            return
        LOCK_PATH.write_text(str(os.getpid()), 'utf-8')

    print(f'👀 监控中: {", ".join(watch_dirs)} → {", ".join(to)}')
    print(f'  模式: {"watchdog" if watchdog_available else "polling (每 {poll_interval}s)"}')
    print(f'  备份: {"开启" if backup else "关闭"}')
    if watchdog_available:
        print('  Ctrl+C 停止')

    def sync_all():
        for wd in watch_dirs:
            d = Path(wd)
            if not d.exists():
                continue
            for f in d.glob('**/*.md'):
                _auto_sync(f, to, backup)
            for f in d.glob('**/*.mdc'):
                _auto_sync(f, to, backup)
            for sub in d.iterdir():
                if sub.is_dir():
                    sf = sub / 'SKILL.md'
                    if sf.exists():
                        _auto_sync(sf, to, backup)

    last_sync = {}

    def _auto_sync(file_path: Path, targets: list, do_backup: bool):
        """单个文件同步（带去重）"""
        key = str(file_path)
        mtime = file_path.stat().st_mtime
        last = last_sync.get(key)
        if last and mtime - last < 0.5:
            return  # 去重
        last_sync[key] = mtime

        try:
            text = file_path.read_text('utf-8')
        except:
            return

        fmt = detect_format(text, str(file_path))
        skill = parse_skill(text, fmt, str(file_path))
        name = skill['name']

        for aid in targets:
            agent = AGENTS.get(aid)
            if not agent:
                continue
            target_path = get_target_path(aid, name)
            content = render_skill(skill, aid, target_path)
            errors = validate_yaml(content)
            if errors:
                continue
            target_path.parent.mkdir(parents=True, exist_ok=True)
            if do_backup and target_path.exists():
                bak = target_path.with_suffix(f'.bak.{int(time.time())}{target_path.suffix}')
                shutil.copy2(target_path, bak)
            target_path.write_text(content, encoding='utf-8')
            ts = datetime.now().strftime('%H:%M:%S')
            print(f'  [{ts}] 🔄 {name} → {aid}')

    if watchdog_available:
        class SkillHandler(FileSystemEventHandler):
            def on_modified(self, event):
                if event.is_directory:
                    return
                p = Path(event.src_path)
                if p.suffix in ('.md', '.mdc') or p.name == 'SKILL.md':
                    _auto_sync(p, to, backup)

            def on_created(self, event):
                self.on_modified(event)

        observer = Observer()
        for wd in watch_dirs:
            d = Path(wd)
            if d.exists():
                observer.schedule(SkillHandler(), str(d), recursive=True)
        observer.start()
        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            observer.stop()
        observer.join()
    else:
        # Polling fallback
        known = {}
        try:
            while True:
                for wd in watch_dirs:
                    d = Path(wd)
                    if not d.exists():
                        continue
                    for pattern in ('**/*.md', '**/*.mdc'):
                        for f in d.glob(pattern):
                            key = str(f)
                            mtime = f.stat().st_mtime
                            if known.get(key) != mtime:
                                known[key] = mtime
                                _auto_sync(f, to, backup)
                time.sleep(poll_interval)
        except KeyboardInterrupt:
            pass

    if daemon and LOCK_PATH.exists():
        LOCK_PATH.unlink()


# ═══════════════════════════════════════════════════════════
# CLI 入口
# ═══════════════════════════════════════════════════════════

def main():
    if len(sys.argv) < 2 or sys.argv[1] in ('-h', '--help'):
        print(f'⭐ 万能 Skill 转换器 v{VERSION} — AI Agent Skill 生命周期管理')
        print(f'  支持 {len(AGENTS)} 种格式: {", ".join(AGENTS.keys())}')
        print()
        print('用法:')
        print(f'  install <file>   安装 skill 到指定 Agent (默认 {",".join(load_config()["default_targets"])})')
        print('  inspect <file>   查看 skill 信息 (格式/依赖/兼容性)')
        print('  check <file>     检测 skill 兼容性/依赖/可用性')
        print('  list             列出已安装 skills')
        print('  convert <file>   导出为指定格式')
        print('  install-dir <dir> 批量安装目录')
        print('  sync <dir>       双向同步')
        print('  watch <dir>      监控目录自动同步')
        print('  config           配置管理')
        print()
        print('新命令:')
        print('  --detect-agent   检测当前 Agent 环境')
        print('  --self-install   自安装为当前 Agent 的 skill')
        print('  --bootstrap      一键引导（检测→安装→报告）')
        print()
        print('通用参数:')
        print('  --to <a,b>   目标 Agent 列表')
        print('  --dry-run    预览不写入')
        print('  --backup     写入前备份')
        print('  --json       机器可读输出')
        print(f'支持 {len(AGENTS)} 种格式: {", ".join(AGENTS.keys())}')
        return

    command = sys.argv[1]
    args = sys.argv[2:]

    def has_flag(*names):
        return any(n in args for n in names)

    def get_flag(*names):
        for i, a in enumerate(args):
            if a in names and i + 1 < len(args):
                return args[i + 1]
        return None

    json_output = has_flag('--json', '-j')

    if command == 'install':
        source = args[0] if args and not args[0].startswith('--') else None
        if not source:
            print('❌ 请指定源文件')
            return
        to_raw = get_flag('--to', '-t')
        if to_raw:
            to = [t.strip() for t in to_raw.split(',')]
        else:
            # 自动检测环境
            env = detect_agent()
            cfg = load_config()
            to = env['formats'] if env['formats'] else cfg.get('default_targets', ['reasonix'])
        dry_run = has_flag('--dry-run', '-n')
        backup = has_flag('--backup', '-b')
        cmd_install(source, to, dry_run, backup, json_output=json_output)

    elif command == 'inspect':
        source = args[0] if args and not args[0].startswith('--') else None
        if not source:
            print('❌ 请指定源文件')
            return
        cmd_inspect(source, json_output)

    elif command == 'check':
        source = args[0] if args and not args[0].startswith('--') else None
        if not source:
            print('❌ 请指定源文件')
            return
        to_raw = get_flag('--to', '-t')
        to = [t.strip() for t in to_raw.split(',')] if to_raw else None
        cmd_check(source, to, json_output)

    elif command == 'list':
        agent = get_flag('--agent', '-a') or 'reasonix'
        cmd_list(agent, json_output=json_output)

    elif command == 'convert':
        source = args[0] if args and not args[0].startswith('--') else None
        to = get_flag('--to', '-t')
        if not source or not to:
            print('❌ 用法: python ua.py convert <file> --to <format>')
            return
        cmd_convert(source, to, json_output)

    elif command == 'install-dir':
        skill_dir = args[0] if args and not args[0].startswith('--') else None
        if not skill_dir:
            print('❌ 请指定目录路径')
            return
        to_raw = get_flag('--to', '-t')
        if to_raw:
            to = [t.strip() for t in to_raw.split(',')]
        else:
            env = detect_agent()
            cfg = load_config()
            to = env['formats'] if env['formats'] else cfg.get('default_targets', ['reasonix'])
        dry_run = has_flag('--dry-run', '-n')
        backup = has_flag('--backup', '-b')
        cmd_install_dir(skill_dir, to, dry_run, backup, json_output=json_output)

    elif command == 'sync':
        source_dir = args[0] if args and not args[0].startswith('--') else None
        if not source_dir:
            print('❌ 请指定源目录')
            return
        to_raw = get_flag('--to', '-t')
        to = [t.strip() for t in to_raw.split(',')] if to_raw else load_config().get('default_targets', ['reasonix'])
        dry_run = has_flag('--dry-run', '-n')
        backup = has_flag('--backup', '-b')
        filter_compat = has_flag('--filter-compatible', '-f')
        cmd_sync(source_dir, to, dry_run, backup, filter_compat, json_output)

    elif command == 'watch':
        watch_dirs = [a for a in args if not a.startswith('--')]
        if not watch_dirs:
            # 从配置读取
            config = load_config()
            watch_dirs = config.get('watch_dirs', ['.'])
        to_raw = get_flag('--to', '-t')
        to = [t.strip() for t in to_raw.split(',')] if to_raw else load_config().get('default_targets', ['reasonix'])
        backup = has_flag('--backup', '-b')
        daemon = has_flag('--daemon', '-d')
        cfg = load_config()
        cmd_watch(watch_dirs, to, backup, daemon, cfg.get('poll_interval', 2.0), json_output)

    elif command in ('detect-agent', '--detect-agent'):
        cmd_detect_agent(json_output)

    elif command in ('self-install', '--self-install'):
        cmd_self_install(json_output)

    elif command in ('bootstrap', '--bootstrap'):
        cmd_bootstrap(json_output)

    elif command == 'config':
        """管理持久化配置"""
        sub = args[0] if args else None
        cfg = load_config()
        if sub == 'show':
            print(json.dumps(cfg, ensure_ascii=False, indent=2))
        elif sub == 'set' and len(args) >= 3:
            key, val = args[1], args[2]
            if key in cfg:
                if isinstance(cfg[key], list):
                    cfg[key] = [v.strip() for v in val.split(',')]
                elif isinstance(cfg[key], bool):
                    cfg[key] = val.lower() in ('true', '1', 'yes')
                elif isinstance(cfg[key], (int, float)):
                    cfg[key] = type(cfg[key])(val)
                else:
                    cfg[key] = val
                save_config(cfg)
                print(f'✅ {key} = {json.dumps(cfg[key])}')
            else:
                print(f'❌ 未知配置项: {key}')
        elif sub == 'path' and len(args) >= 3:
            agent_id, path_val = args[1], args[2]
            overrides = cfg.setdefault('path_overrides', {})
            overrides[f'{agent_id}.path'] = path_val
            save_config(cfg)
            print(f'✅ {agent_id} 路径 → {path_val}')
        else:
            print('用法:')
            print('  python ua.py config show')
            print('  python ua.py config set <key> <value>')
            print('  python ua.py config path <agent_id> <path>')
            print(f'可用配置项: {", ".join(DEFAULT_CONFIG.keys())}')

    else:
        print(f'❌ 未知命令: {command}')
        print('可用命令: install, inspect, check, list, convert, install-dir, sync, watch, config')


if __name__ == '__main__':
    main()
