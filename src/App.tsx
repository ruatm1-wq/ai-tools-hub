import React, { useState, useEffect, useRef, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import ToolCallBlock from './components/ToolCallBlock';

const win = getCurrentWindow();

interface ToolConfig { id: string; name: string; icon: string; tool_type: string; enabled: boolean; provider: string; api_base: string; model_name: string; api_key: string; command: string; args: string[]; system_prompt: string; }
interface ChatMsg { role: string; content: string; toolId?: string; }
interface Session { id: string; tool_id: string; title: string; messages: ChatMsg[]; created: string; updated: string; }
interface SkillInfo { id: string; bundle_name: string; name: string; description: string; prompt: string; version: string; enabled: boolean; }
interface MemoryItem { id: number; tool_id: string; session_id: string | null; memory_type: string; content: string; keywords: string; entities: string; created_at: string; }
interface SearchResult { content: string; source: string; timestamp: string; }

interface ToolCallData {
  name: string;
  args?: Record<string, unknown>;
  result?: string;
  status: 'running' | 'success' | 'error';
}

const MAX_MEMORY_LEN = 500;
const AGENTS: { icon: string; name: string; desc: string }[] = [
  { icon: '⚡', name: '小拉', desc: 'Reasonix Code · 编码' },
  { icon: '🤖', name: '小马', desc: 'Hermes · 分析' },
  { icon: '🧠', name: 'DeepSeek', desc: '深度推理' },
];

function App() {
  const [theme, setTheme] = useState('dark');
  const [activeTool, setActiveTool] = useState('');
  const [input, setInput] = useState('');
  const [messagesMap, setMessagesMap] = useState<Record<string, ChatMsg[]>>({});
  const [sessionId, setSessionId] = useState('');
  const [tools, setTools] = useState<ToolConfig[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [attachedSkill, setAttachedSkill] = useState<SkillInfo | null>(null);
  const [searchEnabled, setSearchEnabled] = useState(false);
  const [memoryInject, setMemoryInject] = useState(true);
  const [memories, setMemories] = useState<MemoryItem[]>([]);
  const [activeToolCalls, setActiveToolCalls] = useState<ToolCallData[]>([]);
  const [toolHistory, setToolHistory] = useState<ToolCallData[][]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [showSessions, setShowSessions] = useState(false);
  const [showSkills, setShowSkills] = useState(false);
  const [editingTool, setEditingTool] = useState<ToolConfig | null>(null);
  const [loaded, setLoaded] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const streamRef = useRef('');

  const messages = messagesMap[activeTool] || [];
  const ct = tools.find(t => t.id === activeTool);

  useEffect(() => { document.documentElement.className = theme; }, [theme]);
  useEffect(() => { scrollRef.current?.scrollTo(0, scrollRef.current.scrollHeight); }, [messages, activeToolCalls]);

  useEffect(() => {
    invoke<ToolConfig[]>('get_tools').then(t => { setTools(t); if (t.length > 0 && !activeTool) setActiveTool(t[0].id); });
    invoke<SkillInfo[]>('get_skills_list').then(setSkills).catch(() => {});
    setLoaded(true);
  }, []);

  useEffect(() => {
    if (!activeTool) return;
    loadSessions();
    invoke<MemoryItem[]>('get_recent_memories', { toolId: activeTool }).then(setMemories).catch(() => {});
  }, [activeTool]);

  // Auto-save session
  useEffect(() => {
    if (!activeTool) return;
    const timer = setTimeout(async () => {
      const msgs = messagesMap[activeTool];
      if (!msgs || msgs.length === 0) return;
      const t = msgs[0]?.content?.slice(0, 30) || 'New';
      const now = new Date().toISOString();
      try { await invoke('save_session', { session: { id: sessionId || Date.now().toString(), tool_id: activeTool, title: t, messages: msgs, created: sessionId ? (sessions.find(s => s.id === sessionId)?.created || now) : now, updated: now } }); if (!sessionId) setSessionId(Date.now().toString()); } catch (e) {}
    }, 2000);
    return () => clearTimeout(timer);
  }, [messagesMap, activeTool]);

  const loadSessions = async () => { if (activeTool) setSessions(await invoke<Session[]>('list_sessions')); };
  const loadSession = (s: Session) => { setMessagesMap(p => ({...p, [activeTool]: s.messages})); setSessionId(s.id); setShowSessions(false); };
  const newSession = () => { setMessagesMap(p => ({...p, [activeTool]: []})); setSessionId(''); setToolHistory([]); };

  const handleSend = async () => {
    const q = input.trim(); if (!q || !ct) return;
    const userMsg: ChatMsg = { role: 'user', content: q };
    const cur = messagesMap[activeTool] || [];
    const newMsgs = [...cur, userMsg];
    setMessagesMap(p => ({...p, [activeTool]: newMsgs}));
    setInput('');
    streamRef.current = '';

    try { await invoke('push_context', { content: q }); } catch (e) {}

    try {
      let sp = attachedSkill
        ? `${ct.system_prompt}\n\n[Skill: ${attachedSkill.name}]\n${attachedSkill.prompt}`
        : ct.system_prompt;

      if (memoryInject && activeTool) {
        try { const r: SearchResult[] = await invoke('search_memory', { query: q, toolId: activeTool || null }); if (r.length) sp += '\n\n[Related Memories]\n' + r.slice(0,3).map((m,i) => `[Mem ${i+1}] ${m.content.slice(0,MAX_MEMORY_LEN)}`).join('\n') + '\n[/Related Memories]'; } catch (e) {}
      }
      if (searchEnabled) {
        try { const sr = await invoke<string>('browser_search', { query: q }); sp += `\n\n[Web Search]\n${sr.slice(0,1500)}\n[/Search]`; } catch (e) {}
      }
      try { const ctx: string[] = await invoke('peek_context'); if (ctx.length) sp += '\n\n[Context]\n' + ctx.slice(-3).join('\n') + '\n[/Context]'; } catch (e) {}

      const emptyMsg: ChatMsg = { role: 'assistant', content: '', toolId: activeTool };
      setMessagesMap(p => ({...p, [activeTool]: [...newMsgs, emptyMsg]}));

      // Tool call event listeners
      const unToolStart = await listen<any>('tool-call-start', ev => {
        const toolsData = ev.payload?.tools || [];
        setActiveToolCalls(toolsData.map((t: any) => ({
          name: t.name, args: t.args, status: 'running' as const,
        })));
      });
      const unToolEnd = await listen<any>('tool-call-end', ev => {
        const results = ev.payload?.results || [];
        setActiveToolCalls((prev: ToolCallData[]) => {
          const updated = prev.map((call, i) => ({
            ...call,
            status: results[i]?.success ? 'success' as const : 'error' as const,
            result: typeof results[i]?.content === 'string' ? results[i].content.slice(0, 1000) : JSON.stringify(results[i]),
          }));
          setToolHistory(h => [...h, updated]);
          return updated;
        });
      });

      invoke('chat_completion_stream', { toolId: activeTool, messages: newMsgs, systemPrompt: sp });
      const unChunk = await listen<string>('stream-chunk', ev => {
        streamRef.current += ev.payload;
        setMessagesMap(p => {
          const msgs = [...(p[activeTool] || [])];
          if (msgs.length) { const last = { ...msgs[msgs.length - 1] }; last.content += ev.payload; msgs[msgs.length - 1] = last; }
          return { ...p, [activeTool]: msgs };
        });
      });
      const unDone = await listen<string>('stream-done', async () => {
        unChunk(); unDone(); unToolStart(); unToolEnd();
        const c = streamRef.current;
        streamRef.current = '';
        setActiveToolCalls([]);
        if (c) {
          try {
            await invoke('save_memory', { toolId: activeTool, memoryType: 'fact', content: `Q: ${q}\nA: ${c}`.slice(0, 500), keywords: q.split(/\s+/).filter((w: string) => w.length > 1).slice(0, 5).join(',') });
          } catch (e) {}
          try { setMemories(await invoke<MemoryItem[]>('get_recent_memories', { toolId: activeTool })); } catch (e) {}
        }
      });
    } catch (e: any) {
      setMessagesMap(p => ({...p, [activeTool]: [...newMsgs, { role: 'assistant', content: 'Error: ' + e, toolId: activeTool }] }));
    }
  };

  const isStreaming = () => {
    const msgs = messagesMap[activeTool] || [];
    const last = msgs[msgs.length - 1];
    return last?.role === 'assistant' && last?.content === '' && activeToolCalls.length === 0;
  };

  const agentForTool = (toolId: string) => {
    const idx = tools.findIndex(t => t.id === toolId);
    return AGENTS[idx] || AGENTS[0];
  };

  if (!loaded) {
    return (
      <div className="h-screen flex items-center justify-center bg-bg-primary text-text-dim">
        <div className="text-center"><div className="text-2xl animate-pulse mb-2 text-accent">✦</div><div className="text-xs">Loading...</div></div>
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col overflow-hidden bg-bg-primary text-text-main select-none">
      {/* ── Title Bar ── */}
      <div className="flex items-center h-10 px-4 bg-bg-deep border-b border-border shrink-0" data-tauri-drag-region>
        <div className="flex items-center gap-2" data-tauri-drag-region>
          <span className="text-accent text-sm font-semibold tracking-wide">AI TOOLS HUB</span>
          <span className="text-text-dim text-[10px]">|</span>
          <span className="text-text-muted text-[11px]">{ct ? `${agentForTool(activeTool).icon} ${ct.name}` : 'No Tool'}</span>
        </div>
        <div className="flex-1" data-tauri-drag-region />
        <div className="flex items-center gap-1">
          <button onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')} className="w-7 h-7 flex items-center justify-center rounded text-text-dim hover:text-text-muted hover:bg-bg-hover text-xs">
            {theme === 'dark' ? '☀' : '☾'}
          </button>
          <button onClick={() => win.minimize()} className="w-7 h-7 flex items-center justify-center rounded text-text-dim hover:text-text-muted hover:bg-bg-hover text-xs">─</button>
          <button onClick={() => win.toggleMaximize()} className="w-7 h-7 flex items-center justify-center rounded text-text-dim hover:text-text-muted hover:bg-bg-hover text-xs">□</button>
          <button onClick={() => win.close()} className="w-7 h-7 flex items-center justify-center rounded text-text-dim hover:text-text-muted hover:bg-bg-hover text-xs hover:bg-red-500/20 hover:text-red-400">✕</button>
        </div>
      </div>

      {/* ── Main Content ── */}
      <div className="flex-1 flex min-h-0">
        {/* Sidebar */}
        <div className="w-52 flex flex-col bg-bg-secondary border-r border-border shrink-0">
          <div className="px-3 pt-3 pb-2 text-[10px] font-semibold text-text-dim uppercase tracking-wider">Agents</div>
          <div className="flex-1 px-2 overflow-y-auto space-y-0.5">
            {tools.filter(t => t.enabled).map((t, i) => {
              const agent = AGENTS[i] || { icon: '🤖', name: t.name, desc: t.tool_type };
              return (
                <button key={t.id} onClick={() => setActiveTool(t.id)}
                  className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-colors ${
                    activeTool === t.id ? 'bg-bg-hover text-text-main' : 'text-text-muted hover:text-text-main hover:bg-bg-hover'
                  }`}>
                  <span className="text-lg shrink-0">{agent.icon}</span>
                  <div className="min-w-0">
                    <div className="text-sm font-medium truncate">{agent.name}</div>
                    <div className="text-[10px] text-text-dim truncate">{agent.desc}</div>
                  </div>
                  {activeTool === t.id && <span className="w-1 h-1 rounded-full bg-accent ml-auto shrink-0" />}
                </button>
              );
            })}
            {tools.filter(t => t.enabled).length === 0 && (
              <div className="text-xs text-text-dim text-center py-4">No agents configured</div>
            )}
          </div>
          <div className="border-t border-border p-2 space-y-0.5">
            <button onClick={() => loadSessions().then(() => setShowSessions(true))} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-text-muted hover:text-text-main hover:bg-bg-hover transition-colors">
              <span className="text-xs">⌛</span> History
            </button>
            <button onClick={() => { setEditingTool(null); setShowSettings(true); }} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-text-muted hover:text-text-main hover:bg-bg-hover transition-colors">
              <span className="text-xs">⚙</span> Settings
            </button>
          </div>
        </div>

        {/* Chat Area */}
        <div className="flex-1 flex flex-col min-w-0 bg-bg-primary">
          {/* Empty State */}
          {messages.length === 0 && !isStreaming() && (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center space-y-3">
                <div className="text-3xl opacity-20">{ct ? agentForTool(activeTool).icon : '✦'}</div>
                <div className="text-text-muted text-sm">{ct ? `Chat with ${agentForTool(activeTool).name}` : 'No agent selected'}</div>
                <div className="text-text-dim text-[10px]">Ask anything — I'll use tools to help</div>
              </div>
            </div>
          )}

          {/* Messages */}
          {(messages.length > 0 || isStreaming()) && (
            <div ref={scrollRef} className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
              {messages.map((msg, i) => (
                <div key={i} className={`flex items-start gap-3 ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
                  {msg.role !== 'user' && (
                    <div className="w-7 h-7 rounded-lg bg-bg-tertiary flex items-center justify-center text-sm shrink-0 mt-0.5">
                      {agentForTool(msg.toolId || activeTool).icon}
                    </div>
                  )}
                  <div className={`max-w-[70%] rounded-xl px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap ${
                    msg.role === 'user'
                      ? 'bg-accent-muted border border-accent/10 rounded-tr-md'
                      : 'bg-bg-secondary border border-border rounded-tl-md'
                  }`}>
                    <div className="text-[10px] text-text-dim mb-1.5">
                      {msg.role === 'user' ? 'You' : agentForTool(msg.toolId || activeTool).name}
                    </div>
                    {msg.content || (msg.role === 'assistant' ? <span className="text-text-dim animate-pulse">...</span> : '')}
                    {msg.role === 'assistant' && activeToolCalls.length > 0 && !msg.content && (
                      <ToolCallBlock calls={activeToolCalls} />
                    )}
                    {msg.role === 'assistant' && msg.content && (
                      <div className="mt-2 pt-2 border-t border-border/50">
                        <button onClick={() => navigator.clipboard.writeText(msg.content)} className="text-[10px] text-text-dim hover:text-text-muted transition-colors">
                          📋 Copy
                        </button>
                      </div>
                    )}
                  </div>
                  {msg.role === 'user' && (
                    <div className="w-7 h-7 rounded-lg bg-accent/10 flex items-center justify-center text-sm shrink-0 mt-0.5 border border-accent/10">You</div>
                  )}
                </div>
              ))}
              {/* Standalone tool call block */}
              {activeToolCalls.length > 0 && messages.length > 0 && messages[messages.length - 1]?.content !== '' && (
                <div className="flex justify-start">
                  <div className="w-7 h-7 rounded-lg bg-bg-tertiary flex items-center justify-center text-sm shrink-0 mt-0.5 mr-3">
                    {agentForTool(activeTool).icon}
                  </div>
                  <div className="max-w-[70%] rounded-xl bg-bg-secondary border border-border p-3">
                    <div className="text-[10px] text-text-dim mb-1.5">{agentForTool(activeTool).name} · Tools</div>
                    <ToolCallBlock calls={activeToolCalls} />
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Input */}
          <div className="px-4 py-3 border-t border-border bg-bg-primary">
            <div className="flex items-end gap-2 max-w-4xl mx-auto">
              <textarea ref={inputRef} value={input} onChange={e => setInput(e.target.value)}
                onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
                placeholder={`Message ${ct ? agentForTool(activeTool).name : '...'}`}
                className="flex-1 bg-bg-tertiary border border-border rounded-xl px-4 py-2.5 text-sm text-text-main placeholder-text-dim resize-none focus:outline-none focus:border-accent/40 transition-colors"
                rows={1} style={{ minHeight: '42px', maxHeight: '120px' }}
              />
              <div className="flex items-center gap-1 pb-1">
                <button onClick={() => setMemoryInject(!memoryInject)}
                  className={`w-8 h-8 flex items-center justify-center rounded-lg text-xs transition-colors ${memoryInject ? 'text-accent bg-accent-muted' : 'text-text-dim hover:text-text-muted hover:bg-bg-hover'}`}
                  title="Memory">🧠</button>
                <button onClick={() => setSearchEnabled(!searchEnabled)}
                  className={`w-8 h-8 flex items-center justify-center rounded-lg text-xs transition-colors ${searchEnabled ? 'text-accent bg-accent-muted' : 'text-text-dim hover:text-text-muted hover:bg-bg-hover'}`}
                  title="Web Search">🌐</button>
                <button onClick={handleSend} disabled={!input.trim() || !ct}
                  className="w-8 h-8 flex items-center justify-center rounded-lg bg-accent text-white hover:bg-accent-hover disabled:opacity-30 transition-colors text-sm">
                  ↵
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* Right Panel */}
        <div className="w-52 flex flex-col bg-bg-secondary border-l border-border shrink-0">
          {/* Active Tool Calls */}
          {toolHistory.length > 0 && (
            <div className="p-3 border-b border-border">
              <div className="text-[10px] font-semibold text-text-dim uppercase tracking-wider mb-2">Tool Calls</div>
              <div className="space-y-1 max-h-28 overflow-y-auto">
                {toolHistory.slice(-5).reverse().map((calls, gi) => (
                  <div key={gi} className="text-[10px] text-text-muted border-l border-accent/30 pl-2 py-0.5">
                    {calls.map((c, ci) => (
                      <span key={ci}>{c.name}{ci < calls.length - 1 ? ', ' : ''}</span>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Memory Preview */}
          <div className="flex-1 p-3 overflow-y-auto">
            <div className="text-[10px] font-semibold text-text-dim uppercase tracking-wider mb-2">Memory</div>
            <div className="space-y-1">
              {memories.slice(0, 6).map(m => (
                <div key={m.id} className="text-[10px] text-text-muted border-l border-accent/20 pl-2 py-0.5">
                  <span className="text-text-dim text-[9px]">{m.created_at?.slice(5, 10)}</span>
                  {' '}{m.content.slice(0, 60)}...
                </div>
              ))}
              {memories.length === 0 && <div className="text-[10px] text-text-dim">No memories yet</div>}
            </div>
          </div>

          {/* Quick Actions */}
          <div className="p-3 border-t border-border space-y-1">
            <button onClick={newSession} className="w-full text-left px-3 py-1.5 rounded-lg text-xs text-text-muted hover:text-text-main hover:bg-bg-hover transition-colors">
              ✦ New Chat
            </button>
          </div>
        </div>
      </div>

      {/* ── Status Bar ── */}
      <div className="h-6 flex items-center justify-between px-4 bg-bg-deep border-t border-border shrink-0 text-[10px] text-text-dim">
        <div className="flex items-center gap-2">
          <span className={`w-1.5 h-1.5 rounded-full ${isStreaming() ? 'bg-accent animate-pulse' : 'bg-green-500'}`} />
          <span>{ct ? `${agentForTool(activeTool).icon} ${ct.name}` : 'No tool'} · {isStreaming() ? 'Streaming' : 'Ready'}</span>
        </div>
        <div className="flex items-center gap-3">
          <span>Memory {memoryInject ? 'ON' : 'OFF'}</span>
          <span>Search {searchEnabled ? 'ON' : 'OFF'}</span>
        </div>
      </div>

      {/* ── Modals ── */}
      {showSessions && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setShowSessions(false)}>
          <div className="bg-bg-secondary rounded-xl w-[380px] max-h-[70vh] shadow-2xl border border-border overflow-y-auto" onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between px-4 py-3 border-b border-border">
              <h2 className="text-sm font-semibold">History</h2>
              <button onClick={() => setShowSessions(false)} className="text-text-dim hover:text-text-muted text-xs">✕</button>
            </div>
            <div className="p-3 space-y-1">
              {sessions.length === 0 ? (
                <div className="text-xs text-text-dim text-center py-8">No sessions yet</div>
              ) : sessions.map(s => (
                <div key={s.id} className="flex items-center gap-3 px-3 py-2.5 rounded-lg cursor-pointer hover:bg-bg-hover transition-colors" onClick={() => loadSession(s)}>
                  <div className="flex-1 min-w-0">
                    <div className="text-xs font-medium truncate">{s.title}</div>
                    <div className="text-[10px] text-text-dim">{s.messages.length} messages</div>
                  </div>
                  <button onClick={e => { e.stopPropagation(); invoke('delete_session', { sessionId: s.id }).then(loadSessions); }} className="text-text-dim hover:text-red-400 text-xs transition-colors shrink-0">✕</button>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {showSettings && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setShowSettings(false)}>
          <div className="bg-bg-secondary rounded-xl w-[480px] max-h-[80vh] shadow-2xl border border-border overflow-y-auto" onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between px-4 py-3 border-b border-border">
              <h2 className="text-sm font-semibold">Settings</h2>
              <button onClick={() => setShowSettings(false)} className="text-text-dim hover:text-text-muted text-xs">✕</button>
            </div>
            <div className="p-4 space-y-4">
              {tools.map((t, i) => {
                const agent = AGENTS[i] || { icon: '🤖', name: t.name, desc: '' };
                return (
                  <div key={t.id} className="bg-bg-tertiary rounded-lg p-3">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="text-lg">{agent.icon}</span>
                      <span className="text-sm font-medium">{agent.name}</span>
                      <span className="text-[10px] text-text-dim">{t.tool_type === 'api' ? t.provider : 'CLI'}</span>
                    </div>
                    {t.tool_type === 'api' && (
                      <div className="space-y-1.5 text-xs">
                        <div className="flex justify-between"><span className="text-text-dim">API:</span><span className="text-text-muted truncate ml-2">{t.api_base}</span></div>
                        <div className="flex justify-between"><span className="text-text-dim">Model:</span><span className="text-text-muted">{t.model_name}</span></div>
                        <div className="flex justify-between"><span className="text-text-dim">Key:</span><span className="text-text-muted">{t.api_key ? '••••' + t.api_key.slice(-4) : 'Not set'}</span></div>
                      </div>
                    )}
                    {t.tool_type === 'process' && (
                      <div className="space-y-1.5 text-xs">
                        <div className="flex justify-between"><span className="text-text-dim">Cmd:</span><span className="text-text-muted truncate ml-2">{t.command}</span></div>
                      </div>
                    )}
                  </div>
                );
              })}
              <div className="pt-2 text-xs text-text-dim text-center">Manage API keys in %APPDATA%/ai-tools-hub/tools.json</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
