import type { ButtonHTMLAttributes } from 'react';
import { Icon, type IconName } from './Icon';

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: IconName;
  iconClassName?: string;
}

export function IconButton({ icon, iconClassName, ...props }: IconButtonProps) {
  return (
    <button
      type="button"
      aria-label={props.title}
      className="press grid size-5 shrink-0 place-items-center rounded-[5px] text-faint focus-visible:text-ink enabled:hover:bg-line-soft enabled:hover:text-ink disabled:opacity-40"
      {...props}
    >
      <Icon name={icon} className={`size-3.5 ${iconClassName ?? ''}`} />
    </button>
  );
}
