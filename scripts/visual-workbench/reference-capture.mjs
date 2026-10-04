// reference-capture.mjs - the visual workbench's reference side (#91).
//
// Opens the hash-pinned authored reference in an isolated headless Chrome,
// sets every board's `platform` to Windows (never the default macOS
// glyphs), saves each board's glass panel at its own logical size, and
// drives the root board through every reference-backed scenario of the
// native fixture's registry - the same steps, through Chrome's real input
// events - capturing the panel and its DOM state at each capture step.
//
//   node scripts/visual-workbench/reference-capture.mjs \
//     --reference docs/evidence/ui-prototype/reference/launcher.html \
//     --sha256 F7E81E03... --registry <registry.json> --out <dir>
//
// Isolation: the reference is served from a loopback-only HTTP server on a
// random port (file:// frames are cross-origin and block the boards'
// scripts), Chrome runs headless with a fresh temporary profile this
// script creates, and on exit the script ends only the Chrome process tree
// it started and deletes only the profile directory it created.
//
// Coordinates: device scale factor 1, so one CSS pixel is one image pixel;
// every rect the manifest records is relative to the board's glass panel,
// which is the native fixture's client.

import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const args = Object.fromEntries(
  process.argv.slice(2).reduce((pairs, arg, index, all) => {
    if (arg.startsWith('--')) pairs.push([arg.slice(2), all[index + 1]]);
    return pairs;
  }, []),
);
for (const required of ['reference', 'sha256', 'registry', 'out']) {
  if (!args[required]) {
    console.error(`reference-capture: --${required} is required`);
    process.exit(2);
  }
}
const CHROME = args.chrome ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const out = resolve(args.out);
mkdirSync(out, { recursive: true });
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

// The reference must be the pinned bytes: a changed file is not the
// authority this workbench compares against.
const html = readFileSync(args.reference);
const sha256 = createHash('sha256').update(html).digest('hex').toUpperCase();
if (sha256 !== args.sha256.toUpperCase()) {
  console.error(`reference-capture: ${args.reference} is ${sha256}, expected ${args.sha256}`);
  process.exit(3);
}
const registry = JSON.parse(readFileSync(args.registry, 'utf8'));

// Board slugs by the board document's title, as the research catalogued
// them; Store and the snap HUD are saved as source-only references.
const BOARDS = [
  ['root', /launcher$/i],
  ['actions', /action panel/i],
  ['calculator', /calculator/i],
  ['empty', /no results/i],
  ['clipboard', /clipboard/i],
  ['window-manager', /window manager/i],
  ['store', /plugin store/i],
  ['settings', /settings/i],
  ['design-language', /design language/i],
];
const SOURCE_ONLY = new Set(['store', 'window-manager']);

const server = createServer((request, response) => {
  response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  response.end(html);
});
await new Promise((ready) => server.listen(0, '127.0.0.1', ready));
const pageUrl = `http://127.0.0.1:${server.address().port}/launcher.html`;

const profile = mkdtempSync(join(tmpdir(), 'pane-visual-workbench-chrome-'));
const debugPort = 9400 + Math.floor(Math.random() * 400);
// One tall viewport holds every board, so viewport and document
// coordinates agree and nothing scrolls between measuring and capturing.
const chrome = spawn(
  CHROME,
  [
    `--remote-debugging-port=${debugPort}`,
    `--user-data-dir=${profile}`,
    '--headless=new',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-extensions',
    '--force-device-scale-factor=1',
    '--hide-scrollbars',
    '--window-size=1600,9600',
    'about:blank',
  ],
  { stdio: 'ignore' },
);

const manifest = {
  kind: 'pane-visual-reference-manifest',
  reference: resolve(args.reference),
  sha256,
  chrome: null,
  chromeProfile: profile,
  deviceScaleFactor: 1,
  platform: 'Windows',
  startedUtc: new Date().toISOString(),
  boards: [],
  scenarios: [],
  fonts: null,
  cleanup: null,
};

