import { useEffect, useState } from 'react';
import {
  appInfo,
  cachedAppInfo,
  cachedOverlay,
  getOverlay,
  onSettingsChanged,
  openLogsFolder,
  previewAttention,
  quitApp,
  setConfig,
  setDisplay,
  setDisplayAvoided,
  setOverlayPosition,
  setStyle,
  type AppInfo,
  type LiveConfig,
  type Overlay,
  type UiSettings,
} from '../beacon';
import { errorText } from '../lib/format.ts';
import {
  ATTENTION_CHOICES,
  attentionSummary,
  avoidedLabel,
  behaviourSummary,
  CLEAR_CHOICES,
  displayLabel,
  labelOf,
  OPACITY_RANGE,
  overlaySummary,
  PEEK_RANGE,
  percent,
  positionChoices,
  QUIT_CHOICES,
  REPEAT_CHOICES,
  SIZE_CHOICES,
  stallMinutes,
  START_CHOICES,
  startupSummary,
  STYLE_CHOICES,
  VISIBILITY_CHOICES,
} from '../lib/settings.ts';
import { HooksSection } from './HooksSection';
import { OutlineButton } from './OutlineButton';
import { SegmentedControl } from './SegmentedControl';
import { Select } from './Select';
import { SettingsRow } from './SettingsRow';
import { SettingsSection } from './SettingsSection';
import { Slider } from './Slider';
import { Switch } from './Switch';
import { TextButton } from './TextButton';

export type SettingsPart = 'overlay' | 'attention' | 'startup' | 'behaviour' | 'hooks' | 'about';

const FIELD =
  'h-6 rounded-field border border-line bg-paper px-2 font-mono text-secondary text-ink placeholder:text-faint focus:border-muted focus:outline-none disabled:opacity-50';

interface SettingsViewProps {
  config: LiveConfig | null;
  ui: UiSettings;
  openParts: SettingsPart[];
  onToggle: (part: SettingsPart) => void;
  onChangeUi: (change: Partial<UiSettings>) => Promise<UiSettings>;
}

