type Point = { x: number; y: number };
type Kind = 'needs' | 'working' | 'done' | 'idle';

interface Session {
  session_id: string;
  display_title: string;
  state: string;
  alerting: boolean;
  dismissed: boolean;
  prompts: { status: string }[];
}

interface Preview {
  emit: (name: string, payload?: unknown) => void;
  listen: (name: string, handler: () => void) => Promise<() => void>;
  snapshot: { sessions: Session[] };
}

const frame = document.getElementById('panel') as HTMLIFrameElement;
const cursor = document.getElementById('cursor') as unknown as SVGSVGElement;
const ripple = document.getElementById('ripple') as HTMLDivElement;
const pill = document.getElementById('pill') as HTMLDivElement;
const steps = [...document.querySelectorAll<HTMLLIElement>('.steps li')];

const MARK: Record<Kind, string> = {
  working:
    '<svg viewBox="0 0 16 16" class="spin"><path d="M12 8A4 4 0 1 1 8 4" fill="none" stroke="var(--ink)" stroke-width="1.9" stroke-linecap="round"/></svg>',
  needs: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="4.6" fill="var(--wip)"/></svg><span class="halo"></span>',
  done: '<svg viewBox="0 0 16 16"><path d="M4.4 8.3 7 10.8l4.7-5.2" fill="none" stroke="var(--live)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  idle: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="3.6" fill="none" stroke="var(--muted)" stroke-width="1.4"/></svg>',
};

const REST: Point = { x: 176, y: 520 };
let position = REST;
let start = '';

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const ease = (t: number) => -(Math.cos(Math.PI * t) - 1) / 2;
const rank = (session: Session) => (session.alerting ? 0 : session.state === 'working' ? 1 : 2);

const preview = () => (frame.contentWindow as unknown as { __beaconPreview: Preview }).__beaconPreview;
const inner = () => frame.contentDocument as Document;

function drawPill() {
  const sessions = preview()
    .snapshot.sessions.filter((session) => !session.dismissed)
    .sort((a, b) => rank(a) - rank(b) || a.session_id.localeCompare(b.session_id));
  const count = sessions.filter((session) => session.alerting).length;
  pill.innerHTML = [
    ...sessions.map((session) => `<span class="mark">${MARK[(session.state.startsWith('needs') ? 'needs' : session.state) as Kind]}</span>`),
    count > 0 ? `<span class="count">${count}</span>` : '',
    '<span class="chevron"><svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 4.5 6 7.5l3-3" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg></span>',
  ].join('');
}

function place(point: Point) {
  position = point;
  cursor.style.translate = `${point.x - 3}px ${point.y - 2}px`;
}

function framePoint(element: Element): Point {
  const rect = element.getBoundingClientRect();
  return { x: frame.offsetLeft + rect.left + rect.width / 2, y: frame.offsetTop + rect.top + rect.height / 2 };
}

function inPanel(selector: string, text?: string): HTMLElement {
  const found = [...inner().querySelectorAll<HTMLElement>(selector)].find(
    (element) => text === undefined || element.textContent?.trim().startsWith(text),
  );
  if (!found) throw new Error(`not found: ${selector} ${text ?? ''}`);
  return found;
}

function moveTo(target: Point): Promise<void> {
  const from = position;
  const distance = Math.hypot(target.x - from.x, target.y - from.y);
  const duration = Math.min(1100, Math.max(650, 550 + distance * 0.9));
  const bend = { x: -(target.y - from.y) * 0.06, y: (target.x - from.x) * 0.06 };
  const control = { x: (from.x + target.x) / 2 + bend.x, y: (from.y + target.y) / 2 + bend.y };
  const begin = performance.now();
  return new Promise((resolve) => {
    const step = (now: number) => {
      const t = Math.min(1, (now - begin) / duration);
      const k = ease(t);
      const a = (1 - k) * (1 - k);
      const b = 2 * (1 - k) * k;
      const c = k * k;
      place({ x: a * from.x + b * control.x + c * target.x, y: a * from.y + b * control.y + c * target.y });
      if (t < 1) requestAnimationFrame(step);
      else resolve();
    };
    requestAnimationFrame(step);
  });
}

