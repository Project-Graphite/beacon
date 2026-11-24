import { useState } from 'react';
import { SCOPES, addRules, parseRule, type AddRules, type RuleDestination } from '../lib/rules.ts';
import { FilledButton } from './FilledButton';
import { OutlineButton } from './OutlineButton';

interface RuleEditorProps {
  initialRule: string;
  initialDestination: RuleDestination;
  disabled: boolean;
  onSave: (update: AddRules) => void;
  onCancel: () => void;
}

export function RuleEditor({ initialRule, initialDestination, disabled, onSave, onCancel }: RuleEditorProps) {
  const [ruleText, setRuleText] = useState(initialRule);
  const [destination, setDestination] = useState(initialDestination);
  const rule = parseRule(ruleText);
  return (
    <div
      className="mt-2 origin-top-left animate-appear space-y-1.5"
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.stopPropagation();
          onCancel();
        }
      }}
    >
      <input
        autoFocus
        aria-label="Permission rule"
        spellCheck={false}
        value={ruleText}
        disabled={disabled}
        onChange={(event) => setRuleText(event.target.value)}
        className={`h-7 w-full rounded-field border bg-raised px-2.5 font-mono text-secondary text-ink shadow-card focus:outline-none ${rule ? 'border-line focus:border-faint' : 'border-danger'}`}
      />
      <select
        aria-label="Where the rule applies"
        value={destination}
        disabled={disabled}
        onChange={(event) => setDestination(event.target.value as RuleDestination)}
        className="h-7 w-full rounded-field border border-line bg-raised px-2 text-secondary text-ink shadow-card focus:border-faint focus:outline-none"
      >
        {SCOPES.map((scope) => (
          <option key={scope.destination} value={scope.destination}>
            {scope.label}
          </option>
        ))}
      </select>
      <p className="text-micro text-muted">
        Claude Code saves this as a normal permission rule; Beacon keeps no rules of its own.
      </p>
      <div className="flex items-center gap-1.5">
        <FilledButton disabled={disabled || rule === null} onClick={() => rule && onSave(addRules(rule, destination))}>
          Allow and save rule
        </FilledButton>
        <OutlineButton disabled={disabled} onClick={onCancel}>
          Cancel
        </OutlineButton>
      </div>
    </div>
  );
}
