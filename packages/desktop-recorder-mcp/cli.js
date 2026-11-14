#!/usr/bin/env node
'use strict';

// npx desktop-recorder-mcp            install: pick your agents from a list
// npx desktop-recorder-mcp status     what is installed, and where
// npx desktop-recorder-mcp uninstall  remove it from your agents

const os = require('os');
const { agents } = require('./lib/agents');
const { install, uninstall, status, layout } = require('./lib/install');
const { multiselect, confirm } = require('./lib/prompt');

const HELP = `desktop-recorder-mcp: agent-desktop with record (events, video, an on-screen indicator) for your AI agents

Usage:
  npx desktop-recorder-mcp [install] [options]   install and set up the agents you pick
  npx desktop-recorder-mcp status                 what is installed, and where
  npx desktop-recorder-mcp uninstall [options]   remove it from the agents you pick

Options:
  --agents a,b   the agents to set up, without asking: ${'claude-code, codex, gemini, claude-desktop, cursor, vscode, windsurf'}
  --all          every agent found on this computer, without asking
  --dry-run      say what would change, change nothing
  --home DIR     act on another home folder (for tests)
  --remove-program   with uninstall: also delete the program (recordings are kept)
  -h, --help     this help`;

function parse(argv) {
  const options = { command: 'install', agents: null, all: false, dryRun: false, home: null, removeProgram: false };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === 'install' || arg === 'status' || arg === 'uninstall') options.command = arg;
    else if (arg === '--agents') options.agents = (argv[++i] || '').split(',').map((s) => s.trim()).filter(Boolean);
    else if (arg === '--all') options.all = true;
    else if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--home') options.home = argv[++i];
    else if (arg === '--remove-program') options.removeProgram = true;
    else if (arg === '-h' || arg === '--help') options.command = 'help';
    else throw new Error(`Unknown option ${arg}; see --help`);
  }
  return options;
}

async function main() {
  const options = parse(process.argv.slice(2));
  if (options.command === 'help') return console.log(HELP);
  if (process.platform !== 'win32' && !options.home) {
    throw new Error('desktop-recorder-mcp runs on Windows (record and its video are Windows only)');
  }
  const home = options.home || os.homedir();
  const run = { home, env: process.env, dryRun: options.dryRun, useCli: !options.home, removeProgram: options.removeProgram };
  if (options.command === 'status') return status(run).forEach((line) => console.log(line));

  const known = agents(home, process.env);
  const found = known.filter((a) => a.detect());
  let chosen = options.agents;
  if (chosen) {
    const ids = known.map((a) => a.id);
    const unknown = chosen.filter((id) => !ids.includes(id));
    if (unknown.length) throw new Error(`Unknown agent ${unknown.join(', ')}; use ${ids.join(', ')}`);
  } else if (options.all) {
    chosen = found.map((a) => a.id);
  } else if (process.stdin.isTTY) {
    const verb = options.command === 'uninstall' ? 'remove it from' : 'set it up in';
    chosen = await multiselect(`Which agents do you want to ${verb}?`, known.map((a) => ({
      label: a.name,
      value: a.id,
      checked: options.command === 'install' && a.detect(),
      note: a.detect() ? '' : '(not found)',
    })));
  } else {
    throw new Error('No terminal to ask in: pass --agents claude-code,codex or --all');
  }
  if (!chosen.length) return console.log('Nothing chosen, nothing changed.');

  if (options.command === 'uninstall') {
    return uninstall(chosen, run).forEach((line) => console.log(line));
  }
  if (!options.dryRun && process.stdin.isTTY && !options.agents && !options.all) {
    const where = layout(home);
    const ok = await confirm(`Install the program in ${where.bin} and set up ${chosen.length} agent(s)?`, true);
    if (!ok) return console.log('Nothing changed.');
  }
  install(chosen, { ...run, log: console.log }).forEach((line) => console.log(line));
  if (!options.dryRun) {
    console.log('\nOpen a new conversation in each agent and ask it: "What displays do I have?"');
    console.log('To record: "Record the Excel window with video and save it in D:\\Videos\\task.mp4".');
  }
}

main().catch((error) => {
  console.error(`desktop-recorder-mcp: ${error.message}`);
  process.exitCode = 1;
});
