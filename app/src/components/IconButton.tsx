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
      className="press grid size-7 shrink-0 place-items-center rounded-[7px] text-faint focus-visible:text-ink enabled:hover:bg-surface enabled:hover:text-ink disabled:opacity-35"
      {...props}
    >
      <Icon name={icon} className={`size-4 ${iconClassName ?? ''}`} />
    </button>
  );
}
