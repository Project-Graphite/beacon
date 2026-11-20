import { useEffect, useState } from 'react';
import {
  appInfo,
  getLoginStart,
  openLogsFolder,
  setConfig,
  setLoginStart,
  type AppInfo,
  type LiveConfig,
  type UiSettings,
} from '../beacon';
import { stallMinutes } from '../lib/settings.ts';
import { CheckboxField } from './CheckboxField';
import { HooksSection } from './HooksSection';
import { OutlineButton } from './OutlineButton';
import { SettingsSection } from './SettingsSection';
import { TextButton } from './TextButton';

type Section = 'behaviour' | 'notifications' | 'shortcut' | 'windows' | 'about';

const FIELD =
  'h-6 rounded-field border border-line bg-paper px-2 font-mono text-secondary text-ink placeholder:text-faint focus:border-muted focus:outline-none';

interface SettingsViewProps {
  config: LiveConfig | null;
  ui: UiSettings | null;
  onChangeUi: (change: Partial<UiSettings>) => Promise<UiSettings>;
}

export function SettingsView({ config, ui, onChangeUi }: SettingsViewProps) {
  const [editedHotkey, setEditedHotkey] = useState<string | null>(null);
  const [loginStart, setLoginStartShown] = useState<boolean | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [stallText, setStallText] = useState<string | null>(null);
  const [errors, setErrors] = useState<Partial<Record<Section, string | null>>>({});

  const attempt = <T,>(section: Section, work: Promise<T>, done?: (value: T) => void) =>
    work.then(
      (value) => {
        setErrors((current) => ({ ...current, [section]: null }));
        done?.(value);
      },
      (error) => setErrors((current) => ({ ...current, [section]: String(error) })),
    );

  const hotkey = editedHotkey ?? ui?.hotkey ?? '';

  useEffect(() => {
    attempt('windows', getLoginStart(), setLoginStartShown);
    attempt('about', appInfo(), setInfo);
  }, []);

  const commitStall = () => {
    if (stallText === null) return;
    const minutes = stallMinutes(stallText);
    if (minutes === null) {
      setErrors((current) => ({ ...current, behaviour: 'Enter a whole number of minutes from 1 to 240.' }));
      return;
    }
    attempt('behaviour', setConfig({ stall_minutes: minutes }), () => setStallText(null));
  };

  const applyHotkey = () => attempt('shortcut', onChangeUi({ hotkey: hotkey.trim() }), () => setEditedHotkey(null));

  return (
    <div>
      <HooksSection />
      <SettingsSection label="Behaviour" error={errors.behaviour}>
        <CheckboxField
          label="Answer headless runs from Beacon"
          help="Off: a claude -p run with nobody attached is passed straight through, so it never waits on you."
          checked={config?.answer_unattended ?? false}
          disabled={!config}
          onChange={(checked) => attempt('behaviour', setConfig({ answer_unattended: checked }))}
        />
        <label className="flex items-center gap-2 py-1">
          <span className="text-ink">Mark a session as stalled after</span>
          <input
            inputMode="numeric"
            disabled={!config}
            value={stallText ?? String(config?.stall_minutes ?? '')}
            onChange={(event) => setStallText(event.target.value)}
            onBlur={commitStall}
            onKeyDown={(event) => {
              if (event.key === 'Enter') commitStall();
            }}
            className={`${FIELD} w-12 text-right tabular-nums`}
          />
          <span className="text-secondary text-muted">minutes</span>
        </label>
      </SettingsSection>
      <SettingsSection label="Notifications" error={errors.notifications}>
        <CheckboxField
          label="Show notifications"
          checked={ui?.toasts_enabled ?? true}
          disabled={!ui}
          onChange={(checked) => attempt('notifications', onChangeUi({ toasts_enabled: checked }))}
        />
        <CheckboxField
          label="Also when a session finishes"
          checked={ui?.toast_on_finished ?? true}
          disabled={!ui?.toasts_enabled}
          onChange={(checked) => attempt('notifications', onChangeUi({ toast_on_finished: checked }))}
        />
        <CheckboxField
          label="Play a sound when a session needs you or finishes"
          checked={ui?.sounds_enabled ?? false}
          disabled={!ui}
          onChange={(checked) => attempt('notifications', onChangeUi({ sounds_enabled: checked }))}
        />
      </SettingsSection>
      <SettingsSection label="Shortcut" error={errors.shortcut}>
        <div className="flex items-center gap-1.5">
          <input
            aria-label="Shortcut that opens the panel"
            placeholder="Ctrl+Alt+B"
            value={hotkey}
            disabled={!ui}
            onChange={(event) => setEditedHotkey(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && hotkey.trim()) applyHotkey();
            }}
            className={`${FIELD} min-w-0 flex-1`}
          />
          <OutlineButton disabled={!ui || !hotkey.trim() || hotkey.trim() === ui.hotkey} onClick={applyHotkey}>
            Apply
          </OutlineButton>
        </div>
      </SettingsSection>
      <SettingsSection label="Windows" error={errors.windows}>
        <CheckboxField
          label="Start Beacon when I sign in"
          checked={loginStart ?? false}
          disabled={loginStart === null}
          onChange={(checked) => attempt('windows', setLoginStart(checked), () => setLoginStartShown(checked))}
        />
      </SettingsSection>
      <SettingsSection label="About" error={errors.about}>
        <p className="text-ink">
          Beacon <span className="font-mono text-secondary text-muted tabular-nums">{info?.version}</span>
        </p>
        <p title={info?.home} className="truncate font-mono text-micro text-muted">
          {info?.home}
        </p>
        <div className="mt-1">
          <TextButton onClick={() => attempt('about', openLogsFolder())}>Open logs folder</TextButton>
        </div>
      </SettingsSection>
    </div>
  );
}
