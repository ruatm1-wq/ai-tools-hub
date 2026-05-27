import React, { useState, useEffect, useRef, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { Moon, Sun, Send, Minus, Square, X, Copy, Plus, Trash2, Globe, History, Brain, Link2 } from 'lucide-react';
import ToolCallBlock from './components/ToolCallBlock';

const win = getCurrentWindow();

interface ToolConfig { id: string; name: string; icon: string; tool_type: string; enabled: boolean; provider: string; api_base: string; model_name: string; api_key: string; command: string; args: string[]; system_prompt: string; }
interface ChatMsg { role: string; content: string; toolId?: string; }
interface Session { id: string; tool_id: string; title: string; messages: ChatMsg[]; created: string; updated: string; }
interface SkillInfo { id: string; bundle_name: string; name: string; description: string; prompt: string; version: string; enabled: boolean; }
interface MemoryItem { id: number; tool_id: string; session_id: string | null; memory_type: string; content: string; keywords: string; entities: string; created_at: string; }
interface SearchResult { content: string; source: string; timestamp: string; }
const MAX_MEMORY_LEN = 500;
const EMPTY_TOOL: ToolConfig = { id: '', name: '', icon: '\u{1F916}', tool_type: 'api', enabled: true, provider: 'deepseek', api_base: 'https://api.deepseek.com', model_name: 'deepseek-chat', api_key: '', command: '', args: [], system_prompt: '' };

function App() {
  const [theme, setTheme] = useState('dark');
  const [activeTool, setActiveTool] = useState('');
  const [input, setInput] = useState('');
  const [messagesMap, setMessagesMap] = useState<Record<string, ChatMsg[]>>({});
  const [sessionId, setSessionId] = useState('');
  const [showSettings, setShowSettings] = useState(false);
  const [showSkills, setShowSkills] = useState(false);
  const [showSessions, setShowSessions] = useState(false);
  const [showMemory, setShowMemory] = useState(false);
  const [showBuffer, setShowBuffer] = useState(false);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [attachedSkill, setAttachedSkill] = useState<SkillInfo | null>(null);
  const [searchEnabled, setSearchEnabled] = useState(false);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [tools, setTools] = useState<ToolConfig[]>([]);
  const [editingTool, setEditingTool] = useState<ToolConfig>({...EMPTY_TOOL});
  const [showEditor, setShowEditor] = useState(false);
  const [autoStart, setAutoStart] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [deleteTargetId, setDeleteTargetId] = useState('');
  const [showTerminal, setShowTerminal] = useState(false);
  const [termInput, setTermInput] = useState('');
  const [termHistory, setTermHistory] = useState<string[]>(['AI Hub Terminal', 'Type commands', '']);
  const [memoryInject, setMemoryInject] = useState(true);
  const [memories, setMemories] = useState<MemoryItem[]>([]);
  const [memoryQuery, setMemoryQuery] = useState('');
  const [memoryResults, setMemoryResults] = useState<SearchResult[]>([]);
  const [contextItems, setContextItems] = useState<string[]>([]);
  const [ptyTestRunning, setPtyTestRunning] = useState(false);
  const [ptyTestOutput, setPtyTestOutput] = useState<string[]>([]);
  const [ptyTestInput, setPtyTestInput] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const termRef = useRef<HTMLDivElement>(null);
  const ptyTestRef = useRef<HTMLDivElement>(null);
  const streamRef = useRef('');
  const [activeToolCalls, setActiveToolCalls] = useState<ToolCallData[]>([]);

  interface ToolCallData {
    name: string;
    args?: Record<string, unknown>;
    result?: string;
    status: 'running' | 'success' | 'error';
  }

  const messages = messagesMap[activeTool] || [];
  const ct = tools.find(t => t.id === activeTool);

  useEffect(() => { document.documentElement.className = theme; }, [theme]);
  useEffect(() => { scrollRef.current?.scrollTo(0, scrollRef.current.scrollHeight); }, [messages]);
  useEffect(() => { termRef.current?.scrollTo(0, termRef.current.scrollHeight); }, [termHistory]);
  useEffect(() => { ptyTestRef.current?.scrollTo(0, ptyTestRef.current.scrollHeight); }, [ptyTestOutput]);
  useEffect(() => { if (activeTool) { loadSessions(); refreshMemories(); } }, [activeTool]);

  useEffect(() => {
    invoke<ToolConfig[]>('get_tools').then(t => { setTools(t); if (t.length > 0 && !activeTool) setActiveTool(t[0].id); });
    invoke<SkillInfo[]>('get_skills_list').then(setSkills).catch(() => {});
    invoke<boolean>('check_auto_start').then(setAutoStart).catch(() => {});
    setLoaded(true);
  }, []);

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

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'n') { e.preventDefault(); newSession(); }
      if ((e.ctrlKey || e.metaKey) && e.key === 'e') { e.preventDefault(); inputRef.current?.focus(); }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [activeTool]);

  const loadSessions = async () => { if (activeTool) setSessions(await invoke<Session[]>('list_sessions')); };
  const refreshMemories = async () => { if (activeTool) setMemories(await invoke<MemoryItem[]>('get_recent_memories', { toolId: activeTool })); };
  const searchMemories = async () => { if (!memoryQuery.trim()) { refreshMemories(); return; } setMemoryResults(await invoke<SearchResult[]>('search_memory', { query: memoryQuery, toolId: activeTool || null })); };
  const loadSession = (s: Session) => { setMessagesMap(p => ({...p, [activeTool]: s.messages})); setSessionId(s.id); setShowSessions(false); };
  const newSession = () => { setMessagesMap(p => ({...p, [activeTool]: []})); setSessionId(''); };
  const confirmDelete = (id: string) => setDeleteTargetId(id);
  const saveOneFact = async (toolId: string, q: string, a: string) => { try { await invoke('save_memory', { toolId, memoryType: 'fact', content: `Q: ${q}\nA: ${a}`.slice(0,500), keywords: q.split(/\s+/).filter(w => w.length > 1).slice(0,5).join(',') }); } catch (e) {} };
  const toolById = useCallback((id?: string) => tools.find(t => t.id === id), [tools]);

  const handleSend = async () => {
    const q = input.trim(); if (!q || !ct) return;
    const userMsg: ChatMsg = { role: 'user', content: q }; const cur = messagesMap[activeTool] || []; const newMsgs = [...cur, userMsg];
    setMessagesMap(p => ({...p, [activeTool]: newMsgs})); setInput(''); streamRef.current = '';
    try { await invoke('push_context', { content: q }); } catch (e) {}
    try {
      let sp = attachedSkill ? `${ct.system_prompt}\n\n[Skill: ${attachedSkill.name}]\n${attachedSkill.prompt}` : ct.system_prompt;
      if (memoryInject && activeTool) { try { const r: SearchResult[] = await invoke('search_memory', { query: q, toolId: activeTool || null }); if (r.length) sp += '\n\n[Related Memories]\n' + r.slice(0,3).map((m,i) => `[Mem ${i+1}] ${m.content.slice(0,MAX_MEMORY_LEN)}`).join('\n') + '\n[/Related Memories]'; } catch (e) {} }
      if (searchEnabled) { try { const sr = await invoke<string>('browser_search', { query: q }); sp += `\n\n[Web Search]\n${sr.slice(0,1500)}\n[/Search]`; } catch (e) {} }
      try { const ctx: string[] = await invoke('peek_context'); if (ctx.length) sp += '\n\n[Context]\n' + ctx.slice(-3).join('\n') + '\n[/Context]'; } catch (e) {}
      const emptyMsg: ChatMsg = { role: 'assistant', content: '', toolId: activeTool };
      setMessagesMap(p => ({...p, [activeTool]: [...newMsgs, emptyMsg]}));
      // Tool call event listeners
      const unToolStart = await listen<any>('tool-call-start', ev => {
        const tools = ev.payload?.tools || [];
        setActiveToolCalls(tools.map((t: any) => ({
          name: t.name,
          args: t.args,
          status: 'running' as const,
        })));
      });
      const unToolEnd = await listen<any>('tool-call-end', ev => {
        const results = ev.payload?.results || [];
        setActiveToolCalls((prev: ToolCallData[]) => prev.map((call, i) => ({
          ...call,
          status: results[i]?.success ? 'success' as const : 'error' as const,
          result: typeof results[i]?.content === 'string' ? results[i].content.slice(0, 1000) : JSON.stringify(results[i]),
        })));
      });

      invoke('chat_completion_stream', { toolId: activeTool, messages: newMsgs, systemPrompt: sp });
      const unChunk = await listen<string>('stream-chunk', ev => { streamRef.current += ev.payload; setMessagesMap(p => { const msgs = [...(p[activeTool]||[])]; if (msgs.length) { const last = {...msgs[msgs.length-1]}; last.content += ev.payload; msgs[msgs.length-1] = last; } return {...p, [activeTool]: msgs}; }); });
      const unDone = await listen<string>('stream-done', async () => { unChunk(); unDone(); unToolStart(); unToolEnd(); const c = streamRef.current; streamRef.current = ''; setActiveToolCalls([]); if (c) { saveOneFact(activeTool, q, c); try { setContextItems(await invoke<string[]>('peek_context')); refreshMemories(); } catch(e) {} } });
    } catch (e: any) { setMessagesMap(p => ({...p, [activeTool]: [...newMsgs, { role: 'assistant', content: 'Error: ' + e, toolId: activeTool }] })); }
  };

  const runTerminal = async () => { const cmd = termInput.trim(); if (!cmd) return; setTermHistory(p => [...p, '> ' + cmd]); setTermInput(''); try { const r = await invoke<string>('run_shell_command', { command: cmd }); setTermHistory(p => [...p, r || '(no output)', '']); } catch (e: any) { setTermHistory(p => [...p, 'Error: ' + e, '']); } };

  const saveTool = async (tool: ToolConfig) => { const idx = tools.findIndex(t => t.id === tool.id); let newTools: ToolConfig[]; if (idx >= 0) { newTools = [...tools]; newTools[idx] = tool; } else { tool.id = 'tool_' + Date.now(); newTools = [...tools, tool]; } await invoke('save_tools', { tools: newTools }); setTools(newTools); setShowEditor(false); setActiveTool(tool.id); };
  const deleteToolConfirm = async (id: string) => { const newTools = tools.filter(t => t.id !== id); await invoke('save_tools', { tools: newTools }); setTools(newTools); if (activeTool === id) setActiveTool(newTools[0]?.id || ''); setDeleteTargetId(''); };
  const isStreaming = () => { const msgs = messagesMap[activeTool]||[]; const last = msgs[msgs.length-1]; return last?.role === 'assistant' && last?.content === ''; };

  // PTY Test commands
  const startPtyTest = async () => {
    const cmd = ptyTestInput.trim(); if (!cmd) return;
    setPtyTestRunning(true); setPtyTestOutput(p => [...p, `$ ${cmd}`]);
    // Parse command into cmd + args (split by space)
    let parts = cmd.match(/(?:[^\s"]+|"[^"]*")+/g) || [cmd];
    let command = parts[0];
    let args = parts.slice(1).map(a => a.replace(/^"|"$/g, ''));
    try {
      // Windows: npx/npm need cmd.exe wrapper
    if(command==='npx'||command==='npm'||command==='node'){args=[command].concat(args);command='cmd.exe';}
    await invoke('start_pty_test', { command, args });
      // Wait a bit then read output
      setTimeout(async () => {
        try { const r = await invoke<string>('pty_test_send', { input: '' }); if (r) setPtyTestOutput(p => [...p, r]); } catch(e) {}
      }, 500);
    } catch (e: any) { setPtyTestOutput(p => [...p, `Error: ${e}`]); }
    setPtyTestInput('');
  };
  const sendPtyTest = async () => {
    const msg = ptyTestInput.trim(); if (!msg) return;
    setPtyTestOutput(p => [...p, `> ${msg}`]); setPtyTestInput('');
    try { const r = await invoke<string>('pty_test_send', { input: msg }); if (r) setPtyTestOutput(p => [...p, r]); } catch (e: any) { setPtyTestOutput(p => [...p, `Error: ${e}`]); }
  };
  const stopPtyTest = async () => {
    try { await invoke('pty_test_stop'); } catch(e) {}
    setPtyTestRunning(false);
  };
  const savePtyTool = async (name: string) => {
    let parts = ptyTestOutput[0]?.replace(/^\$\s*/, '').match(/(?:[^\s"]+|"[^"]*")+/g) || [];
    let command = parts[0] || "";
    let args = parts.slice(1).map(a => a.replace(/^"|"$/g, ''));
    const icon = editingTool.icon || '\u26A1';
    try {
      // Windows: npx/npm need cmd.exe wrapper
    if(command==='npx'||command==='npm'||command==='node'){args=[command].concat(args);command='cmd.exe';}
    await invoke('save_pty_tool', { name, icon, command, args });
      const toolsList = await invoke<ToolConfig[]>('get_tools');
      setTools(toolsList); setShowEditor(false); setActiveTool(toolsList[toolsList.length-1].id);
    } catch (e: any) { alert('Save failed: '+e); }
  };

  if (!loaded) return <div className="h-screen flex items-center justify-center bg-[#1f1f1f] text-[#888]"><div className="text-center"><div className="text-2xl animate-pulse mb-2">+</div><div className="text-sm">Loading...</div></div></div>;

  return (
    <div className="h-screen flex flex-col overflow-hidden bg-bg-primary text-text-main">
      {/* Title bar */}
      <div className="flex items-center h-10 bg-bg-deep border-b border-border select-none" data-tauri-drag-region>
        <div className="flex items-center gap-2 ml-4 text-xs text-text-dim" data-tauri-drag-region>
          <span className="text-accent font-semibold">AI TOOLS HUB</span>
          <span className="opacity-30">|</span><span>{ct?.icon}</span><span className="text-text-muted">{ct?.name}</span>
        </div>
        <div className="flex-1" data-tauri-drag-region />
        <div className="flex items-center h-full">
          <button onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')} className="w-10 h-full flex items-center justify-center text-text-dim hover:bg-bg-hover">{theme === 'dark' ? <Sun size={13} /> : <Moon size={13} />}</button>
          <button onClick={() => win.minimize()} className="w-11 h-full flex items-center justify-center text-text-dim hover:bg-bg-hover"><Minus size={13} /></button>
          <button onClick={() => win.toggleMaximize()} className="w-11 h-full flex items-center justify-center text-text-dim hover:bg-bg-hover"><Square size={11} /></button>
          <button onClick={() => win.close()} className="w-11 h-full flex items-center justify-center text-text-dim hover:bg-red-500/90 hover:text-white"><X size={14} /></button>
        </div>
      </div>

      <div className="flex-1 flex min-h-0">
        {/* Sidebar */}
        <div className="w-52 bg-bg-deep border-r border-border flex flex-col">
          <div className="flex-1 p-2.5 space-y-0.5 overflow-y-auto">
            <button onClick={newSession} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-accent hover:bg-accent-muted mb-1"><Plus size={14} /><span>New Ctrl+N</span></button>
            {tools.filter(t => t.enabled).map(t => (
              <div key={t.id} className="group relative">
                <button onClick={() => setActiveTool(t.id)} className={"w-full flex items-center gap-2.5 px-3 py-2.5 rounded-lg text-sm " + (activeTool === t.id ? "bg-accent-muted text-text-main" : "text-text-muted hover:bg-bg-hover")}>
                  <span>{t.icon}</span>
                  <div className="flex-1 text-left"><div className="text-[13px]">{t.name}</div><div className="text-[10px] text-text-dim">{t.tool_type === 'api' ? t.provider : t.command}</div></div>
                  <button onClick={(e) => { e.stopPropagation(); confirmDelete(t.id); }} className="opacity-0 group-hover:opacity-100 text-text-dim hover:text-red-400"><Trash2 size={12} /></button>
                </button>
              </div>
            ))}
            <button onClick={() => { setEditingTool({...EMPTY_TOOL}); setPtyTestRunning(false); setPtyTestOutput([]); setShowEditor(true); }} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-accent hover:bg-accent-muted mt-1"><Plus size={14} /><span>Add Tool</span></button>
          </div>
          <div className="border-t border-border p-2.5 space-y-0.5">
            <button onClick={() => loadSessions().then(() => setShowSessions(true))} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-text-muted hover:bg-bg-hover"><History size={12} /><span>History</span></button>
            <button onClick={() => { refreshMemories(); setShowMemory(true); }} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-text-muted hover:bg-bg-hover"><Brain size={12} /><span>Memory</span></button>
            <button onClick={() => setShowSkills(true)} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-text-muted hover:bg-bg-hover"><span>#</span><span>Skills</span></button>
            <button onClick={() => setShowSettings(true)} className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs text-text-muted hover:bg-bg-hover"><span>*</span><span>Settings</span></button>
            <button onClick={() => setShowTerminal(!showTerminal)} className={"w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs " + (showTerminal ? "bg-accent-muted text-accent" : "text-text-muted hover:bg-bg-hover")}><span>$</span><span>Terminal</span></button>
          </div>
        </div>

        {/* Chat area */}
        <div className="flex-1 flex flex-col min-w-0">
          <div ref={scrollRef} className="flex-1 overflow-y-auto px-6 py-4 space-y-4">
            {messages.length === 0 && ct && (
              <div className="h-full flex items-center justify-center text-text-dim text-xs">
                <div className="text-center space-y-2">
                  <div className="text-3xl opacity-30">{ct.icon}</div>
                  <div className="text-text-muted text-sm">Chat with <span className="text-accent">{ct.name}</span></div>
                  <div className="text-[10px] text-text-dim">Streaming | Memory | Web Search</div>
                </div>
              </div>
            )}
            {messages.map((msg, i) => (
              <div key={i} className={"flex items-start gap-3 " + (msg.role === 'user' ? 'justify-end' : 'justify-start')}>
                <div className={"max-w-[70%] rounded-xl px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap " + (msg.role === 'user' ? 'bg-accent-muted border border-accent/20' : 'bg-bg-tertiary')}>
                  <div className="text-[10px] text-text-dim mb-1">{msg.role === 'user' ? 'You' : (toolById(msg.toolId)?.name || 'AI')}</div>
                  {msg.content || (msg.role === 'assistant' ? <span className="text-text-dim animate-pulse">...</span> : '')}
                  {/* Show tool call status inside assistant bubble when streaming */}
                  {msg.role === 'assistant' && activeToolCalls.length > 0 && !msg.content && (
                    <ToolCallBlock calls={activeToolCalls} />
                  )}
                  {msg.role === 'assistant' && msg.content && (
                    <div className="mt-2 pt-2 border-t border-border">
                      <button onClick={() => navigator.clipboard.writeText(msg.content)} className="flex items-center gap-1 text-xs text-text-dim hover:text-text-muted"><Copy size={11} /> Copy</button>
                    </div>
                  )}
                </div>
              </div>
            ))}
            {/* Standalone tool call block when not inside a message bubble */}
            {activeToolCalls.length > 0 && messages.length > 0 && messages[messages.length-1]?.content !== '' && (
              <div className="flex justify-start">
                <div className="max-w-[70%] rounded-xl px-4 py-3 bg-bg-tertiary">
                  <div className="text-[10px] text-text-dim mb-1">AI Tools</div>
                  <ToolCallBlock calls={activeToolCalls} />
                </div>
              </div>
            )}
          </div>
          <div className="p-4">
            <div className="relative">
              <textarea ref={inputRef} value={input} onChange={e => setInput(e.target.value)}
                onKeyDown={e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
                placeholder="Type a message... Ctrl+E" className="w-full bg-bg-tertiary border border-border rounded-xl px-4 py-3 pr-24 text-sm text-text-main placeholder-text-dim resize-none focus:outline-none focus:border-accent/50" rows={1} style={{ minHeight: '48px' }} />
              <div className="absolute right-2.5 bottom-2.5 flex items-center gap-1">
                <button onClick={() => setMemoryInject(!memoryInject)} className={'p-1.5 rounded ' + (memoryInject ? 'text-accent bg-accent-muted' : 'text-text-dim')} title="Memory"><Brain size={14} /></button>
                <button onClick={() => setSearchEnabled(!searchEnabled)} className={'p-1.5 rounded ' + (searchEnabled ? 'text-accent bg-accent-muted' : 'text-text-dim')} title="Web Search"><Globe size={14} /></button>
                <button onClick={() => setShowBuffer(true)} className="p-1.5 rounded text-text-dim" title="Context Relay"><Link2 size={14} /></button>
                <button onClick={handleSend} disabled={!input.trim() || !ct} className="p-1.5 rounded-lg bg-accent text-white hover:bg-accent-hover disabled:opacity-30"><Send size={14} /></button>
              </div>
            </div>
          </div>
        </div>

        {/* Right panel */}
        <div className="w-48 bg-bg-deep border-l border-border p-3 flex flex-col gap-3 text-xs">
          <div className="bg-bg-secondary rounded-lg p-3">
            <div className="text-[10px] font-semibold text-text-dim mb-2 uppercase">STATUS</div>
            <div className="space-y-1.5">
              <div className="flex justify-between"><span className="text-text-dim">Tool</span><span className="text-accent">{ct?.name||'-'}</span></div>
              <div className="flex justify-between"><span className="text-text-dim">Search</span><span>{searchEnabled ? 'On' : 'Off'}</span></div>
              <div className="flex justify-between"><span className="text-text-dim">Memory</span><span>{memoryInject ? 'On' : 'Off'}</span></div>
              <div className="flex justify-between"><span className="text-text-dim">Stream</span><span className="text-accent">On</span></div>
            </div>
          </div>
          <div className="bg-bg-secondary rounded-lg p-3">
            <div className="text-[10px] font-semibold text-text-dim mb-2 uppercase">MEMORY</div>
            <div className="space-y-1 max-h-32 overflow-y-auto">
              {memories.slice(0,4).map(m => <div key={m.id} className="text-[10px] text-text-muted border-l border-accent/30 pl-2 py-0.5"><span className="text-text-dim text-[9px]">{m.created_at?.slice(5,10)}</span> {m.content.slice(0,50)}...</div>)}
              {memories.length === 0 && <div className="text-[10px] text-text-dim">None</div>}
            </div>
          </div>
        </div>
      </div>

      {/* Terminal */}
      {showTerminal && (
        <div className="h-36 bg-bg-deep border-t border-border flex flex-col">
          <div className="flex items-center justify-between px-3 py-1 border-b border-border"><span className="text-[10px] text-text-dim font-mono">Terminal</span><button onClick={() => setShowTerminal(false)} className="text-text-dim hover:text-text-main"><X size={12} /></button></div>
          <div ref={termRef} className="flex-1 overflow-y-auto px-3 py-2 font-mono text-[11px]">
            {termHistory.map((line, i) => <div key={i} className={line.startsWith("> ") ? "text-accent" : line.startsWith("Error") ? "text-red-400" : "text-text-dim"}>{line}</div>)}
          </div>
          <div className="flex items-center gap-2 px-3 py-1.5 border-t border-border"><span className="text-accent font-mono text-[11px]">$</span><input value={termInput} onChange={e => setTermInput(e.target.value)} onKeyDown={e => { if (e.key === 'Enter') runTerminal(); }} className="flex-1 bg-transparent border-none outline-none text-text-main font-mono text-[11px] placeholder-text-dim" placeholder="cmd..." /></div>
        </div>
      )}

      {/* Status bar */}
      <div className="h-5 bg-bg-deep border-t border-border flex items-center justify-between px-3 text-[9px] text-text-dim">
        <span>{ct?.icon} {ct?.name} | Streaming</span>
        <div className="flex items-center gap-2"><span className={"w-1.5 h-1.5 rounded-full " + (isStreaming() ? "bg-accent animate-pulse" : "bg-green-400")} /><span>{isStreaming() ? "Streaming..." : "Ready"}</span></div>
      </div>

      {/* Modals */}
      {showSessions&&(<div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={()=>setShowSessions(false)}><div className="bg-bg-secondary rounded-2xl w-[400px] max-h-[70vh] shadow-2xl border border-border overflow-y-auto" onClick={e=>e.stopPropagation()}><div className="flex items-center justify-between p-4 border-b border-border"><h2 className="text-sm font-semibold">History</h2><button onClick={()=>setShowSessions(false)}><X size={16}/></button></div><div className="p-4 space-y-2">{sessions.length===0?<div className="text-xs text-text-dim text-center py-8">None</div>:sessions.map(s=>(<div key={s.id} className="bg-bg-tertiary rounded-lg p-3 flex items-center gap-3 cursor-pointer hover:bg-bg-hover" onClick={()=>loadSession(s)}><div className="flex-1"><div className="text-xs font-medium truncate">{s.title}</div><div className="text-[10px] text-text-dim">{s.messages.length} msgs</div></div><button onClick={e=>{e.stopPropagation();invoke("delete_session",{id:s.id}).then(loadSessions);}} className="text-text-dim hover:text-red-400"><Trash2 size={12}/></button></div>))}</div></div></div>)}

      {/* Add Tool Editor */}
      {showEditor && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={()=>{stopPtyTest();setShowEditor(false);}}>
          <div className="bg-bg-secondary rounded-2xl w-[520px] shadow-2xl border border-border" onClick={e=>e.stopPropagation()}>
            <div className="flex items-center justify-between p-4 border-b border-border"><h2 className="text-sm font-semibold">{editingTool.id?'Edit':'Add'} Tool</h2><button onClick={()=>{stopPtyTest();setShowEditor(false);}}><X size={16}/></button></div>
            <div className="p-4 space-y-3">
              <div className="flex gap-3">
                <input value={editingTool.icon||'🤖'} onChange={e=>setEditingTool({...editingTool,icon:e.target.value})} placeholder="Icon" className="w-14 bg-bg-tertiary border border-border rounded-lg px-2 py-2 text-lg text-center"/>
                <input value={editingTool.name} onChange={e=>setEditingTool({...editingTool,name:e.target.value})} placeholder="Tool Name" className="flex-1 bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs"/>
                <select value={editingTool.tool_type} onChange={e=>setEditingTool({...editingTool,tool_type:e.target.value})} className="w-28 bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs">
                  <option value="api">API</option><option value="process">CLI</option>
                </select>
              </div>

              {editingTool.tool_type==='api' ? (<>
                <input value={editingTool.api_base} onChange={e=>setEditingTool({...editingTool,api_base:e.target.value})} placeholder="API URL" className="w-full bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs"/>
                <input value={editingTool.model_name} onChange={e=>setEditingTool({...editingTool,model_name:e.target.value})} placeholder="Model" className="w-full bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs"/>
                <input type="password" value={editingTool.api_key} onChange={e=>setEditingTool({...editingTool,api_key:e.target.value})} placeholder="API Key" className="w-full bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs"/>
                <textarea value={editingTool.system_prompt} onChange={e=>setEditingTool({...editingTool,system_prompt:e.target.value})} placeholder="System prompt" className="w-full bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs resize-none" rows={3}/>
                <button onClick={()=>saveTool(editingTool)} className="w-full py-2.5 rounded-lg bg-accent text-white text-xs font-medium hover:bg-accent-hover">Save</button>
              </>) : (<>
                {/* CLI Terminal Mode */}
                <div className="bg-black rounded-xl border border-border overflow-hidden" style={{minHeight:'220px',display:'flex',flexDirection:'column'}}>
                  <div className="flex items-center justify-between px-3 py-1.5 bg-[#111] border-b border-[#333]">
                    <span className="text-[10px] text-gray-500 font-mono">Pseudo Terminal</span>
                    <div className="flex gap-1">
                      {ptyTestRunning && <button onClick={stopPtyTest} className="px-2 py-0.5 rounded text-[10px] bg-red-500/20 text-red-400 hover:bg-red-500/40">Stop</button>}
                    </div>
                  </div>
                  <div ref={ptyTestRef} className="flex-1 overflow-y-auto p-3 font-mono text-[12px] leading-relaxed" style={{background:'#0a0a0a',minHeight:'140px',maxHeight:'240px'}}>
                    {!ptyTestRunning ? (
                      <div className="text-gray-600 text-[11px]">Type a command and press Enter to start</div>
                    ) : (
                      ptyTestOutput.map((line, i) => (
                        <div key={i} className={line.startsWith('$ ') ? 'text-green-400' : line.startsWith('> ') ? 'text-cyan-400' : line.startsWith('Error') ? 'text-red-400' : 'text-gray-300'}>{line}</div>
                      ))
                    )}
                    {ptyTestRunning && <span className="animate-pulse text-gray-500">_</span>}
                  </div>
                  <div className="flex items-center gap-2 px-3 py-2 bg-[#111] border-t border-[#333]">
                    <span className="text-green-400 font-mono text-[11px]">{ptyTestRunning ? '>' : '$'}</span>
                    <input value={ptyTestInput} onChange={e => setPtyTestInput(e.target.value)}
                      onKeyDown={e => { if (e.key === 'Enter') { if (!ptyTestRunning) startPtyTest(); else sendPtyTest(); } }}
                      className="flex-1 bg-transparent border-none outline-none text-gray-200 font-mono text-[12px] placeholder-gray-600" placeholder={ptyTestRunning ? "Type message..." : "e.g. npx reasonix run"} autoFocus />
                  </div>
                </div>
                {/* Save button */}
                {ptyTestRunning && ptyTestOutput.length > 1 && (
                  <div className="flex gap-2">
                    <input id="toolNameInput" defaultValue={editingTool.name||'CLI Tool'} placeholder="Tool name" className="flex-1 bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs" style={{display:'none'}}/>
                    <button onClick={() => {
                      const name = (document.getElementById('toolNameInput') as HTMLInputElement)?.value || editingTool.name || 'CLI Tool';
                      savePtyTool(name);
                    }} className="flex-1 py-2.5 rounded-lg bg-accent text-white text-xs font-medium hover:bg-accent-hover">Save as Tool</button>
                    <button onClick={stopPtyTest} className="px-4 py-2.5 rounded-lg bg-red-500/20 text-red-400 text-xs">Stop</button>
                  </div>
                )}
                {/* Show API form for existing CLI tools */}
                {editingTool.id && !ptyTestRunning && (
                  <><input value={editingTool.command} onChange={e=>setEditingTool({...editingTool,command:e.target.value})} placeholder="Command" className="w-full bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs"/><textarea value={editingTool.args.join(', ')} onChange={e=>setEditingTool({...editingTool,args:e.target.value.split(',').map((s:string)=>s.trim()).filter(Boolean)})} placeholder="Args (comma separated)" className="w-full bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs resize-none" rows={2}/><textarea value={editingTool.system_prompt} onChange={e=>setEditingTool({...editingTool,system_prompt:e.target.value})} placeholder="System prompt" className="w-full bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-xs resize-none" rows={2}/><button onClick={()=>saveTool(editingTool)} className="w-full py-2.5 rounded-lg bg-accent text-white text-xs font-medium hover:bg-accent-hover">Save</button></>
                )}
              </>)}
            </div>
          </div>
        </div>
      )}
      {deleteTargetId && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setDeleteTargetId('')}>
          <div className="bg-bg-secondary rounded-2xl w-[320px] shadow-2xl border border-border p-5" onClick={e => e.stopPropagation()}>
            <div className="text-sm font-semibold mb-4">Delete this tool?</div>
            <div className="flex gap-3"><button onClick={() => setDeleteTargetId('')} className="flex-1 py-2 rounded-lg bg-bg-tertiary text-xs">Cancel</button><button onClick={() => deleteToolConfirm(deleteTargetId)} className="flex-1 py-2 rounded-lg bg-red-500/20 text-red-400 text-xs">Delete</button></div>
          </div>
        </div>
      )}
    </div>
  );
}
export default App;