let socket;
let nextId = 0;
const waiting = new Map();
const call = (method, params = {}) =>
  new Promise((done, fail) => {
    const id = ++nextId;
    waiting.set(id, (message) =>
      message.error ? fail(new Error(`${method}: ${message.error.message}`)) : done(message.result),
    );
    socket.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression) => {
  const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (result.exceptionDetails) {
    throw new Error(`page script failed: ${result.exceptionDetails.exception?.description ?? result.exceptionDetails.text}`);
  }
  return result.result.value;
};

// Page-side helpers, installed once per load: the board frames by slug,
// and the root board's DOM state relative to its glass panel.
const HELPERS = `
window.__wb = {
  frames() {
    return Array.from(document.querySelectorAll('iframe')).map((frame) => {
      const doc = frame.contentDocument;
      const r = frame.getBoundingClientRect();
      const glass = doc && doc.querySelector('section.glass');
      const g = glass && glass.getBoundingClientRect();
      return { title: doc ? doc.title : null, frame: [r.x, r.y, r.width, r.height],
               glass: g ? [g.x, g.y, g.width, g.height] : null,
               hasProps: !!(frame.contentWindow && frame.contentWindow.__dcSetProps) };
    });
  },
  frame(index) { return document.querySelectorAll('iframe')[index]; },
  // Board frames sit at fractional page offsets (headings above them);
  // shift each by its fraction so every board renders on whole pixels and
  // a capture never resamples it.
  snap() {
    for (const frame of document.querySelectorAll('iframe')) {
      frame.style.position = 'relative';
      frame.style.left = '0px';
      frame.style.top = '0px';
      const r = frame.getBoundingClientRect();
      frame.style.left = -(r.x - Math.floor(r.x)) + 'px';
      frame.style.top = -(r.y - Math.floor(r.y)) + 'px';
    }
    return Array.from(document.querySelectorAll('iframe')).every((f) => {
      const r = f.getBoundingClientRect();
      return Number.isInteger(r.x) && Number.isInteger(r.y);
    });
  },
  setWindows(index) {
    const w = this.frame(index).contentWindow;
    w.__dcSetProps(w.__dcRootName(), { platform: 'Windows', wallpaper: 'Graphite', accent: '#C9EE6A' });
  },
  rel(index, el) {
    const doc = this.frame(index).contentDocument;
    const g = doc.querySelector('section.glass').getBoundingClientRect();
    const r = el.getBoundingClientRect();
    return { x: r.x - g.x, y: r.y - g.y, width: r.width, height: r.height };
  },
  outer(index, el) {
    const f = this.frame(index).getBoundingClientRect();
    const r = el.getBoundingClientRect();
    return { x: f.x + r.x + r.width / 2, y: f.y + r.y + r.height / 2 };
  },
  rowFor(index, title) {
    const doc = this.frame(index).contentDocument;
    return Array.from(doc.querySelectorAll('button.row')).find((row) => row.querySelector('.row-t').textContent === title);
  },
  rows(index) {
    const doc = this.frame(index).contentDocument;
    const list = doc.querySelector('.list').getBoundingClientRect();
    return Array.from(doc.querySelectorAll('button.row')).map((row) => {
      const rect = this.rel(index, row);
      const r = row.getBoundingClientRect();
      const tile = row.querySelector('.tile');
      const title = row.querySelector('.row-t');
      const sub = row.querySelector('.row-s');
      return {
        title: title.textContent,
        subtitle: sub ? sub.textContent : null,
        selected: row.classList.contains('sel'),
        hovered: row.matches(':hover'),
        visible: r.top >= list.top && r.bottom <= list.bottom,
        rect,
        tile: tile ? this.rel(index, tile) : null,
        tileApp: tile ? tile.classList.contains('app') : null,
        titleRect: this.rel(index, title),
        background: getComputedStyle(row).backgroundColor,
        keys: Array.from(row.querySelectorAll('.keys .kbd')).map((k) => ({ label: k.textContent, rect: this.rel(index, k) })),
      };
    });
  },
  group(index, el) {
    if (!el) return null;
    const caps = el.classList.contains('kbd') ? [el] : Array.from(el.querySelectorAll('.kbd'));
    const style = getComputedStyle(caps[0]);
    return {
      rect: this.rel(index, el),
      caps: caps.map((k) => ({ label: k.textContent, rect: this.rel(index, k) })),
      font: { family: style.fontFamily, size: style.fontSize, weight: style.fontWeight },
      background: style.backgroundColor,
    };
  },
  keycapGroups(index) {
    const doc = this.frame(index).contentDocument;
    const buttons = doc.querySelectorAll('.fbtn');
    const rowKeys = (title) => { const row = this.rowFor(index, title); return row && row.querySelector('.keys'); };
    return {
      'footer-actions': this.group(index, buttons[1] && buttons[1].querySelector('.keys')),
      'left-half': this.group(index, rowKeys('Left Half')),
      'clipboard-history': this.group(index, rowKeys('Clipboard History')),
      'footer-primary': this.group(index, buttons[0] && buttons[0].querySelector('.kbd')),
      // The first pinned slot's compact corner hint (Ctrl 1).
      'pinned-1': this.group(index, doc.querySelector('.slot .slot-k')),
    };
  },
  state(index) {
    const doc = this.frame(index).contentDocument;
    const q = doc.querySelector('input.q');
    const buttons = doc.querySelectorAll('.fbtn');
    return {
      glass: this.rel(index, doc.querySelector('section.glass')),
      query: q.value,
      focused: doc.activeElement === q,
      searchHeader: this.rel(index, q.parentElement),
      list: this.rel(index, doc.querySelector('.list')),
      listScrollTop: doc.querySelector('.list').scrollTop,
      footerPrimary: buttons[0] ? { label: buttons[0].textContent.trim(), rect: this.rel(index, buttons[0]) } : null,
      // Every footer button, left to right (the primary action, then
      // Actions): the last one's right edge is the footer's right padding.
      footerButtons: Array.from(buttons).map((b) => ({ label: b.textContent.trim(), rect: this.rel(index, b) })),
      // The footer strip: the glass panel's child that holds the buttons.
      footer: (() => {
        let el = buttons[0];
        while (el && el.parentElement && !el.parentElement.matches('section.glass')) el = el.parentElement;
        return el ? this.rel(index, el) : null;
      })(),
      rows: this.rows(index),
      keycaps: this.keycapGroups(index),
    };
  },
  fonts() {
    const doc = this.frame(0).contentDocument;
    return Array.from(doc.fonts).map((f) => ({ family: f.family, weight: f.weight, style: f.style, status: f.status }));
  },
};
true`;

async function load() {
  await call('Page.navigate', { url: pageUrl });
  for (let attempt = 0; attempt < 120; attempt++) {
    await sleep(250);
    const ready = await evaluate(
      `Array.from(document.querySelectorAll('iframe')).length >= 9 && Array.from(document.querySelectorAll('iframe')).every((f) => f.contentWindow && f.contentWindow.__dcSetProps && f.contentDocument.readyState === 'complete')`,
    ).catch(() => false);
    if (ready) break;
  }
  await evaluate(HELPERS);
  if (!(await evaluate('__wb.snap()'))) throw new Error('the board frames could not be snapped to whole pixels');
  const frames = await evaluate('__wb.frames()');
  for (let index = 0; index < frames.length; index++) {
    if (frames[index].hasProps) await evaluate(`__wb.setWindows(${index})`);
  }
  await evaluate('document.fonts.ready.then(() => true)');
  await sleep(700);
  return evaluate('__wb.frames()');
}

async function screenshot(file, frame, glass) {
  const clip = { x: frame[0] + glass[0], y: frame[1] + glass[1], width: glass[2], height: glass[3], scale: 1 };
  const shot = await call('Page.captureScreenshot', { format: 'png', clip, captureBeyondViewport: false });
  const bytes = Buffer.from(shot.data, 'base64');
  writeFileSync(file, bytes);
  return { file, width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20), clip };
}

