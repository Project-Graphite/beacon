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
    <div
      className="mt-2 flex origin-top-left animate-appear items-center gap-1.5"
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.stopPropagation();
          onCancel();
        }
      }}
    >
      <input
        autoFocus
        aria-label={label}
        placeholder={placeholder}
        value={message}
        disabled={disabled}
        onChange={(event) => setMessage(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.nativeEvent.isComposing) send();
        }}
        className="h-7 min-w-0 flex-1 rounded-field border border-line bg-raised px-2.5 text-secondary text-ink shadow-card placeholder:text-faint"
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
