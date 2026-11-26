import type { PendingDecision } from '../beacon';
import { lineDiff, writePreview } from '../lib/diff.ts';
import { displayPath } from '../lib/paths.ts';
import { redact } from '../lib/redact.ts';
import { argumentEntries, mcpTool, splitUrl } from '../lib/requests.ts';
import { ArgumentList } from './ArgumentList';
import { CommandBlock } from './CommandBlock';
import { DiffBlock } from './DiffBlock';

interface ToolRequestBodyProps {
  request: PendingDecision;
  masked: boolean;
}

export function ToolRequestBody({ request, masked }: ToolRequestBodyProps) {
  const input = (request.tool_input ?? {}) as Record<string, unknown>;
  const field = (name: string) => {
    const value = input[name];
    return typeof value !== 'string' ? null : masked ? redact(value) : value;
  };
  const path = field('file_path');
  const pathLine = path !== null && (
    <p className="mt-1 font-mono text-secondary break-all text-ink">{displayPath(path, request.cwd)}</p>
  );
  const command = field('command');
  const url = field('url');
  const mcp = mcpTool(request.tool_name);
  const shownArguments = argumentEntries(request.tool_input, masked);

  if ((request.tool_name === 'Bash' || request.tool_name === 'PowerShell') && command !== null) {
    return (
      <>
        {field('description') && <p className="mt-1 text-secondary text-muted">{field('description')}</p>}
        <CommandBlock command={command} />
      </>
    );
  }
  if (request.tool_name === 'Edit' && path !== null) {
    return (
      <>
        {pathLine}
        {input.replace_all === true && <p className="text-micro text-muted">replaces every occurrence</p>}
        <DiffBlock lines={lineDiff(field('old_string') ?? '', field('new_string') ?? '')} />
      </>
    );
  }
  if (request.tool_name === 'Write' && path !== null) {
    const preview = writePreview(field('content') ?? '');
    return (
      <>
        {pathLine}
        {preview.lines.length > 0 && <DiffBlock lines={preview.lines} />}
        {preview.more > 0 && <p className="mt-0.5 font-mono text-micro text-faint">+{preview.more} more lines</p>}
      </>
    );
  }
  if (request.tool_name === 'WebFetch' && url !== null) {
    const { head, host, tail } = splitUrl(url);
    return (
      <>
        <p className="mt-1 font-mono text-secondary text-muted [overflow-wrap:anywhere]">
          {head}
          <span className="text-ink">{host}</span>
          {tail}
        </p>
        {field('prompt') && <p className="mt-0.5 text-secondary text-muted">{field('prompt')}</p>}
      </>
    );
  }
  if (mcp) {
    return (
      <>
        <dl className="mt-1 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 text-secondary">
          <dt className="text-muted">Server</dt>
          <dd className="truncate font-mono text-ink">{mcp.server}</dd>
          <dt className="text-muted">Tool</dt>
          <dd className="truncate font-mono text-ink">{mcp.tool}</dd>
        </dl>
        <ArgumentList entries={shownArguments} />
      </>
    );
  }
  return <ArgumentList entries={shownArguments} />;
}
