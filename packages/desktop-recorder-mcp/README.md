# desktop-recorder-mcp

One command to give your AI agents a desktop they can see, use and **record**: Claude Code, Codex, Gemini CLI, Cursor, VS Code, Windsurf and Claude Desktop.

```bash
npx desktop-recorder-mcp
```

It lists the agents on your computer; tick the ones you want and press Enter. For each one it adds the MCP server and, where the agent reads skills from a folder (Claude Code, Codex, Windsurf), the guides on how to use it, record a task and turn a recording into a script. The other agents read the same guides through the MCP server.

Some agents come in two forms that share one setup, and some do not:

| You use | Tick | Then |
|---|---|---|
| Claude Code in a terminal, or the **Code** tab of the Claude app | Claude Code | Start a new session |
| The **Chat** of the Claude app | Claude Desktop (a setup of its own) | Quit and reopen the app |
| The `codex` command, or the **Codex** app | Codex (both read `~/.codex/config.toml`) | Restart the Codex app |

The chat of the ChatGPT app cannot run a tool on your computer this way: it connects only to MCP servers on the internet.

Then open a new conversation and ask, in your own words:

- "What displays do I have?"
- "Record the Excel window with video and save it in D:\Videos\task.mp4", then "I'm done".
- "Turn that recording into a script; the customer and the amount change every time."

While it records, a small red indicator at the bottom of the screen shows the time and a Stop button.

## Passwords

The tool never records a password or writes one into a script. A script or an agent asks for a password by name (`secret fill <field> bank-login`). The first time, a small window asks you to type it once; it is saved in your system's own secret store (Windows Credential Manager, the macOS Keychain, the Secret Service on Linux), bound to that app and site, and the task goes on. From then on it is filled on its own. Agents never see it, and it is refused anywhere but a password field of that app and site.

To save one ahead of time, or change it:

```bash
agent-desktop secret set bank-login --app chrome.exe --site login.mybank.com
agent-desktop secret delete bank-login
```

## Other commands

```bash
npx desktop-recorder-mcp status                     # what is installed, and where
npx desktop-recorder-mcp --agents claude-code,codex # set up these agents without asking
npx desktop-recorder-mcp --dry-run                  # say what would change
npx desktop-recorder-mcp uninstall                  # remove it from the agents you pick
```

The program goes in `%USERPROFILE%\.desktop-recorder-mcp\bin` and its private state (refs, recordings) in `%USERPROFILE%\.desktop-recorder-mcp\state`. Every configuration file it changes keeps a backup next to it (`.bak-desktop-recorder-mcp`).

**Windows for now. macOS and Linux are coming soon.**

## Responsible use

This tool can watch the mouse and keyboard and film the screen. It is built to be seen and to stay on your computer:

- It records only when you (or an agent you asked) start it, and a red indicator with a **Stop** button stays on screen the whole time.
- A recording stops by itself after 10 minutes (at most one hour). Nothing starts with Windows, and nothing records in the background without being asked.
- Passwords are never written to the recording. A video shows whatever is on the recorded window or display: record one window, not a whole screen.
- Recordings stay in the files you choose. Nothing is uploaded, and the tool has no telemetry.
- The source is open: [the fork](https://github.com/giancarlo92/agent-desktop). Every file it changes in your agents keeps a backup.

Record only your own work, or with the consent of the people whose screen or data appear in it, and follow the privacy rules that apply to that data. Some sites forbid automation in their terms: check them before automating a site you do not own. The tool does not solve CAPTCHAs or get around bot detection.

## What this is

A fork of [vercel-labs/agent-desktop](https://github.com/vercel-labs/agent-desktop) (Apache-2.0), not the official package. It adds `record` (a person's own actions to JSON Lines, with an MP4 video, `record start`/`stop`/`status` in the background and an on-screen indicator), `secret` (passwords asked for once and kept in the system's secret store), and the guides `recording-to-script` and `script-repair`. It works only in the background: it never uses your mouse or keyboard and never brings a window to the front, so you can keep working while an agent or a script runs (commands that would need that are refused). Windows for now; macOS and Linux are coming soon. For the official tool, use `npm install -g agent-desktop`.
