import React from 'react';
import { CheckCircle, XCircle, Loader2, FileText, Search, Globe, Database, Code, BookOpen } from 'lucide-react';

interface ToolCallData {
  name: string;
  args?: Record<string, unknown>;
  result?: string;
  status: 'running' | 'success' | 'error';
}

const toolIcons: Record<string, React.ReactNode> = {
  read_file: <FileText size={14} />,
  write_file: <FileText size={14} />,
  search_knowledge: <BookOpen size={14} />,
  web_search: <Globe size={14} />,
  memory_search: <Database size={14} />,
  memory_save: <Database size={14} />,
  run_code: <Code size={14} />,
};

const toolLabels: Record<string, string> = {
  read_file: 'Read File',
  write_file: 'Write File',
  search_knowledge: 'Search Knowledge',
  web_search: 'Web Search',
  memory_search: 'Search Memory',
  memory_save: 'Save Memory',
  run_code: 'Run Code',
};

interface ToolCallBlockProps {
  calls: ToolCallData[];
}

export default function ToolCallBlock({ calls }: ToolCallBlockProps) {
  if (!calls || calls.length === 0) return null;

  return (
    <div className="space-y-2 my-2">
      {calls.map((call, i) => (
        <div
          key={i}
          className="bg-bg-deep border border-border rounded-lg overflow-hidden text-xs"
        >
          {/* Header */}
          <div className="flex items-center gap-2 px-3 py-2 border-b border-border/50">
            <span className="text-accent">
              {toolIcons[call.name] || <Code size={14} />}
            </span>
            <span className="font-medium text-text-main">
              {toolLabels[call.name] || call.name}
            </span>
            <span className="ml-auto">
              {call.status === 'running' && (
                <Loader2 size={12} className="animate-spin text-accent" />
              )}
              {call.status === 'success' && (
                <CheckCircle size={12} className="text-green-400" />
              )}
              {call.status === 'error' && (
                <XCircle size={12} className="text-red-400" />
              )}
            </span>
          </div>

          {/* Args (collapsible) */}
          {call.args && Object.keys(call.args).length > 0 && (
            <details className="px-3 py-1.5">
              <summary className="text-[10px] text-text-dim cursor-pointer hover:text-text-muted">
                Parameters
              </summary>
              <pre className="mt-1 text-[10px] text-text-muted font-mono bg-black/20 rounded p-2 overflow-x-auto">
                {JSON.stringify(call.args, null, 2)}
              </pre>
            </details>
          )}

          {/* Result (collapsible) */}
          {call.result && call.status !== 'running' && (
            <details className="px-3 py-1.5 border-t border-border/30">
              <summary className="text-[10px] text-text-dim cursor-pointer hover:text-text-muted">
                Result
              </summary>
              <pre className="mt-1 text-[10px] text-text-muted font-mono bg-black/20 rounded p-2 overflow-x-auto max-h-32 overflow-y-auto">
                {call.result.length > 500
                  ? call.result.slice(0, 500) + '...'
                  : call.result}
              </pre>
            </details>
          )}
        </div>
      ))}
    </div>
  );
}
