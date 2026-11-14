# agent-desktop-record

One command to give your AI agents a desktop they can see, use and **record**: Claude Code, Codex, Gemini CLI, Cursor, VS Code, Windsurf and Claude Desktop.

```bash
npx agent-desktop-record
```

It lists the agents on your computer; tick the ones you want and press Enter. For each one it adds the MCP server and, where the agent reads skills from a folder (Claude Code, Codex, Windsurf), the guides on how to use it, record a task and turn a recording into a script. The other agents read the same guides through the MCP server.

Then open a new conversation and ask, in your own words:

- "What displays do I have?"
- "Record the Excel window with video and save it in D:\Videos\task.mp4", then "I'm done".
- "Turn that recording into a script; the customer and the amount change every time."

While it records, a small red indicator at the bottom of the screen shows the time and a Stop button.

## Other commands

```bash
npx agent-desktop-record status                     # what is installed, and where
npx agent-desktop-record --agents claude-code,codex # set up these agents without asking
npx agent-desktop-record --dry-run                  # say what would change
npx agent-desktop-record uninstall                  # remove it from the agents you pick
```

The program goes in `%USERPROFILE%\.agent-desktop-record\bin` and its private state (refs, recordings) in `%USERPROFILE%\.agent-desktop-record\state`. Every configuration file it changes keeps a backup next to it (`.bak-agent-desktop-record`). Windows only.

## What this is

A fork of [vercel-labs/agent-desktop](https://github.com/vercel-labs/agent-desktop) (Apache-2.0), not the official package. It adds `record` (a person's own actions to JSON Lines, with an MP4 video, `record start`/`stop`/`status` in the background and an on-screen indicator), `stroke` (replaying a drawn path), and the guides `recording-to-script` and `script-repair`. For the official tool, use `npm install -g agent-desktop`.
