// desktop-script.mjs: the helpers a script made from a recording needs, in one file with no
// dependencies. Copy it next to the script; the script imports it with
//   import { task } from './desktop-script.mjs';
// It runs agent-desktop (from desktop-recorder-mcp) for every action, so a script works in the
// background: it never uses the mouse or the keyboard and never brings a window to the front.
// Node 18 or later. Part of the desktop-recorder-mcp fork of agent-desktop (Apache-2.0).

import { execFile, execFileSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { createInterface } from 'node:readline/promises';

const exe = process.platform === 'win32' ? 'agent-desktop.exe' : 'agent-desktop';

/** An agent-desktop answer with ok: false, or a step that could not be done. */
export class ScriptError extends Error {
  constructor(message, details = {}) {
    super(message);
    this.name = 'ScriptError';
    Object.assign(this, details);
  }
}

let found = null;

/**
 * The agent-desktop program: $AGENT_DESKTOP_BIN, else the one desktop-recorder-mcp installed, else
 * one on PATH. It must be a background-only build: an older agent-desktop would use the mouse and
 * bring windows forward, so it is refused.
 */
export function agentDesktop() {
  if (found) return found;
  const installed = join(homedir(), '.desktop-recorder-mcp', 'bin', exe);
  const candidates = [process.env.AGENT_DESKTOP_BIN, existsSync(installed) ? installed : null, 'agent-desktop'];
  for (const candidate of candidates.filter(Boolean)) {
    let help = '';
    try {
      help = execFileSync(candidate, ['launch', '--help'], { encoding: 'utf8', windowsHide: true });
    } catch (_) {
      continue;
    }
    if (!/always works in the background/.test(help)) {
      throw new ScriptError(`${candidate} is not the background-only agent-desktop: run "npx desktop-recorder-mcp" or set AGENT_DESKTOP_BIN`);
    }
    found = candidate;
    return found;
  }
  throw new ScriptError('agent-desktop was not found: run "npx desktop-recorder-mcp" or set AGENT_DESKTOP_BIN');
}

function environment() {
  const env = { ...process.env };
  const state = join(homedir(), '.desktop-recorder-mcp', 'state');
  if (!env.AGENT_DESKTOP_HOME && existsSync(state)) env.AGENT_DESKTOP_HOME = state;
  return env;
}

/** Runs one agent-desktop command and returns its data; an answer with ok: false throws. */
export function run(...args) {
  return new Promise((resolve, reject) => {
    execFile(agentDesktop(), args.map(String), { encoding: 'utf8', windowsHide: true, env: environment(), maxBuffer: 64 << 20 },
      (failure, stdout) => {
        let answer;
        try {
          answer = JSON.parse(stdout);
        } catch (_) {
          reject(new ScriptError(`${args[0]}: no answer${failure ? ` (${failure.message.split('\n')[0]})` : ''}`));
          return;
        }
        if (answer.ok) resolve(answer.data);
        else {
          const { code, message, disposition, details } = answer.error || {};
          const options = details && Array.isArray(details.options) ? ` (its options: ${details.options.join(', ')})` : '';
          reject(new ScriptError(`${args[0]}: ${code} ${message}${options}`, { code, disposition, details }));
        }
      });
  });
}

const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

/** Calls `attempt` until it returns something, or throws `failure()` once the time is up. */
async function until(timeoutMs, attempt, failure) {
  const end = Date.now() + timeoutMs;
  let last;
  for (;;) {
    try {
      const result = await attempt();
      if (result !== undefined && result !== null && result !== false) return result;
    } catch (error) {
      // The screen may still be changing; a refusal that waiting cannot change ends it now.
      if (['POLICY_DENIED', 'INVALID_ARGS', 'INTERNAL', 'PERMISSION_DENIED'].includes(error.code)) throw error;
      last = error;
    }
    if (Date.now() >= end) throw new ScriptError(failure() + (last ? ` (${last.message})` : ''));
    await sleep(400);
  }
}

function describe(query) {
  if (query.nativeId) return `element with id "${query.nativeId}"`;
  return `${query.role || 'element'}${query.name ? ` "${query.name}"` : ''}${query.text ? ` with "${query.text}"` : ''}`;
}

/** One element, found again right before each use (refs go stale when the screen changes). */
export class Element {
  constructor(window, query, ref) {
    this.window = window;
    this.query = query;
    this.ref = ref;
  }

  toString() {
    return describe(this.query);
  }

  /** Clicks a button, link, tab or menu item. */
  async click() {
    return run('click', this.ref);
  }

  /** Writes a value and checks the field kept it. */
  async setValue(value, { timeoutMs = 5000 } = {}) {
    const done = await run('set-value', this.ref, String(value));
    // agent-desktop already read the value back (from the page itself for a web field: a date field
    // shows its parts, not the whole value, to the accessibility tree).
    if (done && done.disposition && done.disposition.delivery === 'delivered_verified') return;
    await until(timeoutMs, async () => (await this.value()) === String(value),
      () => `${this} does not hold what was written`);
  }

  async clear() {
    return run('clear', this.ref);
  }

  /** Chooses an option in a combo box or list, and checks it was chosen. */
  async select(option) {
    return run('select', this.ref, String(option));
  }

  async check() {
    await run('check', this.ref);
    if (!(await this.is('checked'))) throw new ScriptError(`${this} is not checked`);
  }

  async uncheck() {
    await run('uncheck', this.ref);
    if (await this.is('checked')) throw new ScriptError(`${this} is still checked`);
  }

  /**
   * Writes a saved password into this field, by name. The first time, when nothing is saved under
   * that name, a small window asks the person for it once; it is never seen by the script.
   */
  async fillSecret(name) {
    const done = await run('secret', 'fill', this.ref, name);
    return { askedNow: Boolean(done && done.saved_now) };
  }

  async value() {
    return (await run('get', this.ref, '--property', 'value')).value ?? '';
  }

  async text() {
    const data = await run('get', this.ref, '--property', 'text');
    return data.text ?? data.value ?? '';
  }

  /** visible, enabled, checked, focused, expanded or selected. */
  async is(state) {
    const data = await run('is', this.ref, '--property', state);
    return Boolean(data.result ?? data.value ?? data[state]);
  }
}

/** A window of the task, found by its title (and app). */
export class Window {
  constructor(info) {
    Object.assign(this, info);
  }

  toString() {
    return `"${this.title}"`;
  }

  #findArgs(query) {
    const args = ['find', '--window-id', this.id, '--first'];
    if (this.app_name) args.push('--app', this.app_name);
    if (query.nativeId) args.push('--native-id', query.nativeId);
    if (query.role) args.push('--role', query.role);
    if (query.name) args.push('--name', query.name);
    if (query.text) args.push('--text', query.text);
    if (query.exact) args.push('--exact');
    return args;
  }

  /**
   * An element of this window, waiting until it is there. `query` takes nativeId (the automation
   * id, the most stable), or role and name, or text; exact: true for an exact name.
   */
  async find(query, { timeoutMs = 15000 } = {}) {
    const ref = await until(timeoutMs, async () => {
      const match = (await run(...this.#findArgs(query))).match;
      return match && match.ref_id;
    }, () => `No ${describe(query)} in ${this}`);
    return new Element(this, query, ref);
  }

  /**
   * The text of an element that is only read, such as a message or a total (a text you cannot act on
   * has no ref, so `find` would not return it): its value, else its name. Waits until it is there.
   */
  async read(query, { timeoutMs = 15000 } = {}) {
    const match = await until(timeoutMs, async () => {
      const found = (await run(...this.#findArgs(query))).match;
      return found && (found.value || found.name) ? found : null;
    }, () => `No ${describe(query)} in ${this}`);
    return match.value || match.name;
  }

  /** Whether an element is there now, without waiting long: for a dialog that only sometimes appears. */
  async has(query, { timeoutMs = 1500 } = {}) {
    try {
      return await this.find(query, { timeoutMs });
    } catch (_) {
      return null;
    }
  }

  /** Waits until this window shows a text. */
  async waitForText(text, { timeoutMs = 15000 } = {}) {
    const args = ['wait', '--text', text, '--window-id', this.id, '--timeout', timeoutMs];
    if (this.app_name) args.push('--app', this.app_name);
    try {
      await run(...args);
    } catch (error) {
      throw new ScriptError(`${this} did not show "${text}" (${error.message})`);
    }
  }

  async close() {
    return run('close-window', this.id).catch(() => null);
  }
}

/** A window whose title contains `title` (and of `app`, or of the process `pid`), waiting until it is there. */
export async function findWindow(title, { app, pid, timeoutMs = 30000 } = {}) {
  const info = await until(timeoutMs, async () => {
    const windows = await run('list-windows', ...(app ? ['--app', app] : []));
    return windows.find((w) => (!title || (w.title || '').includes(title)) && (!pid || w.pid === pid));
  }, () => `No window titled "${title}"${app ? ` of ${app}` : ''}`);
  return new Window(info);
}

/**
 * Opens an app with `launch` (it never takes the focus) and returns its window. `title` is a part
 * of the window's title, as the recording noted it.
 */
export async function openApp(app, { title, args = [], timeoutMs = 30000 } = {}) {
  const launched = await run('launch', app, ...args.map((a) => `--arg=${a}`), '--timeout', timeoutMs);
  return findWindow(title, { pid: title ? undefined : launched && launched.pid, timeoutMs });
}

function chromePath() {
  const candidates = process.platform === 'win32'
    ? [process.env.PROGRAMFILES, process.env['PROGRAMFILES(X86)'], process.env.LOCALAPPDATA]
      .filter(Boolean).map((root) => join(root, 'Google', 'Chrome', 'Application', 'chrome.exe'))
      .concat([process.env['PROGRAMFILES(X86)'], process.env.PROGRAMFILES].filter(Boolean)
        .map((root) => join(root, 'Microsoft', 'Edge', 'Application', 'msedge.exe')))
    : process.platform === 'darwin' ? ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'] : ['/usr/bin/google-chrome'];
  const found = candidates.find((path) => existsSync(path));
  if (!found) throw new ScriptError('Chrome (or Edge) was not found: pass { browser: "<path to the browser>" }');
  return found;
}

const PID_FILE = 'desktop-script.pid';
const opened = [];

/** Whether `pid` is still a process of the program `image` (a pid can be reused by another program). */
function isRunning(pid, image) {
  try {
    if (process.platform === 'win32') {
      const out = execFileSync('tasklist', ['/FI', `PID eq ${pid}`, '/FO', 'CSV', '/NH'], { encoding: 'utf8', windowsHide: true });
      return out.toLowerCase().includes(`"${image.toLowerCase()}"`);
    }
    return execFileSync('ps', ['-p', String(pid), '-o', 'comm='], { encoding: 'utf8' }).includes(image);
  } catch (_) {
    return false;
  }
}

/** Ends a browser the script started (only that one: the person's own browser is left alone). */
function closeBrowser({ pid, image }) {
  if (!isRunning(pid, image)) return;
  try {
    if (process.platform === 'win32') execFileSync('taskkill', ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true });
    else process.kill(pid, 'SIGTERM');
  } catch (_) {
    /* it ended on its own */
  }
}

/**
 * Opens a web page in a browser of the script's own (a separate profile, so the person's own
 * browser is left alone) with DevTools, so its page can be clicked and written in the background.
 * `title` is a part of the page's title. The browser is closed when the task finishes; one left
 * open by an earlier run (a failed run, or one stopped with --stop-before) is closed first.
 */
export async function openBrowser(url, { title, profile, browser, timeoutMs = 30000 } = {}) {
  const dir = profile || join(homedir(), '.desktop-recorder-mcp', 'script-browser');
  const exePath = browser || chromePath();
  const image = exePath.split(/[\\/]/).pop();
  const pidFile = join(dir, PID_FILE);
  try {
    closeBrowser({ pid: Number(readFileSync(pidFile, 'utf8')), image });
    await sleep(500);
  } catch (_) {
    /* no earlier run */
  }
  const launched = await run('launch', exePath, '--no-attach', '--cdp', `--arg=--user-data-dir=${dir}`, '--arg=--no-first-run',
    '--arg=--no-default-browser-check', '--arg=--force-renderer-accessibility', '--arg=--new-window', `--arg=${url}`,
    '--timeout', timeoutMs);
  const own = { pid: launched.pid, image, pidFile };
  opened.push(own);
  writeFileSync(pidFile, String(launched.pid));
  return findWindow(title, { app: process.platform === 'win32' ? image : undefined, pid: launched.pid, timeoutMs });
}

function closeOpened() {
  for (const own of opened.splice(0)) {
    closeBrowser(own);
    rmSync(own.pidFile, { force: true });
  }
}

function parseArgs(argv) {
  const values = {};
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg.startsWith('--')) throw new ScriptError(`Unexpected "${arg}": every value goes after its --name`);
    const [rawName, inline] = arg.slice(2).split(/=(.*)/s);
    const name = rawName.replace(/-/g, '_');
    if (inline !== undefined) values[name] = inline;
    else if (argv[i + 1] !== undefined && !argv[i + 1].startsWith('--')) values[name] = argv[++i];
    else values[name] = true;
  }
  return values;
}

function usage(title, params) {
  const lines = [title, '', 'Values:'];
  for (const [name, spec] of Object.entries(params)) {
    const extra = spec.default !== undefined ? ` (default: ${spec.default})` : spec.example !== undefined ? ` (e.g. ${spec.example})` : '';
    lines.push(`  --${name.replace(/_/g, '-')}  ${spec.ask}${spec.options ? ` [${spec.options.join(', ')}]` : ''}${extra}`);
  }
  lines.push('', 'Options:', '  --yes             do the steps that cannot be undone without asking',
    '  --stop-before N   stop before step N and leave the screen there (to repair a step)',
    '  --keep-open       leave the browser the script opened when it finishes', '  --help            this help');
  return lines.join('\n');
}

/**
 * Runs a task: reads its values from the command line (--name value), runs the steps and prints
 * where it stopped when one fails. The exit code is 0 when it finished, 1 when a step failed and 2
 * when a value is missing or wrong.
 *
 *   await task({ title, params: { full_name: { ask: 'The person's full name', example: 'Lucia' } } },
 *     async ({ params, step, confirm }) => { ... });
 */
export async function task({ title = 'Task', params: spec = {} }, body) {
  let given;
  try {
    given = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exit(2);
  }
  if (given.help) {
    console.log(usage(title, spec));
    return;
  }
  const params = {};
  const missing = [];
  for (const [name, item] of Object.entries(spec)) {
    const value = given[name] ?? item.default;
    if (value === undefined || value === true) missing.push(`--${name.replace(/_/g, '-')}: ${item.ask}${item.example !== undefined ? ` (e.g. ${item.example})` : ''}`);
    else if (item.options && !item.options.map(String).includes(String(value))) missing.push(`--${name}: one of ${item.options.join(', ')}`);
    else params[name] = String(value);
  }
  if (missing.length) {
    console.error(`${title}: these values are missing or wrong:\n  ${missing.join('\n  ')}`);
    process.exit(2);
  }
  const stopBefore = Number(given.stop_before || 0);
  const autoConfirm = given.yes === true || given.yes === 'true';
  let current = null;

  class Stop extends Error {}
  const step = async (number, what, action) => {
    if (stopBefore > 0 && number >= stopBefore) {
      console.log(`Stopped before step ${number} (${what}).`);
      throw new Stop();
    }
    current = { number, what };
    console.log(`Step ${number}. ${what}`);
    return action();
  };
  // Asks before a step that cannot be undone (send, post, delete, buy, pay); --yes answers for the person.
  const confirm = async (question) => {
    if (autoConfirm) return;
    if (!process.stdin.isTTY) throw new ScriptError(`"${question}" needs an answer: run it in a terminal, or with --yes`);
    const reader = createInterface({ input: process.stdin, output: process.stdout });
    const answer = (await reader.question(`${question} (y/n) `)).trim().toLowerCase();
    reader.close();
    if (!['y', 'yes', 's', 'si', 'sí'].includes(answer)) throw new ScriptError('Not confirmed: nothing was done in this step');
  };

  try {
    const result = await body({ params, step, confirm, run, findWindow, openApp, openBrowser });
    // A finished task closes the browser it opened; --keep-open leaves it to look at.
    if (!given.keep_open) closeOpened();
    console.log(result ? `Done: ${result}` : 'Done.');
  } catch (error) {
    // A failed or stopped run leaves the screen as it is, to see where it stopped (and repair it).
    if (error instanceof Stop) return;
    const where = current ? `Step ${current.number} (${current.what}) failed` : 'The task failed before its first step';
    console.error(`${where}: ${error.message}`);
    process.exitCode = 1;
  }
}
