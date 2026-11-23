import { useState } from 'react';
import { OutlineButton } from './OutlineButton';

interface MessageFieldProps {
  label: string;
  placeholder: string;
  sendLabel: string;
  disabled: boolean;
  onSend: (message: string) => void;
  onCancel: () => void;
}

export function MessageField({ label, placeholder, sendLabel, disabled, onSend, onCancel }: MessageFieldProps) {
  const [message, setMessage] = useState('');
  const send = () => {
    if (message.trim()) onSend(message.trim());
  };
  return (
    <div className="mt-2 flex origin-top-left animate-appear items-center gap-1.5">
      <input
        autoFocus
        aria-label={label}
        placeholder={placeholder}
        value={message}
        disabled={disabled}
        onChange={(event) => setMessage(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.nativeEvent.isComposing) send();
          if (event.key === 'Escape') {
            event.stopPropagation();
            onCancel();
          }
        }}
        className="h-6 min-w-0 flex-1 rounded-field border border-line bg-paper px-2 text-secondary text-ink placeholder:text-faint focus:border-muted focus:outline-none"
      />
      <OutlineButton disabled={disabled || !message.trim()} onClick={send}>
        {sendLabel}
      </OutlineButton>
      <OutlineButton disabled={disabled} onClick={onCancel}>
        Cancel
      </OutlineButton>
    </div>
  );
}
