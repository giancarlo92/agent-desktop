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
  assert.ok(lines.some((l) => /^Codex \(ChatGPT\): found, MCP not set up/.test(l)), lines.join('\n'));
});
