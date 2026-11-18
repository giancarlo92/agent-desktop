'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');

const { install, uninstall, status, layout, SERVER } = require('../lib/install');
const config = require('../lib/config');

function fakeHome() {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'adr-home-'));
  const env = { APPDATA: path.join(home, 'AppData', 'Roaming'), LOCALAPPDATA: path.join(home, 'AppData', 'Local'), PATH: '' };
  for (const dir of ['.claude', '.codex', '.cursor', path.join('AppData', 'Roaming', 'Code', 'User')]) {
    fs.mkdirSync(path.join(home, dir), { recursive: true });
  }
  fs.writeFileSync(path.join(home, '.claude.json'), JSON.stringify({ theme: 'dark', mcpServers: { other: { command: 'x' } } }));
  fs.writeFileSync(path.join(home, '.codex', 'config.toml'), 'model = "gpt"\n\n[mcp_servers.other]\ncommand = "x"\n');
  return { home, env, options: { home, env, useCli: false, log: () => {} } };
}

const read = (file) => fs.readFileSync(file, 'utf8');

test('install sets up each chosen agent and keeps what was there', () => {
  const { home, options } = fakeHome();
  const lines = install(['claude-code', 'codex', 'cursor', 'vscode'], options);
  const where = layout(home);
  assert.ok(fs.existsSync(where.exe), 'the program is copied');

  const claude = JSON.parse(read(path.join(home, '.claude.json')));
  assert.equal(claude.theme, 'dark');
  assert.ok(claude.mcpServers.other, 'another server is kept');
  assert.equal(claude.mcpServers[SERVER].command, where.exe);
  assert.deepEqual(claude.mcpServers[SERVER].args, ['mcp']);
  assert.equal(claude.mcpServers[SERVER].env.AGENT_DESKTOP_HOME, where.state);
  assert.ok(fs.existsSync(path.join(home, '.claude.json' + config.BACKUP_SUFFIX)), 'a backup is kept');

  const toml = read(path.join(home, '.codex', 'config.toml'));
  assert.match(toml, /model = "gpt"/);
  assert.match(toml, /\[mcp_servers\.other\]/);
  assert.match(toml, new RegExp(`\\[mcp_servers\\.${SERVER}\\]`));
  assert.ok(toml.includes(`command = '${where.exe}'`), 'a Windows path is a literal TOML string');

  const vscode = JSON.parse(read(path.join(home, 'AppData', 'Roaming', 'Code', 'User', 'mcp.json')));
  assert.equal(vscode.servers[SERVER].type, 'stdio');
  assert.ok(JSON.parse(read(path.join(home, '.cursor', 'mcp.json'))).mcpServers[SERVER]);

  assert.ok(fs.existsSync(path.join(home, '.claude', 'skills', 'agent-desktop', 'SKILL.md')), 'Claude Code gets the skills');
  assert.ok(fs.existsSync(path.join(home, '.claude', 'skills', 'agent-desktop', 'references', 'recording-to-script.md')));
  assert.ok(fs.existsSync(path.join(home, '.codex', 'skills', 'agent-desktop-windows', 'SKILL.md')), 'Codex gets the skills');
  assert.equal(lines.length, 4);
  assert.ok(lines.every((line) => !line.includes('not set up')), lines.join('\n'));
});

test('installing twice leaves one server, and uninstall removes only ours', () => {
  const { home, options } = fakeHome();
  install(['claude-code', 'codex'], options);
  install(['claude-code', 'codex'], options);
  const toml = read(path.join(home, '.codex', 'config.toml'));
  assert.equal(toml.split(`[mcp_servers.${SERVER}]`).length - 1, 1, 'one block');

  uninstall(['claude-code', 'codex'], options);
  const claude = JSON.parse(read(path.join(home, '.claude.json')));
  assert.ok(!claude.mcpServers[SERVER]);
  assert.ok(claude.mcpServers.other);
  const after = read(path.join(home, '.codex', 'config.toml'));
  assert.ok(!after.includes(SERVER), after);
  assert.match(after, /\[mcp_servers\.other\]/);
  assert.ok(!fs.existsSync(path.join(home, '.claude', 'skills', 'agent-desktop')), 'our skills are removed');
});

test('a skill folder the installer did not write is never removed', () => {
  const { home, options } = fakeHome();
  const own = path.join(home, '.claude', 'skills', 'agent-desktop');
  fs.mkdirSync(own, { recursive: true });
  fs.writeFileSync(path.join(own, 'SKILL.md'), 'the person installed the official skill');
  uninstall(['claude-code'], options);
  assert.ok(fs.existsSync(path.join(own, 'SKILL.md')));
});

test('a JSON file with comments is reported and left as it was', () => {
  const { home, options } = fakeHome();
  const file = path.join(home, '.cursor', 'mcp.json');
  fs.writeFileSync(file, '{\n  // mine\n  "mcpServers": {}\n}\n');
  const [line] = install(['cursor'], options);
  assert.match(line, /not set up/);
  assert.equal(read(file), '{\n  // mine\n  "mcpServers": {}\n}\n');
});

