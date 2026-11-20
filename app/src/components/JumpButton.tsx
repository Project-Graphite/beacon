import { Icon } from './Icon';
import { KeyHint } from './KeyHint';
import { OutlineButton } from './OutlineButton';

interface JumpButtonProps {
  shortcut?: boolean;
  disabled?: boolean;
  onJump: () => void;
}

export function JumpButton({ shortcut, disabled, onJump }: JumpButtonProps) {
  return (
    <OutlineButton title="Jump to the session's window" disabled={disabled} onClick={onJump}>
      <Icon name="arrow-out" className="mr-1 size-3" />
      Jump
      {shortcut && <KeyHint shortcut="J" />}
    </OutlineButton>
  );
}
