type Point = { x: number; y: number };
type Kind = 'needs' | 'working' | 'done' | 'idle';

interface Preview {
  emit: (name: string, payload?: unknown) => void;
  snapshot: { sessions: Record<string, unknown>[] };
  ui: Record<string, unknown>;
}

const VIEW = { width: 1280, height: 720 };
const world = document.getElementById('world') as HTMLDivElement;
const frame = document.getElementById('panel') as HTMLIFrameElement;
const cursor = document.getElementById('cursor') as unknown as SVGSVGElement;
const ripple = document.getElementById('ripple') as HTMLDivElement;
const pill = document.getElementById('pill') as HTMLDivElement;
const endcard = document.getElementById('endcard') as HTMLDivElement;
const intro = document.getElementById('intro') as HTMLDivElement;
const chip = document.getElementById('step') as HTMLDivElement;

const MARK: Record<Kind, string> = {
  working:
    '<svg viewBox="0 0 16 16" class="spin"><path d="M12 8A4 4 0 1 1 8 4" fill="none" stroke="var(--ink)" stroke-width="1.9" stroke-linecap="round"/></svg>',
  needs: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="4.6" fill="var(--wip)"/></svg><span class="halo"></span>',
  done: '<svg viewBox="0 0 16 16"><path d="M4.4 8.3 7 10.8l4.7-5.2" fill="none" stroke="var(--live)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  idle: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="3.6" fill="none" stroke="var(--muted)" stroke-width="1.4"/></svg>',
};

let marks: Kind[] = ['needs', 'needs', 'working', 'working', 'done', 'idle'];
let open = false;
let position: Point = { x: 470, y: 430 };

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const ease = (t: number) => -(Math.cos(Math.PI * t) - 1) / 2;

function drawPill() {
  const count = marks.filter((kind) => kind === 'needs').length;
  pill.innerHTML = [
    ...marks.map((kind) => `<span class="mark">${MARK[kind]}</span>`),
    count > 0 ? `<span class="count">${count}</span>` : '',
    `<span class="chevron${open ? ' up' : ''}"><svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 4.5 6 7.5l3-3" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg></span>`,
  ].join('');
}

function place(point: Point) {
  position = point;
  cursor.style.translate = `${point.x - 3}px ${point.y - 2}px`;
}

const preview = () => (frame.contentWindow as unknown as { __beaconPreview: Preview }).__beaconPreview;
const inner = () => frame.contentDocument as Document;

function framePoint(element: Element): Point {
  const rect = element.getBoundingClientRect();
  return { x: frame.offsetLeft + rect.left + rect.width / 2, y: frame.offsetTop + rect.top + rect.height / 2 };
}

function pillPoint(element: HTMLElement): Point {
  return {
    x: pill.offsetLeft + element.offsetLeft + element.offsetWidth / 2,
    y: pill.offsetTop + element.offsetTop + element.offsetHeight / 2,
  };
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
  const duration = Math.min(1400, Math.max(750, 600 + distance * 0.9));
  const bend = { x: -(target.y - from.y) * 0.06, y: (target.x - from.x) * 0.06 };
  const control = { x: (from.x + target.x) / 2 + bend.x, y: (from.y + target.y) / 2 + bend.y };
  const start = performance.now();
  return new Promise((resolve) => {
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
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
      .getAnimations({ subtree: true })
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
  await wait(320);
  await press(() => element.click());
  await still();
}

async function step(number: number, text: string) {
  chip.classList.remove('on');
  await wait(chip.textContent ? 280 : 0);
  chip.innerHTML = `<span class="step-number">${number}</span>${text}`;
  chip.classList.add('on');
}

function settle(title: string, fields: Record<string, unknown>) {
  const state = preview().snapshot;
  state.sessions = state.sessions.map((session) => (session.display_title === title ? { ...session, ...fields } : session));
  preview().emit('beacon://snapshot', state);
}

function zoom(scale: number, focus: Point) {
  const x = Math.min(0, Math.max(VIEW.width - VIEW.width * scale, VIEW.width / 2 - focus.x * scale));
  const y = Math.min(0, Math.max(VIEW.height - VIEW.height * scale, VIEW.height / 2 - focus.y * scale));
  world.style.transform = `translate3d(${x}px, ${y}px, 0) scale(${scale})`;
}

async function type(text: string) {
  const field = inPanel('textarea') as HTMLTextAreaElement;
  const view = frame.contentWindow as unknown as typeof window;
  const setter = Object.getOwnPropertyDescriptor(view.HTMLTextAreaElement.prototype, 'value')?.set;
  for (let index = 1; index <= text.length; index++) {
    setter?.call(field, text.slice(0, index));
    field.dispatchEvent(new view.Event('input', { bubbles: true }));
    await wait(55);
  }
}

async function ready() {
  await new Promise<void>((resolve) =>
    frame.contentDocument?.readyState === 'complete' ? resolve() : frame.addEventListener('load', () => resolve(), { once: true }),
  );
  const style = inner().createElement('style');
  style.textContent =
    'html{background:transparent!important}body{margin:0!important;width:376px!important}#root>section{height:auto!important}*:focus-visible,label:has(:focus-visible){outline:none!important}';
  inner().head.append(style);
  await wait(900);
}

async function play() {
  await wait(1900);
  intro.classList.add('off');
  cursor.classList.add('shown');
  await wait(700);
  await step(1, 'Open Beacon from the pill');
  await wait(700);
  const chevron = pill.querySelector('.chevron') as HTMLElement;
  await moveTo(pillPoint(chevron));
  chevron.classList.add('hover');
  await wait(450);
  await press(() => {
    open = true;
    drawPill();
    frame.classList.add('open');
    preview().emit('beacon://closing');
    setTimeout(
      () => preview().emit('beacon://opened', { mode: 'sticky', focused: true, edge: 'top', snapshot: preview().snapshot, ui: preview().ui }),
      30,
    );
  });
  await wait(700);
  zoom(1.15, { x: 1100, y: 313 });
  await wait(1300);

  await step(2, 'Approve a permission request');
  await wait(350);
  await clickPanel('button', 'Allow');
  marks = ['working', 'needs', 'working', 'working', 'done', 'idle'];
  drawPill();
  await wait(500);

  await step(3, 'Answer a question');
  await wait(350);
  await clickPanel('button', 'Answer');
  settle('Fix the flaky login test', { state: 'working', alerting: false, attention: false, step_current: { kind: 'run', label: 'Running npm test -- auth/login.spec.ts' } });
  await wait(500);
  await clickElement(inPanel('label', 'Redis').querySelector('input') as HTMLElement);
  await clickPanel('button', 'Send answer');
  marks = ['working', 'working', 'working', 'working', 'done', 'idle'];
  drawPill();
  settle('Choose a cache for the tile server', { state: 'working', alerting: false, attention: false });
  await wait(400);
  await clickPanel('button[aria-label="Back to sessions"]');
  await wait(300);

  await step(4, 'Send a session a prompt');
  await wait(350);
  await clickPanel('span', 'Draft the 0.2.0 release notes');
  await wait(400);
  await clickPanel('textarea');
  await type('Open a pull request with these notes');
  await wait(450);
  await clickPanel('button', 'Send');
  await wait(1200);

  chip.classList.remove('on');
  zoom(1, { x: 640, y: 360 });
  await moveTo({ x: 560, y: 470 });
  await wait(900);
  endcard.classList.add('on');
  await wait(2800);
}

drawPill();
place(position);
Object.assign(window, { demo: { ready, play } });
