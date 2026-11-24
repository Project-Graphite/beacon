import { useState } from 'react';
import { recordShortcut, shortcutKeys } from '../lib/shortcut.ts';
import type { Field } from './SettingsRow';

interface ShortcutRecorderProps {
  field: Field;
  value: string;
  onRecord: (shortcut: string) => Promise<unknown>;
  onPause: (paused: boolean) => void;
}

export function ShortcutRecorder({ field, value, onRecord, onPause }: ShortcutRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  const start = () => {
    setRecording(true);
    onPause(true);
  };

  const stop = () => {
    if (!recording) return;
    setRecording(false);
    setHint(null);
    onPause(false);
  };

  return (
    <button
      type="button"
      id={field.id}
      aria-describedby={field.helpId}
      aria-pressed={recording}
      title={recording ? 'Press the keys, or Escape to cancel' : 'Click, then press the keys you want'}
      onClick={() => (recording ? stop() : start())}
      onBlur={stop}
      onKeyDown={(event) => {
        if (!recording) return;
        event.preventDefault();
        event.stopPropagation();
        if (event.repeat) return;
        const recorded = recordShortcut(event);
        if (recorded.kind === 'cancel') stop();
        else if (recorded.kind === 'invalid') setHint(recorded.reason);
        else if (recorded.kind === 'shortcut') {
          setRecording(false);
          setHint(null);
          void onRecord(recorded.text).finally(() => onPause(false));
        }
      }}
      className={`press flex h-7 min-w-0 shrink items-center gap-1 rounded-field border px-1.5 text-secondary shadow-card transition-colors duration-150 ease-settle ${recording ? 'border-wip bg-wip/[0.08] text-ink' : 'border-line bg-raised text-ink hover:border-faint/60'}`}
    >
      {recording ? (
        <span aria-live="polite" className="animate-appear truncate px-1 text-muted">{hint ?? 'Press a shortcut…'}</span>
      ) : (
        shortcutKeys(value).map((key) => (
          <kbd key={key} className="rounded-[5px] border border-line-soft bg-surface px-1.5 font-sans text-micro leading-5 text-ink">
            {key}
          </kbd>
        ))
      )}
    </button>
  );
}
