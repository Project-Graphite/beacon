import { useEffect, useRef, useState } from 'react';
import { snooze, unsnooze } from '../beacon';
import { clockTime } from '../lib/format.ts';
import { Icon } from './Icon';
import { OutlineButton } from './OutlineButton';

interface SnoozeMenuProps {
  snoozedUntil: number | null;
  run: (action: Promise<unknown>) => void;
}

export function SnoozeMenu({ snoozedUntil, run }: SnoozeMenuProps) {
  const [open, setOpen] = useState(false);
  const menu = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: PointerEvent) => {
      if (!(event.target instanceof Node && menu.current?.contains(event.target))) setOpen(false);
    };
    window.addEventListener('pointerdown', closeOutside);
    return () => window.removeEventListener('pointerdown', closeOutside);
  }, [open]);

  const choose = (action: Promise<unknown>) => {
    setOpen(false);
    run(action);
  };

  return (
    <div
      ref={menu}
      className="relative"
      onKeyDown={(event) => {
        if (event.key === 'Escape' && open) {
          event.stopPropagation();
          setOpen(false);
        }
      }}
    >
      <button
        type="button"
        aria-label="Snooze"
        title={snoozedUntil === null ? 'Snooze' : `Snoozed until ${clockTime(snoozedUntil)}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className={`press grid size-6 place-items-center rounded-field hover:bg-surface hover:text-ink ${snoozedUntil === null ? 'text-muted' : 'text-ink'}`}
      >
        <Icon name="bell-off" />
      </button>
      {open && (
        <div
          role="menu"
          className="absolute top-full right-0 z-20 mt-1 w-56 origin-top-right animate-appear rounded-field border border-line bg-raised p-1 text-secondary"
        >
          {[5, 15].map((minutes) => (
            <button
              key={minutes}
              type="button"
              role="menuitem"
              onClick={() => choose(snooze(null, minutes))}
              className="flex h-7 w-full items-center rounded-[6px] px-2 text-left text-ink transition-colors duration-120 ease-settle hover:bg-line-soft"
            >
              Snooze all for {minutes} minutes
            </button>
          ))}
          {snoozedUntil !== null && (
            <div className="mt-1 flex items-center justify-between gap-2 border-t border-line-soft px-2 pt-1.5 pb-0.5">
              <span className="text-muted">Snoozed until {clockTime(snoozedUntil)}</span>
              <OutlineButton role="menuitem" onClick={() => choose(unsnooze(null))}>
                Clear
              </OutlineButton>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
