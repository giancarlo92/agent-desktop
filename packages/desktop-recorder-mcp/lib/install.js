'use strict';

// Installing desktop-recorder-mcp: the program in one folder of its own, then for each chosen agent the
// MCP server and, where the agent reads skills from a folder, the skills.

const fs = require('fs');
const path = require('path');
const { execFileSync, execSync } = require('child_process');

const config = require('./config');
const { agents, onPath } = require('./agents');

const SERVER = 'desktop-recorder';
const PACKAGE = path.join(__dirname, '..');
const SKILLS = ['agent-desktop', 'agent-desktop-windows', 'desktop-recording', 'recording-to-script', 'script-repair'];

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
  makePrivate(where.state);
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

/**
 * agent-desktop keeps its state (snapshots, refs) only in a folder no one else can open, and refuses
 * one it finds open to others: it never widens or repairs one it did not make. Other tools may grant
 * themselves access to folders under the person's home (the Codex sandbox gives its own group
 * access to ~/.desktop-recorder-mcp), and a folder made here would inherit that. So the state folder
 * stops inheriting and is opened only to the person, SYSTEM and Administrators, the same three
 * agent-desktop sets on the folders it makes; what is already inside goes back to inheriting from it.
 */
function makePrivate(dir) {
  if (process.platform !== 'win32') {
    fs.chmodSync(dir, 0o700);
    return;
  }
  // Windows' own programs by full path: another whoami (Git's, for one) may come first on PATH.
  const system32 = path.join(process.env.SystemRoot || process.env.windir || 'C:\\Windows', 'System32');
  const user = execFileSync(path.join(system32, 'whoami.exe'), ['/user', '/fo', 'csv', '/nh'], { encoding: 'utf8', windowsHide: true })
    .trim().split(',').pop().replace(/"/g, '');
  if (!/^S-1-5-[\d-]+$/.test(user)) throw new Error(`could not read your account's id to protect ${dir}`);
  const icacls = (...args) => execFileSync(path.join(system32, 'icacls.exe'), args, { stdio: 'ignore', windowsHide: true });
  icacls(dir, '/inheritance:r', '/grant:r', `*${user}:(OI)(CI)F`, '*S-1-5-18:(OI)(CI)F', '*S-1-5-32-544:(OI)(CI)F');
  if (fs.readdirSync(dir).length) icacls(path.join(dir, '*'), '/reset', '/T', '/C', '/Q');
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
// On Windows those CLIs are npm's .cmd scripts, which only the command shell can run, so each call is
// one command line built here, every part quoted (see commandLine). A value that cannot be passed that
// way falls back to editing the configuration file, as for the other agents.
function cliArgs(agent, spec) {
  const env = Object.entries(spec.env).flatMap(([key, value]) => ['--env', `${key}=${value}`]);
  if (agent.id === 'claude-code') {
    return {
      remove: ['claude', 'mcp', 'remove', SERVER, '--scope', 'user'],
      add: ['claude', 'mcp', 'add', SERVER, '--scope', 'user', ...env, '--', spec.command, ...spec.args],
    };
  }
  return {
    remove: ['codex', 'mcp', 'remove', SERVER],
    add: ['codex', 'mcp', 'add', SERVER, ...env, '--', spec.command, ...spec.args],
  };
}

function usesCli(agent, options) {
  return options.useCli && agent.cli && ['claude-code', 'codex'].includes(agent.id) && onPath(agent.cli, options.env);
}

function addServer(agent, where, options) {
  const spec = server(where);
  if (usesCli(agent, options)) {
    const calls = cliArgs(agent, spec);
    const add = commandLine(calls.add);
    if (add) {
      try { run(commandLine(calls.remove), options); } catch (_) { /* not there yet */ }
      run(add, options);
      return;
    }
  }
  config.add(agent.mcp, SERVER, spec);
}

function removeServer(agent, options) {
  if (usesCli(agent, options)) {
    try { run(commandLine(cliArgs(agent, server(layout(options.home))).remove), options); return true; } catch (_) { return false; }
  }
  return config.remove(agent.mcp, SERVER);
}

/**
 * One command line for the command shell, or null when a part cannot be passed through it safely.
 * Plain parts go as they are; the rest in double quotes, inside which the shell takes &, |, <, >, (, )
 * and spaces literally. A part with ", %, ! or ^ is refused: the shell would read those even in quotes.
 */
function commandLine(parts) {
  const quoted = [];
  for (const part of parts) {
    const text = String(part);
    if (/^[A-Za-z0-9_.:\\/=@+,-]+$/.test(text)) quoted.push(text);
    else if (/["%!^\r\n]/.test(text) || text.endsWith('\\')) return null;
    else quoted.push(`"${text}"`);
  }
  return quoted.join(' ');
}

function run(line, options) {
  execSync(line, { stdio: 'ignore', windowsHide: true, env: options.env });
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

module.exports = { commandLine, install, uninstall, status, layout, server, SERVER, SKILLS };
