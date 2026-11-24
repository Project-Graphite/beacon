type Point = { x: number; y: number };
type Kind = 'needs' | 'working' | 'done' | 'idle';

interface Preview {
  emit: (name: string, payload?: unknown) => void;
  snapshot: { sessions: Record<string, unknown>[] };
  ui: Record<string, unknown>;
}

const VIEW = { width: 1280, height: 720 };
const CAMERA = 1.3;
const world = document.getElementById('world') as HTMLDivElement;
const frame = document.getElementById('panel') as HTMLIFrameElement;
const cursor = document.getElementById('cursor') as unknown as SVGSVGElement;
const ripple = document.getElementById('ripple') as HTMLDivElement;
const caption = document.getElementById('caption') as HTMLDivElement;
const pill = document.getElementById('pill') as HTMLDivElement;
const endcard = document.getElementById('endcard') as HTMLDivElement;

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
  cursor.style.transform = `translate(${point.x - 3}px, ${point.y - 2}px)`;
}

const preview = () => (frame.contentWindow as unknown as { __beaconPreview: Preview }).__beaconPreview;
const inner = () => frame.contentDocument as Document;

function worldPoint(element: Element, inFrame: boolean): Point {
  const rect = element.getBoundingClientRect();
  if (inFrame) return { x: frame.offsetLeft + rect.left + rect.width / 2, y: frame.offsetTop + rect.top + rect.height / 2 };
  const view = world.getBoundingClientRect();
  const scale = view.width / VIEW.width;
  return { x: (rect.left - view.left + rect.width / 2) / scale, y: (rect.top - view.top + rect.height / 2) / scale };
}

function inPanel(selector: string, text?: string): Element {
  const found = [...inner().querySelectorAll(selector)].find((element) => text === undefined || element.textContent?.trim() === text);
  if (!found) throw new Error(`not found: ${selector} ${text ?? ''}`);
  return found;
}

const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

function moveTo(target: Point): Promise<void> {
  const from = position;
  const distance = Math.hypot(target.x - from.x, target.y - from.y);
  const duration = Math.min(1250, Math.max(520, 420 + distance * 0.75));
  const bend = { x: -(target.y - from.y) * 0.12, y: (target.x - from.x) * 0.12 };
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
  await wait(110);
  action();
  cursor.classList.remove('pressed');
}

async function reach(element: Element) {
  element.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  await wait(380);
  const point = worldPoint(element, true);
  zoom(CAMERA, point);
  await moveTo(point);
}

async function clickPanel(selector: string, text?: string, settle = 260) {
  const element = inPanel(selector, text);
  await reach(element);
  await wait(settle);
  await press(() => (element as HTMLElement).click());
}

function zoom(scale: number, focus: Point) {
  const x = Math.min(0, Math.max(VIEW.width - VIEW.width * scale, VIEW.width / 2 - focus.x * scale));
  const y = Math.min(0, Math.max(VIEW.height - VIEW.height * scale, VIEW.height / 2 - focus.y * scale));
  world.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
}

async function say(text: string | null) {
  caption.classList.remove('on');
  if (text === null) return;
  await wait(text && caption.textContent ? 260 : 0);
  caption.textContent = text;
  caption.classList.add('on');
}

function settleSession(title: string, fields: Record<string, unknown>) {
  const state = preview().snapshot;
  state.sessions = state.sessions.map((session) => (session.display_title === title ? { ...session, ...fields } : session));
  preview().emit('beacon://snapshot', state);
}

async function type(text: string) {
  const field = inPanel('textarea') as HTMLTextAreaElement;
  const view = frame.contentWindow as unknown as typeof window;
  const setter = Object.getOwnPropertyDescriptor(view.HTMLTextAreaElement.prototype, 'value')?.set;
  for (let index = 1; index <= text.length; index++) {
    setter?.call(field, text.slice(0, index));
    field.dispatchEvent(new view.Event('input', { bubbles: true }));
    await wait(38 + Math.random() * 40);
  }
}

