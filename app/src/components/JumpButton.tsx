import { Icon } from './Icon';
import { KeyHint } from './KeyHint';
import { OutlineButton } from './OutlineButton';
import { TextButton } from './TextButton';

interface JumpButtonProps {
  shortcut?: boolean;
  quiet?: boolean;
  disabled?: boolean;
  onJump: () => void;
}

export function JumpButton({ shortcut, quiet, disabled, onJump }: JumpButtonProps) {
  const Button = quiet ? TextButton : OutlineButton;
  return (
    <Button title="Jump to the session's window" disabled={disabled} onClick={onJump}>
      <Icon name="arrow-out" className="mr-1 size-3" />
      Jump
      {shortcut && <KeyHint shortcut="J" />}
    </Button>
  );
}
