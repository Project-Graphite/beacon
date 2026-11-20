export type RuleDestination = 'session' | 'localSettings' | 'projectSettings' | 'userSettings';

export interface PermissionRule {
  toolName: string;
  ruleContent?: string;
}

export interface AddRules {
  type: 'addRules';
  rules: PermissionRule[];
  behavior: 'allow';
  destination: RuleDestination;
}

export const SCOPES: { destination: RuleDestination; label: string }[] = [
  { destination: 'session', label: 'This session' },
  { destination: 'localSettings', label: 'This project, only me' },
  { destination: 'projectSettings', label: 'This project, shared' },
  { destination: 'userSettings', label: 'All my projects' },
];

const isDestination = (value: unknown): value is RuleDestination =>
  SCOPES.some((scope) => scope.destination === value);

export function parseRule(text: string): PermissionRule | null {
  const open = text.indexOf('(');
  const toolName = open === -1 ? text : text.slice(0, open);
  if (toolName === '' || /[\s)]/.test(toolName)) return null;
  if (open === -1) return { toolName };
  const ruleContent = text.endsWith(')') ? text.slice(open + 1, -1) : '';
  return ruleContent === '' ? null : { toolName, ruleContent };
}

export function formatRule(rule: PermissionRule): string {
  return rule.ruleContent === undefined ? rule.toolName : `${rule.toolName}(${rule.ruleContent})`;
}

export function suggestedRule(
  toolName: string,
  permissionSuggestions: unknown,
): { text: string; destination: RuleDestination } {
  const suggestion = (Array.isArray(permissionSuggestions) ? permissionSuggestions : []).find(
    (entry) => entry?.type === 'addRules' && Array.isArray(entry.rules) && typeof entry.rules[0]?.toolName === 'string',
  );
  const rule = suggestion?.rules[0];
  return {
    text: rule
      ? formatRule({ toolName: rule.toolName, ruleContent: typeof rule.ruleContent === 'string' ? rule.ruleContent : undefined })
      : toolName,
    destination: isDestination(suggestion?.destination) ? suggestion.destination : 'session',
  };
}

export function addRules(rule: PermissionRule, destination: RuleDestination): AddRules {
  return { type: 'addRules', rules: [rule], behavior: 'allow', destination };
}