const KEYS = {
  down: { key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 },
  up: { key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 },
  escape: { key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 },
};

async function pointerTo(x, y) {
  await call('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none' });
}

async function runScenario(scenario, rootIndex, frames) {
  const dir = join(out, scenario.name);
  mkdirSync(dir, { recursive: true });
  // Rest: the query field focused by a real click, as the native fixture
  // opens with its field focused; the pointer is then over the header,
  // never over a row.
  const field = await evaluate(`__wb.outer(${rootIndex}, __wb.frame(${rootIndex}).contentDocument.querySelector('input.q'))`);
  await pointerTo(field.x, field.y);
  await call('Input.dispatchMouseEvent', { type: 'mousePressed', x: field.x, y: field.y, button: 'left', clickCount: 1 });
  await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x: field.x, y: field.y, button: 'left', clickCount: 1 });
  await sleep(300);
  const record = { name: scenario.name, captures: [], steps: scenario.steps };
  const after = [];
  for (const step of scenario.steps) {
    if (step.action === 'capture') {
      const state = await evaluate(`__wb.state(${rootIndex})`);
      const frame = frames[rootIndex].frame;
      const glass = (await evaluate('__wb.frames()'))[rootIndex].glass;
      const shot = await screenshot(join(dir, `${step.name}.png`), frame, glass);
      record.captures.push({ name: step.name, after: [...after], file: `${scenario.name}/${step.name}.png`, width: shot.width, height: shot.height, state });
      continue;
    }
    if (step.action === 'pointer') {
      // The fixture's row index names a shown row; the reference row with
      // the same title is the same item (both lists hold the same data).
      const titles = (await evaluate(`__wb.state(${rootIndex})`)).rows.map((row) => row.title);
      const title = titles[step.row];
      const point = await evaluate(`__wb.outer(${rootIndex}, __wb.rowFor(${rootIndex}, ${JSON.stringify(title)}))`);
      await pointerTo(point.x, point.y);
    } else if (step.action === 'key') {
      const key = KEYS[step.key];
      if (!key) throw new Error(`unknown key ${step.key}`);
      await call('Input.dispatchKeyEvent', { type: 'rawKeyDown', ...key });
      await call('Input.dispatchKeyEvent', { type: 'keyUp', ...key });
    } else if (step.action === 'type') {
      for (const character of step.text) {
        await call('Input.insertText', { text: character });
        await sleep(40);
      }
    }
    after.push(step);
    await sleep(350);
  }
  return record;
}

