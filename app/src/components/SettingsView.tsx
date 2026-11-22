import { useEffect, useState } from 'react';
import { appInfo, cachedAppInfo, openLogsFolder, previewAttention, setConfig, type AppInfo, type LiveConfig, type UiSettings } from '../beacon';
import { errorText } from '../lib/format.ts';
import {
  ATTENTION_CHOICES,
  OPACITY_RANGE,
  PEEK_RANGE,
  percent,
  QUIT_CHOICES,
  REPEAT_CHOICES,
  SIZE_CHOICES,
  stallMinutes,
  START_CHOICES,
  UI_DEFAULTS,
  VISIBILITY_CHOICES,
} from '../lib/settings.ts';
import { CheckboxField } from './CheckboxField';
import { HooksSection } from './HooksSection';
import { OutlineButton } from './OutlineButton';
import { RadioList } from './RadioList';
import { SegmentedControl } from './SegmentedControl';
import { SettingsSection } from './SettingsSection';
import { Slider } from './Slider';
import { TextButton } from './TextButton';

type Section = 'overlay' | 'attention' | 'startup' | 'behaviour' | 'shortcut' | 'about';

const FIELD =
  'h-6 rounded-field border border-line bg-paper px-2 font-mono text-secondary text-ink placeholder:text-faint focus:border-muted focus:outline-none';

interface SettingsViewProps {
  config: LiveConfig | null;
  ui: UiSettings | null;
  onChangeUi: (change: Partial<UiSettings>) => Promise<UiSettings>;
}

export function SettingsView({ config, ui, onChangeUi }: SettingsViewProps) {
  const [editedHotkey, setEditedHotkey] = useState<string | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(cachedAppInfo);
  const [stallText, setStallText] = useState<string | null>(null);
  const [errors, setErrors] = useState<Partial<Record<Section, string | null>>>({});

  const attempt = <T,>(section: Section, work: Promise<T>, done?: (value: T) => void) =>
    work.then(
      (value) => {
        setErrors((current) => ({ ...current, [section]: null }));
        done?.(value);
      },
      (error) => setErrors((current) => ({ ...current, [section]: errorText(error) })),
    );

  const change = (section: Section, update: Partial<UiSettings>) => attempt(section, onChangeUi(update));
  const shown = ui ?? UI_DEFAULTS;
  const hotkey = editedHotkey ?? ui?.hotkey ?? '';

  useEffect(() => {
    attempt('about', appInfo(), setInfo);
  }, []);

  const commitStall = () => {
    if (stallText === null) return;
    const minutes = stallMinutes(stallText);
    if (minutes === null) {
      setErrors((current) => ({ ...current, behaviour: 'Enter a whole number of minutes from 1 to 240' }));
      return;
    }
    attempt('behaviour', setConfig({ stall_minutes: minutes }), () => setStallText(null));
  };

  const applyHotkey = () => attempt('shortcut', onChangeUi({ hotkey: hotkey.trim() }), () => setEditedHotkey(null));

  return (
    <div>
      <HooksSection />
      <SettingsSection label="Overlay" error={errors.overlay}>
        <RadioList
          label="Show it"
          choices={VISIBILITY_CHOICES}
          value={shown.visibility}
          disabled={!ui}
          onChange={(visibility) => change('overlay', { visibility })}
        />
        <CheckboxField
          label="Reveal when I hover its screen edge"
          checked={shown.reveal_on_hover}
          disabled={!ui || shown.visibility === 'never'}
          onChange={(reveal_on_hover) => change('overlay', { reveal_on_hover })}
        />
        <SegmentedControl
          label="Size"
          choices={SIZE_CHOICES}
          value={shown.size}
          disabled={!ui}
          onChange={(size) => change('overlay', { size })}
        />
        <Slider
          label="Opacity when nothing needs you"
          value={shown.idle_opacity}
          range={OPACITY_RANGE}
          format={percent}
          disabled={!ui}
          onCommit={(idle_opacity) => change('overlay', { idle_opacity })}
        />
      </SettingsSection>
      <SettingsSection label="Attention" error={errors.attention}>
        <RadioList
          label="When a session needs you"
          choices={ATTENTION_CHOICES}
          value={shown.attention_style}
          disabled={!ui}
          onChange={(attention_style) => change('attention', { attention_style })}
        />
        <SegmentedControl
          label="Remind me"
          choices={REPEAT_CHOICES}
          value={shown.attention_repeat_seconds}
          disabled={!ui}
          onChange={(attention_repeat_seconds) => change('attention', { attention_repeat_seconds })}
        />
        <Slider
          label="Label stays for"
          value={shown.peek_seconds}
          range={PEEK_RANGE}
          format={(seconds) => `${seconds} s`}
          disabled={!ui || shown.attention_style !== 'pulse_peek'}
          onCommit={(peek_seconds) => change('attention', { peek_seconds })}
        />
        <CheckboxField
          label="Show notifications"
          checked={shown.toasts_enabled}
          disabled={!ui}
          onChange={(toasts_enabled) => change('attention', { toasts_enabled })}
        />
        <CheckboxField
          label="Also when a session finishes"
          checked={shown.toast_on_finished}
          disabled={!ui?.toasts_enabled}
          onChange={(toast_on_finished) => change('attention', { toast_on_finished })}
        />
        <CheckboxField
          label="Play a sound when a session needs you or finishes"
          checked={shown.sounds_enabled}
          disabled={!ui}
          onChange={(sounds_enabled) => change('attention', { sounds_enabled })}
        />
        <div className="mt-1">
          <OutlineButton disabled={!ui} onClick={() => attempt('attention', previewAttention())}>
            Preview
          </OutlineButton>
        </div>
      </SettingsSection>
      <SettingsSection label="Startup" error={errors.startup}>
        <RadioList
          label="Start Beacon"
          choices={START_CHOICES}
          value={shown.start_mode}
          disabled={!ui}
          onChange={(start_mode) => change('startup', { start_mode })}
        />
        <SegmentedControl
          label="Close it after Claude has been unused for"
          choices={QUIT_CHOICES}
          value={shown.quit_when_idle_minutes}
          disabled={!ui || shown.start_mode !== 'claude'}
          onChange={(quit_when_idle_minutes) => change('startup', { quit_when_idle_minutes })}
        />
      </SettingsSection>
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
