import type { Session, SessionDetail } from '../beacon';
import { formatTokens } from '../lib/format.ts';
import { textLines } from '../lib/lines.ts';
import { Collapse } from './Collapse';
import { Icon } from './Icon';
import { LineClamp } from './LineClamp';
import { Markdown } from './Markdown';

interface SessionDetailsProps {
  session: Session;
  detail: SessionDetail | null;
  open: boolean;
  onToggle: () => void;
}

export function SessionDetails({ session, detail, open, onToggle }: SessionDetailsProps) {
  const usage = session.usage;
  if (!detail?.last_message && usage?.status !== 'ok' && usage?.status !== 'unrecognized') return null;
  return (
    <section>
      <button
        type="button"
        aria-expanded={open}
        onClick={onToggle}
        className="press flex items-center gap-1.5 rounded-field text-secondary text-faint hover:text-muted"
      >
        <Icon name="chevron" className={`size-3 transition-transform duration-200 ease-settle ${open ? 'rotate-90' : ''}`} />
        More
      </button>
      <Collapse open={open}>
        <div className="space-y-4 pt-2.5">
          {detail?.last_message && (
            <div>
              <h3 className="mb-1.5 text-micro font-medium text-faint">Last message</h3>
              <LineClamp lines={textLines(detail.last_message)}>
                {(shown) => <Markdown source={shown.join('\n')} />}
              </LineClamp>
            </div>
          )}
          {usage?.status === 'ok' && (
            <div>
              <h3 className="mb-1.5 text-micro font-medium text-faint">Usage</h3>
              <dl className="grid grid-cols-2 gap-x-6 gap-y-0.5 text-secondary">
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
                <p className="mt-1.5 text-micro text-muted">
                  reported cost ${usage.reported_cost_usd.toFixed(2)} (list price, from Claude Code)
                </p>
              )}
            </div>
          )}
          {usage?.status === 'unrecognized' && (
            <p className="text-micro text-muted">Usage unavailable: transcript format not recognised</p>
          )}
        </div>
      </Collapse>
    </section>
  );
}