let failure = null;
try {
  let targets = [];
  for (let attempt = 0; attempt < 75 && !targets.some((t) => t.type === 'page'); attempt++) {
    await sleep(200);
    targets = await fetch(`http://127.0.0.1:${debugPort}/json/list`).then((r) => r.json()).catch(() => []);
  }
  const page = targets.find((target) => target.type === 'page');
  if (!page) throw new Error('Chrome exposed no page target');
  socket = new WebSocket(page.webSocketDebuggerUrl);
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    const done = message.id && waiting.get(message.id);
    if (done) {
      waiting.delete(message.id);
      done(message);
    }
  };
  await new Promise((open, fail) => {
    socket.onopen = open;
    socket.onerror = () => fail(new Error('could not connect to Chrome'));
  });
  manifest.chrome = (await call('Browser.getVersion')).product;
  await call('Page.enable');

  // Every board, at its own size, with Windows labels.
  const frames = await load();
  const rootIndex = frames.findIndex((frame) => BOARDS[0][1].test(frame.title ?? ''));
  if (rootIndex < 0) throw new Error(`the reference has no root board: ${JSON.stringify(frames)}`);
  mkdirSync(join(out, 'boards'), { recursive: true });
  for (const [index, frame] of frames.entries()) {
    const slug = (BOARDS.find(([, pattern]) => pattern.test(frame.title ?? '')) ?? [`board-${index}`])[0];
    const board = { slug, title: frame.title, frame: frame.frame, glass: frame.glass, sourceOnly: SOURCE_ONLY.has(slug), file: null };
    if (frame.glass) {
      const shot = await screenshot(join(out, 'boards', `${slug}.png`), frame.frame, frame.glass);
      board.file = `boards/${slug}.png`;
      board.image = [shot.width, shot.height];
    }
    manifest.boards.push(board);
  }
  manifest.fonts = await evaluate('__wb.fonts()');

  // Every reference-backed scenario from a fresh load, so no state leaks
  // from one scenario into the next.
  const only = args.scenario ? new Set(args.scenario.split(',')) : null;
  for (const scenario of registry.scenarios) {
    if (!scenario.reference || (only && !only.has(scenario.name))) continue;
    const fresh = await load();
    manifest.scenarios.push(await runScenario(scenario, rootIndex, fresh));
    console.log(`reference ${scenario.name}: ${manifest.scenarios.at(-1).captures.length} captures`);
  }
} catch (error) {
  failure = error;
  manifest.error = String(error.stack ?? error);
  console.error(`reference-capture failed: ${error.message}`);
} finally {
  try { socket?.close(); } catch {}
  server.close();
  // End only the Chrome tree this script started, then delete only the
  // profile it created.
  let killed = false;
  try {
    execFileSync('taskkill', ['/PID', String(chrome.pid), '/T', '/F'], { stdio: 'ignore' });
    killed = true;
  } catch {
    killed = chrome.exitCode !== null;
  }
  await sleep(500);
  let profileRemoved = false;
  for (let attempt = 0; attempt < 10 && !profileRemoved; attempt++) {
    try {
      rmSync(profile, { recursive: true, force: true });
      profileRemoved = true;
    } catch {
      await sleep(300);
    }
  }
  manifest.cleanup = { chromePid: chrome.pid, chromeEnded: killed, profileRemoved, server: 'closed' };
  manifest.finishedUtc = new Date().toISOString();
  writeFileSync(join(out, 'reference-manifest.json'), JSON.stringify(manifest, null, 2));
}
process.exit(failure ? 1 : 0);