async function ready() {
  await new Promise<void>((resolve) => (frame.contentDocument?.readyState === 'complete' ? resolve() : frame.addEventListener('load', () => resolve(), { once: true })));
  const style = inner().createElement('style');
  style.textContent = 'html{background:transparent!important}body{margin:0!important;width:376px!important}#root>section{height:100vh!important}*:focus-visible,label:has(:focus-visible){outline:none!important}';
  inner().head.append(style);
  await wait(900);
}

async function partOne() {
  await say('Beacon watches every Claude Code session on your PC');
  await wait(2100);
  const chevron = pill.querySelector('.chevron') as HTMLElement;
  zoom(CAMERA, { x: 1120, y: 180 });
  await moveTo(worldPoint(chevron, false));
  chevron.classList.add('hover');
  await wait(380);
  await press(() => {
    open = true;
    drawPill();
    frame.classList.add('open');
    preview().emit('beacon://closing');
    setTimeout(() => preview().emit('beacon://opened', { mode: 'sticky', focused: true, edge: 'top', snapshot: preview().snapshot, ui: preview().ui }), 30);
  });
  await wait(900);

  await say('Answer permission prompts without switching windows');
  await clickPanel('button', 'Allow', 420);
  await wait(500);
  marks = ['working', 'needs', 'working', 'working', 'done', 'idle'];
  drawPill();
  settleSession('Fix the flaky login test', { state: 'working', alerting: false, attention: false, step_current: { kind: 'run', label: 'Running npm test -- auth/login.spec.ts' } });
  await wait(1300);

  await say('Questions and plans, right from the panel');
  await clickPanel('button', 'Answer');
  await wait(1300);
  const redis = [...inner().querySelectorAll('label')].find((label) => label.textContent?.startsWith('Redis'));
  if (redis) {
    await reach(redis);
    await wait(220);
    await press(() => (redis as HTMLElement).click());
  }
  await wait(600);
  await clickPanel('button', 'Send answerA');
  marks = ['working', 'working', 'working', 'working', 'done', 'idle'];
  drawPill();
  settleSession('Choose a cache for the tile server', { state: 'working', alerting: false, attention: false });
  await wait(1200);
  await clickPanel('button[aria-label="Back to sessions"]');
  await wait(1000);

  await say('Send a prompt; Beacon delivers it, even to an idle session');
  await clickPanel('span', 'Draft the 0.2.0 release notes');
  await wait(1400);
  await clickPanel('textarea');
  await wait(200);
  await type('Open a pull request with these notes');
  await wait(350);
  await clickPanel('button', 'Send');
  await wait(1500);
  await clickPanel('button[aria-label="Back to sessions"]');
  await wait(1000);

  await say('Hide what you are done with; undo is one click away');
  const row = [...inner().querySelectorAll('li')].find((item) => item.textContent?.includes('Tidy the PowerShell profile'));
  const hide = row?.querySelector('button[title^="Hide until"]');
  if (hide) {
    await reach(hide);
    await wait(300);
    await press(() => (hide as HTMLElement).click());
  }
  await wait(1700);
  await clickPanel('button', 'Undo');
  await wait(1300);
}

async function partTwo() {
  await say('It follows your Windows theme, light or dark');
  await wait(2200);
  await clickPanel('button[aria-label="Settings"]');
  await wait(900);
  await clickPanel('section[aria-label="Overlay"] > button');
  await wait(1000);
  await say('Solid or invisible: with no background, only the marks float');
  await clickPanel('label', 'None');
  pill.classList.add('clear');
  await wait(600);
  zoom(1.9, { x: 1170, y: 40 });
  await wait(2300);
  await say(null);
  zoom(1, { x: 640, y: 360 });
  await wait(1300);
  endcard.classList.add('on');
  await wait(3200);
}

drawPill();
place(position);
Object.assign(window, { demo: { ready, partOne, partTwo } });
