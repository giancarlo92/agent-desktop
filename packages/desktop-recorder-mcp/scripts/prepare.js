'use strict';

// Puts the program and the skills into the package before packing it: the release build of this
// repository's agent-desktop (with record), and its skills folders.

const fs = require('fs');
const path = require('path');
const os = require('os');

const pkg = path.join(__dirname, '..');
const repo = path.join(pkg, '..', '..');
const target = process.env.CARGO_TARGET_DIR || path.join(os.homedir(), 'AppData', 'Local', 'agent-desktop-record', 'target-msvc');
const exe = path.join(target, 'release', 'agent-desktop.exe');

if (!fs.existsSync(exe)) {
  console.error(`No release build at ${exe}: run cargo build --release -p agent-desktop first`);
  process.exit(1);
}
fs.mkdirSync(path.join(pkg, 'vendor'), { recursive: true });
fs.copyFileSync(exe, path.join(pkg, 'vendor', 'agent-desktop.exe'));

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const a = path.join(from, entry.name);
    const b = path.join(to, entry.name);
    if (entry.isDirectory()) copyDir(a, b);
    else fs.copyFileSync(a, b);
  }
}
for (const skill of require('../lib/install').SKILLS) {
  fs.rmSync(path.join(pkg, 'skills', skill), { recursive: true, force: true });
  copyDir(path.join(repo, 'skills', skill), path.join(pkg, 'skills', skill));
}
fs.copyFileSync(path.join(repo, 'LICENSE'), path.join(pkg, 'LICENSE'));
fs.copyFileSync(path.join(repo, 'THIRD-PARTY-LICENSES.txt'), path.join(pkg, 'THIRD-PARTY-LICENSES.txt'));
console.log(`vendor/agent-desktop.exe and skills/ ready (from ${exe})`);
