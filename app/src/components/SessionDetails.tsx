import type { Session, SessionDetail } from '../beacon';
import { formatTokens } from '../lib/format.ts';
import type { JumpReport } from '../lib/jump.ts';
import { textLines } from '../lib/lines.ts';
import { LineClamp } from './LineClamp';
import { Markdown } from './Markdown';
import { PromptBox } from './PromptBox';

interface SessionDetailsProps {
  session: Session;
  detail: SessionDetail | null;
  onJumpReport: (report: JumpReport) => void;
}

export function SessionDetails({ session, detail, onJumpReport }: SessionDetailsProps) {
  const usage = session.usage;
  return (
    <>
      <PromptBox session={session} onJumpReport={onJumpReport} />
      {detail?.last_message && (
        <section>
          <h3 className="mb-1 text-micro text-faint">Last message</h3>
          <LineClamp lines={textLines(detail.last_message)}>
            {(shown) => <Markdown source={shown.join('\n')} />}
          </LineClamp>
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
    </>
  );
}
