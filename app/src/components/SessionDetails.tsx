import { useEffect, useRef, useState } from 'react';
import { getDetail, type Session, type SessionDetail } from '../beacon';
import { clockTimeWithSeconds, formatTokens } from '../lib/format.ts';
import type { JumpReport } from '../lib/jump.ts';
import { Markdown } from './Markdown';
import { PromptBox } from './PromptBox';

const TIMELINE_LIMIT = 12;

interface SessionDetailsProps {
  session: Session;
  expanded: boolean;
  onJumpReport: (report: JumpReport) => void;
}

export function SessionDetails({ session, expanded, onJumpReport }: SessionDetailsProps) {
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);
  const loadedAt = useRef<number | null>(null);

  useEffect(() => {
    if (!expanded || loadedAt.current === session.event_count) return;
    loadedAt.current = session.event_count;
    getDetail(session.session_id).then(
      (loaded) => {
        setDetail(loaded);
        setFailure(null);
      },
      (error) => setFailure(String(error)),
    );
  }, [expanded, session.session_id, session.event_count]);

  const usage = session.usage;
  const timeline = [...(detail?.timeline ?? [])].sort((a, b) => b.t_ms - a.t_ms);

  return (
    <div className="space-y-3 border-t border-line-soft pt-2.5 pr-2.5 pb-3 pl-8">
      {failure && <p className="text-micro text-danger">Cannot load details: {failure}</p>}
      {detail?.last_message && (
        <section>
          <h3 className="mb-1 text-micro text-faint">Last message</h3>
          <div className="scroll-thin max-h-60 overflow-y-auto pr-1">
            <Markdown source={detail.last_message} />
          </div>
        </section>
      )}
      {usage?.status === 'ok' && (
        <section>
          <h3 className="mb-1 text-micro text-faint">Usage</h3>
          <dl className="grid grid-cols-2 gap-x-4 font-mono text-micro">
            {(
              [
                ['input', usage.input_tokens],
                ['output', usage.output_tokens],
                ['cache write', usage.cache_creation_tokens],
                ['cache read', usage.cache_read_tokens],
                ['subagent', usage.subagent_tokens],
                ['context', usage.context_tokens],
              ] as const
            ).map(([label, count]) => (
              <div key={label} className="flex justify-between gap-2">
                <dt className="text-muted">{label}</dt>
                <dd className="text-ink tabular-nums">{formatTokens(count)}</dd>
              </div>
            ))}
          </dl>
          {usage.reported_cost_usd !== null && (
            <p className="mt-1 font-mono text-micro text-muted">
              reported cost ${usage.reported_cost_usd.toFixed(2)} (list price, from Claude Code)
            </p>
          )}
        </section>
      )}
      {usage?.status === 'unrecognized' && (
        <p className="text-micro text-muted">Usage unavailable: transcript format not recognised</p>
      )}
      <PromptBox session={session} onJumpReport={onJumpReport} />
      {timeline.length > 0 && (
        <section>
          <h3 className="mb-1 text-micro text-faint">Timeline</h3>
          <ol className="space-y-0.5">
            {(showAll ? timeline : timeline.slice(0, TIMELINE_LIMIT)).map((entry, index) => (
              <li key={index} className="flex items-baseline gap-2 text-secondary">
                <span className="shrink-0 font-mono text-micro text-faint tabular-nums">
                  {clockTimeWithSeconds(entry.t_ms)}
                </span>
                <span className="min-w-0 truncate text-muted">{entry.label}</span>
              </li>
            ))}
          </ol>
          {!showAll && timeline.length > TIMELINE_LIMIT && (
            <button
              type="button"
              className="press mt-1 rounded-field text-micro text-muted hover:text-ink"
              onClick={() => setShowAll(true)}
            >
              Show all
            </button>
          )}
        </section>
      )}
    </div>
  );
}
