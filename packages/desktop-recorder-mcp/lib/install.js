'use strict';

// Installing desktop-recorder-mcp: the program in one folder of its own, then for each chosen agent the
// MCP server and, where the agent reads skills from a folder, the skills.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const config = require('./config');
const { agents, onPath } = require('./agents');

const SERVER = 'desktop-recorder';
const PACKAGE = path.join(__dirname, '..');
const SKILLS = ['agent-desktop', 'agent-desktop-windows'];

/** Where everything goes for one home folder. */
function layout(home) {
  const root = path.join(home, '.desktop-recorder-mcp');
  return {
    root,
    bin: path.join(root, 'bin'),
    exe: path.join(root, 'bin', 'agent-desktop.exe'),
    // The tool's private state (refs, snapshots, recordings), apart from the upstream tool's ~/.agent-desktop.
    state: path.join(root, 'state'),
  };
}

function server(where) {
  return { command: where.exe, args: ['mcp'], env: { AGENT_DESKTOP_HOME: where.state } };
}

function installProgram(where, log) {
  const source = path.join(PACKAGE, 'vendor', 'agent-desktop.exe');
  if (!fs.existsSync(source)) throw new Error(`The package has no program (${source}); run npm run prepare-vendor`);
  fs.mkdirSync(where.bin, { recursive: true });
  fs.mkdirSync(where.state, { recursive: true });
  try {
    fs.copyFileSync(source, where.exe);
  } catch (error) {
    if (error.code === 'EBUSY' || error.code === 'EPERM') {
      throw new Error(`${where.exe} is in use: close your agents (or stop recordings) and run the installer again`);
    }
    throw error;
  }
  log(`Program: ${where.exe}`);
}

function version(exe) {
  try {
    const out = execFileSync(exe, ['version'], { encoding: 'utf8', windowsHide: true });
    return JSON.parse(out).data.version;
  } catch (_) {
    return null;
  }
}

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const a = path.join(from, entry.name);
    const b = path.join(to, entry.name);
    if (entry.isDirectory()) copyDir(a, b);
    else fs.copyFileSync(a, b);
  }
}

function installSkills(agent) {
  if (!agent.skillsDir) return [];
  const installed = [];
  for (const skill of SKILLS) {
    const from = path.join(PACKAGE, 'skills', skill);
    if (!fs.existsSync(from)) continue;
    const to = path.join(agent.skillsDir, skill);
    fs.rmSync(to, { recursive: true, force: true });
    copyDir(from, to);
    installed.push(skill);
  }
  return installed;
}

function removeSkills(agent) {
  if (!agent.skillsDir) return [];
  return SKILLS.filter((skill) => {
    const dir = path.join(agent.skillsDir, skill);
    if (!fs.existsSync(path.join(dir, '.desktop-recorder-mcp'))) return false;
    fs.rmSync(dir, { recursive: true, force: true });
    return true;
  });
}

// Claude Code and Codex rewrite their own configuration while they run: their CLIs change it safely.
function addServer(agent, where, options) {
  const spec = server(where);
  if (options.useCli && agent.id === 'claude-code' && onPath('claude', options.env)) {
    try { execFileSync('claude', ['mcp', 'remove', SERVER, '--scope', 'user'], { stdio: 'ignore', shell: true }); } catch (_) { /* absent */ }
    const json = JSON.stringify({ type: 'stdio', command: spec.command, args: spec.args, env: spec.env });
    execFileSync('claude', ['mcp', 'add-json', SERVER, quote(json), '--scope', 'user'], { stdio: 'ignore', shell: true });
    return;
  }
  if (options.useCli && agent.id === 'codex' && onPath('codex', options.env)) {
    try { execFileSync('codex', ['mcp', 'remove', SERVER], { stdio: 'ignore', shell: true }); } catch (_) { /* absent */ }
    const env = Object.entries(spec.env).flatMap(([k, v]) => ['--env', quote(`${k}=${v}`)]);
    execFileSync('codex', ['mcp', 'add', SERVER, ...env, '--', quote(spec.command), ...spec.args], { stdio: 'ignore', shell: true });
    return;
  }
  config.add(agent.mcp, SERVER, spec);
}

function removeServer(agent, options) {
  if (options.useCli && agent.id === 'claude-code' && onPath('claude', options.env)) {
    try { execFileSync('claude', ['mcp', 'remove', SERVER, '--scope', 'user'], { stdio: 'ignore', shell: true }); return true; } catch (_) { return false; }
  }
  if (options.useCli && agent.id === 'codex' && onPath('codex', options.env)) {
    try { execFileSync('codex', ['mcp', 'remove', SERVER], { stdio: 'ignore', shell: true }); return true; } catch (_) { return false; }
  }
  return config.remove(agent.mcp, SERVER);
}

function quote(text) {
  return `"${String(text).replace(/"/g, '\\"')}"`;
}

/** Installs the program and sets up each chosen agent. Returns one line of result per agent. */
function install(chosen, options) {
  const where = layout(options.home);
  const log = options.log || console.log;
  if (!options.dryRun) installProgram(where, log);
  const results = [];
  for (const agent of agents(options.home, options.env).filter((a) => chosen.includes(a.id))) {
    if (options.dryRun) {
      results.push(`${agent.name}: would add the MCP server to ${agent.mcp.file}` +
        (agent.skillsDir ? ` and the skills to ${agent.skillsDir}` : ''));
      continue;
    }
    try {
      addServer(agent, where, options);
      const skills = installSkills(agent);
      for (const skill of skills) fs.writeFileSync(path.join(agent.skillsDir, skill, '.desktop-recorder-mcp'), '');
      results.push(`${agent.name}: MCP server added` + (skills.length ? `, skills in ${agent.skillsDir}` :
        ' (it reads the guides through the MCP server)'));
    } catch (error) {
      results.push(`${agent.name}: not set up: ${error.message}`);
    }
  }
  return results;
}

function uninstall(chosen, options) {
  const results = [];
  for (const agent of agents(options.home, options.env).filter((a) => chosen.includes(a.id))) {
    try {
      const removed = removeServer(agent, options);
      const skills = removeSkills(agent);
      results.push(`${agent.name}: ${removed ? 'MCP server removed' : 'no MCP server'}` +
        (skills.length ? `, skills removed` : ''));
    } catch (error) {
      results.push(`${agent.name}: ${error.message}`);
    }
  }
  if (options.removeProgram) {
    fs.rmSync(layout(options.home).bin, { recursive: true, force: true });
    results.push('Program removed (recordings kept in ' + layout(options.home).state + ')');
  }
  return results;
}

function status(options) {
  const where = layout(options.home);
  const lines = [`Program: ${fs.existsSync(where.exe) ? `${where.exe} (${version(where.exe) || 'unknown version'})` : 'not installed'}`];
  for (const agent of agents(options.home, options.env)) {
    const found = agent.detect();
    const mcp = config.has(agent.mcp, SERVER);
    const skills = agent.skillsDir && fs.existsSync(path.join(agent.skillsDir, 'agent-desktop', '.desktop-recorder-mcp'));
    lines.push(`${agent.name}: ${found ? 'found' : 'not found'}` +
      (found ? `, MCP ${mcp ? 'set up' : 'not set up'}` + (agent.skillsDir ? `, skills ${skills ? 'installed' : 'not installed'}` : '') : ''));
  }
  return lines;
}

module.exports = { install, uninstall, status, layout, server, SERVER };