async function press(action: () => void) {
  cursor.classList.add('pressed');
  ripple.style.left = `${position.x}px`;
  ripple.style.top = `${position.y}px`;
  ripple.classList.remove('go');
  void ripple.offsetWidth;
  ripple.classList.add('go');
  await wait(140);
  action();
  cursor.classList.remove('pressed');
}

async function clickPanel(selector: string, text?: string) {
  await clickElement(inPanel(selector, text));
}

async function still() {
  const busy = () =>
    inner()
      .getAnimations()
      .some(
        (animation) =>
          animation.playState === 'running' &&
          animation.effect?.getTiming().iterations !== Infinity &&
          !(animation instanceof CSSAnimation && /^roll-/.test(animation.animationName)),
      );
  const until = performance.now() + 450;
  await wait(120);
  while (busy() && performance.now() < until) await wait(60);
  await wait(200);
}

async function clickElement(element: HTMLElement) {
  await moveTo(framePoint(element));
  await wait(280);
  await press(() => element.click());
  await still();
}

function step(index: number) {
  steps.forEach((item, at) => item.classList.toggle('on', at === index));
}

function settle(title: string, change: (session: Session) => Record<string, unknown>) {
  const state = preview().snapshot;
  state.sessions = state.sessions.map((session) => (session.display_title === title ? { ...session, ...change(session) } : session));
  preview().emit('beacon://snapshot', state);
}

async function type(text: string) {
  const field = inPanel('textarea') as HTMLTextAreaElement;
  const view = frame.contentWindow as unknown as typeof window;
  const setter = Object.getOwnPropertyDescriptor(view.HTMLTextAreaElement.prototype, 'value')?.set;
  for (let index = 1; index <= text.length; index++) {
    setter?.call(field, text.slice(0, index));
    field.dispatchEvent(new view.Event('input', { bubbles: true }));
    await wait(45);
  }
}

async function ready() {
  await new Promise<void>((resolve) =>
    frame.contentDocument?.readyState === 'complete' ? resolve() : frame.addEventListener('load', () => resolve(), { once: true }),
  );
  const style = inner().createElement('style');
  style.textContent =
    'html{background:transparent!important}body{margin:0!important;width:376px!important}body #root>section{height:100vh!important}*:focus-visible,label:has(:focus-visible){outline:none!important}';
  inner().head.append(style);
  start = JSON.stringify(preview().snapshot);
  await preview().listen('beacon://snapshot', drawPill);
  drawPill();
  await wait(900);
}

async function play() {
  await wait(1400);

  step(1);
  settle('Fix the flaky login test', () => ({ step_current: { kind: 'run', label: 'Running npm test -- auth/login.spec.ts' } }));
  await clickPanel('button', 'Allow');
  await wait(900);

  step(2);
  await clickPanel('button', 'Answer');
  await clickElement(inPanel('label', 'Redis').querySelector('input') as HTMLElement);
  await clickPanel('button', 'Send answer');
  await wait(500);
  await clickPanel('button[aria-label="Back to sessions"]');

  step(3);
  await clickPanel('span', 'Draft the 0.2.0 release notes');
  await clickPanel('textarea');
  await type('Open a pull request with these notes');
  await wait(300);
  await clickPanel('button', 'Send');
  await wait(900);
  settle('Draft the 0.2.0 release notes', (session) => ({
    state: 'working',
    alerting: false,
    attention: false,
    turn_started_ms: Date.now(),
    step_current: { kind: 'read', label: 'Reading docs/notes/0.2.0.md' },
    prompts: session.prompts.map((prompt) => ({ ...prompt, status: 'acknowledged' })),
  }));
  await wait(1100);
  await clickPanel('button[aria-label="Back to sessions"]');
  await wait(500);

  step(0);
  await moveTo(REST);
  Object.assign(preview().snapshot, JSON.parse(start));
  preview().emit('beacon://snapshot', preview().snapshot);
  await wait(500);
  const view = frame.contentWindow as unknown as typeof window;
  for (let count = 0; count < 2; count++) {
    view.dispatchEvent(new view.KeyboardEvent('keydown', { key: 'ArrowUp' }));
    await wait(180);
  }
  await wait(700);
}

place(position);
step(0);
Object.assign(window, { demo: { ready, play } });

export {};
