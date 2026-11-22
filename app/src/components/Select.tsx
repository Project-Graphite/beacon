import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import { menuOpensUp, selectKey } from '../lib/select.ts';
import type { Choice } from '../lib/settings.ts';
import { Icon } from './Icon';
import type { Field } from './SettingsRow';

const OPTION_PX = 28;
const MENU_PADDING_PX = 10;
const GAP_PX = 4;

interface SelectProps<T extends string | number> {
  field: Field;
  choices: Choice<T>[];
  text: string;
  selected: (value: T) => boolean;
  multiple?: boolean;
  disabled?: boolean;
  onChoose: (value: T) => void;
}

export function Select<T extends string | number>({
  field,
  choices,
  text,
  selected,
  multiple,
  disabled,
  onChoose,
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [place, setPlace] = useState<CSSProperties>({});
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLUListElement>(null);
  const listId = `${field.id}list`;
  const optionId = (index: number) => `${field.id}option${index}`;

  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: PointerEvent) => {
      const inside = [trigger.current, menu.current].some(
        (element) => event.target instanceof Node && element?.contains(event.target),
      );
      if (!inside) setOpen(false);
    };
    const close = () => setOpen(false);
    window.addEventListener('pointerdown', closeOutside);
    document.addEventListener('scroll', close, true);
    return () => {
      window.removeEventListener('pointerdown', closeOutside);
      document.removeEventListener('scroll', close, true);
    };
  }, [open]);

  const show = (index: number) => {
    const rect = trigger.current?.getBoundingClientRect();
    if (!rect) return;
    const up = menuOpensUp(rect, choices.length * OPTION_PX + MENU_PADDING_PX, window.innerHeight, GAP_PX);
    setPlace({
      right: window.innerWidth - rect.right,
      minWidth: rect.width,
      ...(up ? { bottom: window.innerHeight - rect.top + GAP_PX } : { top: rect.bottom + GAP_PX }),
    });
    setActive(index);
    setOpen(true);
  };

  const choose = (index: number) => {
    onChoose(choices[index].value);
    if (!multiple) setOpen(false);
  };

  const selectedIndex = choices.findIndex((choice) => selected(choice.value));

  return (
    <>
      <button
        ref={trigger}
        type="button"
        role="combobox"
        id={field.id}
        aria-describedby={field.helpId}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={open ? optionId(active) : undefined}
        disabled={disabled}
        onClick={() => (open ? setOpen(false) : show(Math.max(selectedIndex, 0)))}
        onKeyDown={(event) => {
          const step = selectKey({ open, active }, event.key, choices.length, selectedIndex);
          if (!step) return;
          if (event.key !== 'Tab') event.preventDefault();
          if (open) event.stopPropagation();
          if (!open) return show(step.active);
          setActive(step.active);
          if (step.choose) choose(step.active);
          else if (!step.open) setOpen(false);
        }}
        className={`flex h-6 max-w-44 min-w-0 shrink items-center gap-1 rounded-field border bg-paper pr-1 pl-2 text-secondary text-ink disabled:cursor-default ${open ? 'border-muted' : 'border-line hover:border-muted'}`}
      >
        <span className="min-w-0 truncate">{text}</span>
        <Icon name="chevron" className="size-3.5 shrink-0 rotate-90 text-muted" />
      </button>
      {open &&
        createPortal(
          <ul
            ref={menu}
            role="listbox"
            id={listId}
            aria-labelledby={field.labelId}
            aria-multiselectable={multiple}
            style={place}
            className={`fixed z-30 max-w-[calc(100vw-16px)] animate-appear rounded-field border border-line bg-raised p-1 text-secondary shadow-popover ${'bottom' in place ? 'origin-bottom-right' : 'origin-top-right'}`}
          >
            {choices.map((choice, index) => (
              <li
                key={choice.value}
                role="option"
                id={optionId(index)}
                aria-selected={selected(choice.value)}
                title={choice.help}
                onPointerDown={(event) => event.preventDefault()}
                onPointerEnter={() => setActive(index)}
                onClick={() => choose(index)}
                className={`flex h-7 cursor-default items-center justify-between gap-3 rounded-[6px] px-2 text-ink ${index === active ? 'bg-line-soft' : ''}`}
              >
                <span className="truncate">{choice.label}</span>
                <Icon
                  name="check"
                  className={`size-3.5 shrink-0 text-ink ${selected(choice.value) ? '' : 'opacity-0'}`}
                />
              </li>
            ))}
          </ul>,
          document.body,
        )}
    </>
  );
}