export function SettingsView({ config, ui, openParts, onToggle, onChangeUi }: SettingsViewProps) {
  const [editedHotkey, setEditedHotkey] = useState<string | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(cachedAppInfo);
  const [overlay, setOverlay] = useState<Overlay | null>(cachedOverlay);
  const [stallText, setStallText] = useState<string | null>(null);
  const [errors, setErrors] = useState<Partial<Record<SettingsPart, string | null>>>({});

  const attempt = <T,>(part: SettingsPart, work: Promise<T>, done?: (value: T) => void) =>
    work.then(
      (value) => {
        setErrors((current) => ({ ...current, [part]: null }));
        done?.(value);
      },
      (error) => setErrors((current) => ({ ...current, [part]: errorText(error) })),
    );

  const change = (part: SettingsPart, update: Partial<UiSettings>) => attempt(part, onChangeUi(update));
  const changeOverlay = (work: Promise<void>) => attempt('overlay', work.then(getOverlay), setOverlay);
  const hotkey = editedHotkey ?? ui.hotkey;
  const section = (part: SettingsPart) => ({ open: openParts.includes(part), onToggle: () => onToggle(part), error: errors[part] });

  useEffect(() => {
    const reload = () => attempt('overlay', getOverlay(), setOverlay);
    attempt('about', appInfo(), setInfo);
    reload();
    const subscription = onSettingsChanged(reload);
    return () => void subscription.then((unlisten) => unlisten());
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

  const applyHotkey = () => attempt('behaviour', onChangeUi({ hotkey: hotkey.trim() }), () => setEditedHotkey(null));
  const target = overlay?.displays.find((display) => display.target);

  return (
    <div>
      <SettingsSection label="Overlay" summary={overlaySummary(ui, overlay)} {...section('overlay')}>
        <SettingsRow label="Style" help="The edge tab is a thin strip on a screen edge; the pill floats near a corner." disabled={!overlay}>
          {(field) => (
            <SegmentedControl
              field={field}
              choices={STYLE_CHOICES}
              value={overlay?.style ?? 'tab'}
              disabled={!overlay}
              onChange={(style) => changeOverlay(setStyle(style))}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Display" help="The display the overlay and the panel appear on." disabled={!overlay}>
          {(field) => (
            <Select
              field={field}
              choices={(overlay?.displays ?? []).map((display) => ({ value: display.id, label: displayLabel(display) }))}
              text={target ? displayLabel(target) : 'None'}
              selected={(id) => id === target?.id}
              disabled={!overlay}
              onChoose={(id) => changeOverlay(setDisplay(id))}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Never show on" help="Displays Beacon never uses, even when the chosen one is gone." disabled={!overlay}>
          {(field) => (
            <Select
              field={field}
              multiple
              choices={(overlay?.displays ?? []).map((display) => ({ value: display.id, label: displayLabel(display) }))}
              text={avoidedLabel(overlay?.displays ?? [])}
              selected={(id) => overlay?.displays.some((display) => display.id === id && display.avoided) ?? false}
              disabled={!overlay}
              onChoose={(id) =>
                changeOverlay(setDisplayAvoided(id, !overlay?.displays.some((display) => display.id === id && display.avoided)))
              }
            />
          )}
        </SettingsRow>
        <SettingsRow label="Position" help="Where the overlay sits on its display." disabled={!overlay?.position}>
          {(field) => (
            <Select
              field={field}
              choices={positionChoices(overlay?.style ?? 'tab')}
              text={overlay?.position ? labelOf(positionChoices(overlay.style), overlay.position) : 'None'}
              selected={(position) => position === overlay?.position}
              disabled={!overlay?.position}
              onChoose={(position) => changeOverlay(setOverlayPosition(position))}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Show it" help="When the overlay is on screen.">
          {(field) => (
            <Select
              field={field}
              choices={VISIBILITY_CHOICES}
              text={labelOf(VISIBILITY_CHOICES, ui.visibility)}
              selected={(visibility) => visibility === ui.visibility}
              onChoose={(visibility) => change('overlay', { visibility })}
            />
          )}
        </SettingsRow>
        <SettingsRow
          label="Reveal on edge hover"
          help="While the overlay is hidden, hovering its screen edge brings it back."
          disabled={ui.visibility === 'never'}
        >
          {(field) => (
            <Switch
              field={field}
              checked={ui.reveal_on_hover}
              disabled={ui.visibility === 'never'}
              onChange={(reveal_on_hover) => change('overlay', { reveal_on_hover })}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Size" help="Size of the edge tab or the pill.">
          {(field) => (
            <SegmentedControl
              field={field}
              choices={SIZE_CHOICES}
              value={ui.size}
              onChange={(size) => change('overlay', { size })}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Idle opacity" help="Opacity of the overlay while nothing needs you.">
          {(field) => (
            <Slider
              field={field}
              value={ui.idle_opacity}
              range={OPACITY_RANGE}
              format={percent}
              onCommit={(idle_opacity) => change('overlay', { idle_opacity })}
            />
          )}
        </SettingsRow>
        <SettingsRow
          label="Pin while Claude is active"
          help="While Claude is working or needs you, the panel opens by itself without taking focus and stays open until 5 s after Claude is done."
        >
          {(field) => (
            <Switch
              field={field}
              checked={ui.pin_while_active}
              onChange={(pin_while_active) => change('overlay', { pin_while_active })}
            />
          )}
        </SettingsRow>
      </SettingsSection>
      <SettingsSection label="Attention" summary={attentionSummary(ui)} {...section('attention')}>
        <SettingsRow label="Style" help="How the overlay shows that a session needs you.">
          {(field) => (
            <SegmentedControl
              field={field}
              choices={ATTENTION_CHOICES}
              value={ui.attention_style}
              onChange={(attention_style) => change('attention', { attention_style })}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Remind me" help="How often the pulse repeats until you open the panel.">
          {(field) => (
            <Select
              field={field}
              choices={REPEAT_CHOICES}
              text={labelOf(REPEAT_CHOICES, ui.attention_repeat_seconds)}
              selected={(seconds) => seconds === ui.attention_repeat_seconds}
              onChoose={(attention_repeat_seconds) => change('attention', { attention_repeat_seconds })}
            />
          )}
        </SettingsRow>
        <SettingsRow
          label="Label stays for"
          help="How long the label naming the session stays out."
          disabled={ui.attention_style !== 'pulse_peek'}
        >
          {(field) => (
            <Slider
              field={field}
              value={ui.peek_seconds}
              range={PEEK_RANGE}
              format={(seconds) => `${seconds} s`}
              disabled={ui.attention_style !== 'pulse_peek'}
              onCommit={(peek_seconds) => change('attention', { peek_seconds })}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Notifications" help="A Windows notification when a session needs you.">
          {(field) => (
            <Switch
              field={field}
              checked={ui.toasts_enabled}
              onChange={(toasts_enabled) => change('attention', { toasts_enabled })}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Also when finished" help="Also notify when a session finishes." disabled={!ui.toasts_enabled}>
          {(field) => (
            <Switch
              field={field}
              checked={ui.toast_on_finished}
              disabled={!ui.toasts_enabled}
              onChange={(toast_on_finished) => change('attention', { toast_on_finished })}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Sounds" help="Play a sound when a session needs you or finishes.">
          {(field) => (
            <Switch
              field={field}
              checked={ui.sounds_enabled}
              onChange={(sounds_enabled) => change('attention', { sounds_enabled })}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Preview" help="Shows the attention style on the overlay once.">
          {(field) => (
            <OutlineButton
              compact
              id={field.id}
              aria-describedby={field.helpId}
              onClick={() => attempt('attention', previewAttention())}
            >
              Show
            </OutlineButton>
          )}
        </SettingsRow>
      </SettingsSection>
      <SettingsSection label="Startup" summary={startupSummary(ui)} {...section('startup')}>
        <SettingsRow label="Start Beacon" help="When Beacon starts by itself.">
          {(field) => (
            <Select
              field={field}
              choices={START_CHOICES}
              text={labelOf(START_CHOICES, ui.start_mode)}
              selected={(mode) => mode === ui.start_mode}
              onChoose={(start_mode) => change('startup', { start_mode })}
            />
          )}
        </SettingsRow>
        <SettingsRow
          label="Close when unused for"
          help="Beacon quits after Claude has been unused this long. Only when it starts with Claude."
          disabled={ui.start_mode !== 'claude'}
        >
          {(field) => (
            <Select
              field={field}
              choices={QUIT_CHOICES}
              text={labelOf(QUIT_CHOICES, ui.quit_when_idle_minutes)}
              selected={(minutes) => minutes === ui.quit_when_idle_minutes}
              disabled={ui.start_mode !== 'claude'}
              onChoose={(quit_when_idle_minutes) => change('startup', { quit_when_idle_minutes })}
            />
          )}
        </SettingsRow>
      </SettingsSection>
      <SettingsSection label="Behaviour" summary={behaviourSummary(ui, config)} {...section('behaviour')}>
        <SettingsRow
          label="Answer headless runs"
          help="Off: a claude -p run with nobody attached is passed straight through, so it never waits on you."
          disabled={!config}
        >
          {(field) => (
            <Switch
              field={field}
              checked={config?.answer_unattended ?? false}
              disabled={!config}
              onChange={(checked) => attempt('behaviour', setConfig({ answer_unattended: checked }))}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Stalled after" help="Mark a session as stalled after this many minutes without news. Enter to apply." disabled={!config}>
          {(field) => (
            <span className="flex shrink-0 items-center gap-1.5">
              <input
                id={field.id}
                aria-describedby={field.helpId}
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
              <span className="text-secondary text-muted">min</span>
            </span>
          )}
        </SettingsRow>
        <SettingsRow label="Clear finished after" help="A finished session is acknowledged by itself after this long." disabled={!config}>
          {(field) => (
            <Select
              field={field}
              choices={CLEAR_CHOICES}
              text={config ? labelOf(CLEAR_CHOICES, config.clear_finished_minutes) : ''}
              selected={(minutes) => minutes === config?.clear_finished_minutes}
              disabled={!config}
              onChoose={(clear_finished_minutes) => attempt('behaviour', setConfig({ clear_finished_minutes }))}
            />
          )}
        </SettingsRow>
        <SettingsRow label="Shortcut" help="Opens and closes the panel from anywhere.">
          {(field) => (
            <span className="flex min-w-0 items-center gap-1.5">
              <input
                id={field.id}
                aria-describedby={field.helpId}
                placeholder="Ctrl+Alt+B"
                value={hotkey}
                onChange={(event) => setEditedHotkey(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && hotkey.trim()) applyHotkey();
                }}
                className={`${FIELD} w-40 min-w-0`}
              />
              <OutlineButton compact disabled={!hotkey.trim() || hotkey.trim() === ui.hotkey} onClick={applyHotkey}>
                Apply
              </OutlineButton>
            </span>
          )}
        </SettingsRow>
      </SettingsSection>
      <HooksSection open={openParts.includes('hooks')} onToggle={() => onToggle('hooks')} />
      <SettingsSection label="About" summary={info ? `Beacon ${info.version}` : ''} {...section('about')}>
        <SettingsRow label="Version">
          {(field) => (
            <span id={field.id} className="font-mono text-secondary text-muted tabular-nums">
              {info?.version}
            </span>
          )}
        </SettingsRow>
        <SettingsRow label="Home" help={info?.home}>
          {(field) => (
            <span id={field.id} className="min-w-0 truncate font-mono text-micro text-muted">
              {info?.home}
            </span>
          )}
        </SettingsRow>
        <div className="flex h-7 items-center justify-between px-2.5">
          <TextButton onClick={() => attempt('about', openLogsFolder())}>Open logs folder</TextButton>
          <TextButton onClick={() => attempt('about', quitApp())}>Quit Beacon</TextButton>
        </div>
      </SettingsSection>
    </div>
  );
}
