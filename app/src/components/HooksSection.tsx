import { useEffect, useState } from 'react';
import { hooksApply, hooksPlan, hooksStatus, type HooksAction, type HooksStatus } from '../beacon';
import { HOOKS_STATES, hooksButtons, splitPlan } from '../lib/settings.ts';
import { DiffBlock } from './DiffBlock';
import { FilledButton } from './FilledButton';
import { OutlineButton } from './OutlineButton';
import { SettingsSection } from './SettingsSection';

export function HooksSection() {
  const [status, setStatus] = useState<HooksStatus | null>(null);
  const [plan, setPlan] = useState<{ action: HooksAction; text: string } | null>(null);
  const [applied, setApplied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const attempt = <T,>(work: Promise<T>, done: (value: T) => void) => {
    setBusy(true);
    setError(null);
    work.then(done, (failure) => setError(String(failure))).finally(() => setBusy(false));
  };

  useEffect(() => {
    attempt(hooksStatus(), setStatus);
  }, []);

  const shownPlan = plan && splitPlan(plan.text);

  return (
    <SettingsSection label="Claude Code hooks" error={error}>
      {status && (
        <>
          <p className="flex items-center gap-1.5 text-ink">
            <span aria-hidden="true" className={`size-1.5 rounded-full ${HOOKS_STATES[status.state].dot}`} />
            {HOOKS_STATES[status.state].label}
          </p>
          <p title={status.settings_path} className="truncate font-mono text-micro text-muted">
            {status.settings_path}
          </p>
          {status.problems.length + status.warnings.length > 0 && (
            <ul className="mt-1 space-y-0.5 text-secondary">
              {status.problems.map((problem) => (
                <li key={problem} className="break-words text-danger">
                  {problem}
                </li>
              ))}
              {status.warnings.map((warning) => (
                <li key={warning} className="break-words text-muted">
                  {warning}
                </li>
              ))}
            </ul>
          )}
          {plan && shownPlan ? (
            <div className="mt-2">
              <div className="space-y-0.5 font-mono text-micro text-muted">
                {shownPlan.notes.map((note, index) => (
                  <p key={index} className="whitespace-pre-wrap wrap-anywhere">
                    {note}
                  </p>
                ))}
              </div>
              {shownPlan.diff.length > 0 && <DiffBlock lines={shownPlan.diff} />}
              <div className="mt-2 flex items-center gap-1.5">
                <FilledButton
                  autoFocus
                  disabled={busy}
                  onClick={() =>
                    attempt(hooksApply(plan.action), (next) => {
                      setStatus(next);
                      setPlan(null);
                      setApplied(true);
                    })
                  }
                >
                  Apply
                </FilledButton>
                <OutlineButton disabled={busy} onClick={() => setPlan(null)}>
                  Cancel
                </OutlineButton>
              </div>
            </div>
          ) : (
            <div className="mt-2 flex items-center gap-1.5">
              {hooksButtons(status.state).map(({ label, action }) => (
                <OutlineButton
                  key={action}
                  disabled={busy}
                  onClick={() =>
                    attempt(hooksPlan(action), (text) => {
                      setApplied(false);
                      setPlan({ action, text });
                    })
                  }
                >
                  {label}
                </OutlineButton>
              ))}
            </div>
          )}
          {applied && (
            <p className="mt-1.5 text-secondary text-muted">
              Restart running Claude sessions is not needed: they pick up hook changes automatically.
            </p>
          )}
        </>
      )}
    </SettingsSection>
  );
}
