'use strict';

// The agents the installer knows: how to tell one is installed, where its MCP servers are configured,
// and where it reads skills from. An agent with no skills folder still gets the guides through the MCP
// server itself (agent-desktop serves them as the skills tools).

const path = require('path');
const fs = require('fs');

function onPath(command, env) {
  const dirs = (env.PATH || env.Path || '').split(path.delimiter).filter(Boolean);
  const exts = (env.PATHEXT || '.EXE;.CMD;.BAT;.PS1').split(';').map((e) => e.toLowerCase()).concat(['']);
  return dirs.some((dir) => exts.some((ext) => fs.existsSync(path.join(dir, command + ext))));
}

function claudeDesktopConfig(home, env) {
  const local = env.LOCALAPPDATA || path.join(home, 'AppData', 'Local');
  const packages = path.join(local, 'Packages');
  let packaged = null;
  try {
    const found = fs.readdirSync(packages).find((name) => /^Claude_/i.test(name));
    if (found) packaged = path.join(packages, found, 'LocalCache', 'Roaming', 'Claude');
  } catch (_) {
    /* no packaged install */
  }
  const roaming = path.join(env.APPDATA || path.join(home, 'AppData', 'Roaming'), 'Claude');
  const dir = packaged && fs.existsSync(packaged) ? packaged : roaming;
  return path.join(dir, 'claude_desktop_config.json');
}

/** Every agent, described for one home folder and environment. */
function agents(home, env) {
  const appData = env.APPDATA || path.join(home, 'AppData', 'Roaming');
  return [
    {
      id: 'claude-code',
      name: 'Claude Code',
      cli: 'claude',
      detect: () => fs.existsSync(path.join(home, '.claude')) || onPath('claude', env),
      mcp: { kind: 'json', file: path.join(home, '.claude.json'), key: 'mcpServers', type: 'stdio' },
      skillsDir: path.join(home, '.claude', 'skills'),
    },
    {
      id: 'codex',
      name: 'Codex (ChatGPT)',
      cli: 'codex',
      detect: () => fs.existsSync(path.join(home, '.codex')) || onPath('codex', env),
      mcp: { kind: 'toml', file: path.join(home, '.codex', 'config.toml') },
      skillsDir: path.join(home, '.codex', 'skills'),
    },
    {
      id: 'gemini',
      name: 'Gemini CLI',
      detect: () => fs.existsSync(path.join(home, '.gemini')) || onPath('gemini', env),
      mcp: { kind: 'json', file: path.join(home, '.gemini', 'settings.json'), key: 'mcpServers' },
      skillsDir: null,
    },
    {
      id: 'claude-desktop',
      name: 'Claude Desktop',
      detect: () => fs.existsSync(path.dirname(claudeDesktopConfig(home, env))),
      mcp: { kind: 'json', file: claudeDesktopConfig(home, env), key: 'mcpServers' },
      skillsDir: null,
    },
    {
      id: 'cursor',
      name: 'Cursor',
      detect: () => fs.existsSync(path.join(home, '.cursor')) || onPath('cursor', env),
      mcp: { kind: 'json', file: path.join(home, '.cursor', 'mcp.json'), key: 'mcpServers' },
      skillsDir: null,
    },
    {
      id: 'vscode',
      name: 'VS Code (Copilot)',
      detect: () => fs.existsSync(path.join(appData, 'Code', 'User')) || onPath('code', env),
      mcp: { kind: 'json', file: path.join(appData, 'Code', 'User', 'mcp.json'), key: 'servers', type: 'stdio' },
      skillsDir: null,
    },
    {
      id: 'windsurf',
      name: 'Windsurf',
      detect: () => fs.existsSync(path.join(home, '.codeium', 'windsurf')) || onPath('windsurf', env),
      mcp: { kind: 'json', file: path.join(home, '.codeium', 'windsurf', 'mcp_config.json'), key: 'mcpServers' },
      skillsDir: path.join(home, '.codeium', 'windsurf', 'skills'),
    },
  ];
}

module.exports = { agents, onPath };