test('a dry run changes nothing', () => {
  const { home, options } = fakeHome();
  const before = read(path.join(home, '.claude.json'));
  const lines = install(['claude-code'], { ...options, dryRun: true });
  assert.match(lines[0], /would add/);
  assert.equal(read(path.join(home, '.claude.json')), before);
  assert.ok(!fs.existsSync(layout(home).exe));
});

test('status says what is set up', () => {
  const { options } = fakeHome();
  install(['claude-code'], options);
  const lines = status(options);
  assert.ok(lines.some((l) => /^Claude Code: found, MCP set up, skills installed/.test(l)), lines.join('\n'));
  assert.ok(lines.some((l) => /^Codex \(the codex command and the Codex app\): found, MCP not set up/.test(l)), lines.join('\n'));
});

test('a command line quotes what needs it and refuses what the shell would still read', () => {
  const { commandLine } = require('../lib/install');
  assert.strictEqual(
    commandLine(['claude', 'mcp', 'add', 'x', '--env', 'K=C:\\Users\\Ana\\.tool', '--', 'C:\\Program Files (x86)\\a & b\\t.exe', 'mcp']),
    'claude mcp add x --env K=C:\\Users\\Ana\\.tool -- "C:\\Program Files (x86)\\a & b\\t.exe" mcp'
  );
  for (const unsafe of ['a"b', '%PATH%', 'wow!', 'x^y', 'line\nbreak', 'C:\\dir with space\\']) {
    assert.strictEqual(commandLine(['claude', unsafe]), null, unsafe);
  }
});

test('the agents CLIs get every argument intact, through npm .cmd scripts, without a Node warning', { skip: process.platform !== 'win32' }, () => {
  const { spawnSync } = require('child_process');
  const { home, env } = fakeHome();
  const bin = path.join(home, 'npm-bin');
  fs.mkdirSync(bin);
  const log = path.join(home, 'calls.jsonl');
  fs.writeFileSync(path.join(bin, 'record.js'), `require('fs').appendFileSync(${JSON.stringify(log)}, JSON.stringify(process.argv.slice(2)) + '\\n');`);
  for (const cli of ['claude', 'codex']) {
    // The shape of a script npm installs for a global command.
    fs.writeFileSync(path.join(bin, `${cli}.cmd`), `@ECHO off\r\n"${process.execPath}" "%~dp0\\record.js" ${cli} %*\r\n`);
  }
  const homeWithSpace = path.join(home, 'Ana & Co (work)');
  fs.mkdirSync(homeWithSpace);
  const script = `
    const { install } = require(${JSON.stringify(path.join(__dirname, '..', 'lib', 'install'))});
    install(['claude-code', 'codex'], { home: ${JSON.stringify(homeWithSpace)}, env: process.env, useCli: true, log: () => {} });`;
  const childEnv = { ...process.env, ...env, PATH: `${bin};${process.env.PATH}` };
  const child = spawnSync(process.execPath, ['-e', script], { env: childEnv, encoding: 'utf8' });
  assert.strictEqual(child.status, 0, child.stderr);
  const stderr = child.stderr;
  const calls = read(log).trim().split('\n').map((line) => JSON.parse(line));
  const where = layout(homeWithSpace);
  assert.deepStrictEqual(calls.find((c) => c[0] === 'claude' && c[2] === 'add'),
    ['claude', 'mcp', 'add', SERVER, '--scope', 'user', '--env', `AGENT_DESKTOP_HOME=${where.state}`, '--', where.exe, 'mcp']);
  assert.deepStrictEqual(calls.find((c) => c[0] === 'codex' && c[2] === 'add'),
    ['codex', 'mcp', 'add', SERVER, '--env', `AGENT_DESKTOP_HOME=${where.state}`, '--', where.exe, 'mcp']);
  assert.ok(!/DEP0190|DeprecationWarning/.test(stderr || ''), stderr);
});

test('Claude Desktop is set up where recent versions read it (Claude-Data), else where older ones did', () => {
  const { agents } = require('../lib/agents');
  const { home, env } = fakeHome();
  const desktop = () => agents(home, env).find((a) => a.id === 'claude-desktop');
  const old = path.join(env.APPDATA, 'Claude');
  fs.mkdirSync(old, { recursive: true });
  assert.strictEqual(desktop().mcp.file, path.join(old, 'claude_desktop_config.json'));
  const current = path.join(env.LOCALAPPDATA, 'Claude-Data', 'claude_pzs8sxrjxfjjc');
  fs.mkdirSync(current, { recursive: true });
  assert.strictEqual(desktop().mcp.file, path.join(current, 'claude_desktop_config.json'));
  assert.ok(desktop().detect());
});
