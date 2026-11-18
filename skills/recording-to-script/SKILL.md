---
name: recording-to-script
tags: automation-script, rpa, record-and-replay, desktop-automation, ai-agent
requirements:
  - agent-desktop
description: >
  Turn a recording of a person's task (agent-desktop record) into a script that repeats it
  without AI, in the background, with the values that change as parameters and saved passwords
  filled by name. JavaScript by default; any language the person asks for (Python, Go,
  PowerShell...). Use when the person wants to automate a task they recorded or showed you,
  make it repeatable, turn steps into a script, or run it again with other data.
  Triggers on: "turn the recording into a script", "convierte la grabación en script",
  "automate this task", "make a script from what I did", "repeat this every day",
  "the customer and the amount change every time", "write it in Python".
  When such a script later fails, use the script-repair skill.
---
<!-- Part of the desktop-recorder-mcp fork of agent-desktop (see NOTICE). -->

# From a recording to a script

A person recorded a task (see the `desktop-recording` skill) and wants it to run again without doing it by hand. You write a script that repeats it with agent-desktop commands and no AI. The script opens what the task needs, finds each element by what it is (not by where it was on the screen), does the same actions and checks each one worked. It stops with a clear message when something is not as expected. It runs in the background: the person keeps using their computer meanwhile.

The script replays the task as it was recorded. When the app or page changes, it stops at the step that no longer matches: then follow the `script-repair` skill.

## 1. Read the recording

The recording is a JSON Lines file: `recording_started`, `recording_ready`, one line per action, `recording_stopped`. Read every action in order and first write it down in plain words, one line each. Show this list to the person: it is what they can correct before any code exists.

```
1. Chrome, page northwind.example/signup: type "Lucia Fernandez" in "Full name"
2. same page: choose "Peru" in "Country"
3. click the button "Create account"
```

What matters in each action line:

- `kind`: `click`, `type`, `select`, `press`, `scroll`, `drag`.
- `element`: `role`, `name`, `native_id` (the automation id, or a web element's `id`), `help_text` (often a placeholder), `label` (the text beside a field with no name), `bounds`.
- `ancestors` and `match_count`: above 1, several elements look the same and the nearest named ancestor tells them apart.
- `window`: `title`, `app_name`, `path` (the program), `bounds`, and `url` for a web page.
- `value` on `type` and `select`: what the field ended up holding. `secure: true` is a password: no text was recorded.
- `state_after.checked` on a click: the box ended checked or not.
- `video.offset_ms` in the last line: an action at `t_ms` is at `t_ms - offset_ms` in the video. Watch the video around an action you do not understand.

## 2. Keep what the task needs

Leave out what a replay must not repeat:

- Clicks that only moved the focus or did nothing, and repeated attempts at the same thing (keep the last one that worked).
- Mistakes the person corrected: a value typed and then changed keeps only the last value; a menu or dialog opened and closed without choosing anything is left out.
- Clicks on the taskbar or the Start menu (`explorer.exe`): the script opens the app itself.
- Minimize, maximize and restore.

Keep everything that changes the result: every value, every option, every file opened or saved, every confirmation. A part that has no direct background command is still done, another way (section 6); never drop it. What the person says about the task (a step they forgot, a value that changes, something to ignore) wins over the recording.

## 3. Decide the parameters

A value that will be different next time is a parameter: a name, an email, an amount, a date, a file, a search term. Ask the person when it is not clear. Give each one an English `snake_case` name, a short question that explains it and the recorded value as its example. A choice among fixed options lists them.

A password (`secure: true`) is never written in the script, never a parameter and never invented. The script fills it by name with `agent-desktop secret fill <field> <name>`, with an English name that says what it is for (`bank-login`). The first run opens a small window where the person types it once; it is saved in the system's secret store, bound to that app and site, and later runs fill it on their own. Whoever runs a shared script is asked for their own password the first time.

## 4. Choose the language

**JavaScript is the default**: Node is already on the computer (the tool was installed with `npx`), it runs the same on Windows, macOS and Linux, and `scripts/desktop-script.mjs` in this skill already does the rules below. Use it unless the person asks for something else.

**If the person asks for another language** (Python, Go, PowerShell, C#, TypeScript...), write the script in that language, with the structure that suits it. The rules in the next section are what make a script work; they hold in any language. Section 8 shows the few helpers to write.

## 5. The rules every script keeps

These were tested on real forms, a password prompt and a desktop app, with a person typing and clicking meanwhile.

1. **Run agent-desktop and read its JSON.** Every command prints one envelope: `{"ok": true, "data": ...}` or `{"ok": false, "error": {"code", "message", "details", "disposition"}}`. A step fails when `ok` is false. Call the program directly with an argument list, never through a shell string (values with spaces, quotes or `&` stay intact).
2. **Use the installed program.** `$AGENT_DESKTOP_BIN` if set, else `~/.desktop-recorder-mcp/bin/agent-desktop(.exe)`, else `agent-desktop` on PATH. Check that `agent-desktop launch --help` says it "always works in the background": an older agent-desktop would move the mouse and bring windows forward.
3. **Open, never attach to, what the person is using.** A desktop app: `launch <program>` (it never takes the focus), then wait for its window by title. A web page: `launch <browser> --no-attach --cdp --arg=--user-data-dir=<a folder of the script's own> --arg=--no-first-run --arg=<url>`. The DevTools endpoint lets clicks, values, list choices and checkboxes on the page happen without the browser ever coming forward, and the separate profile leaves the person's own browser alone. Find its window by the `pid` that `launch` returned.
4. **Find each element right before acting on it**, inside the task's window (`find --window-id <id> --first ...`). Refs belong to one snapshot and go stale. Prefer, in this order: `--native-id`, then `--role` with `--name` (`--exact` for short names such as "AI"), then `--name` with the `help_text` or `label`, then an ancestor when `match_count` was above 1. Retry `find` for a while (15 s) instead of sleeping a fixed time: pages and apps answer at different speeds.
5. **A text that is only read has no ref.** A message, a total or a status is read from the `find` match itself (`match.value`, else `match.name`), not with `get`.
6. **Act with background commands** (table in section 6): `click`, `set-value`, `clear`, `select`, `check`, `uncheck`, `secret fill`. This build refuses `press`, `type`, `focus-window`, `hover`, `drag` and mouse commands, because they would use the person's keyboard and mouse. When a step has no background command, the script still does it another way: the app's own automation (COM for Excel, Word or Outlook), its command line, its files or database, or an API.
7. **Check every step.** `set-value`, `select`, `check` and `secret fill` report `data.disposition.delivery`. `delivered_verified` means agent-desktop read the result back (for a web field, from the page itself: a date field shows its parts, not the value, to accessibility). Otherwise read it with `get <ref> --property value` or `is <ref> --property checked`. After an action that opens or changes something, wait for what the next step needs (`wait --text <text> --window-id <id>`, or a retried `find`).
8. **Retry an action only when `disposition.retry` is `safe`.** Any other answer means it may have happened: read the screen and decide, never repeat it blindly.
9. **Ask before what cannot be undone** (send, post, delete, buy, pay), unless the person asked for it to run on its own (a `--yes` option). Never retry those steps, and never try two ways of doing one.
10. **Say where it failed.** Number the steps and give each a short description. On failure print `Step N (what) failed: <why>` and exit with a non-zero code. A `select` with no such option lists the options there are (`error.details.options`): print them. Leave the screen as it is: it is where a repair starts.
11. **Let a repair stop before a step**: an option such as `--stop-before N` that runs the steps before N and leaves the screen there.
12. **Clean up only what it opened.** On success, close the browser it started (by its `pid`, never all browsers). Before opening its browser, close the one an earlier run left open with the same profile.

## 6. Turn each action into a command

| Recorded | In the script |
|---|---|
| `click` on a button, link, tab, menu item, radio button | `click` the element |
| `click` with `state_after.checked` | `check` or `uncheck` (they do nothing when it is already so) |
| `type` into a field | `set-value` with the value (or the parameter), then check it |
| `type` with `secure: true` | `secret fill` the field with the secret's name |
| `type` into a grid cell or canvas editor | select the cell (`select` or `click`), then `set-value` the cell or its editor; a grid that only accepts typing is written through the app's automation (COM for Excel) or its file |
| `select` | `select` the list with the option's text (or value) |
| `press` (`return`, `ctrl+s`, `tab`) | `click` the button or menu item that does the same (Save, OK, Send, Submit, Execute); with none, do it through the app's automation or command line |
| `scroll` | usually nothing: `find` reaches elements out of view; `scroll-to` if one must be visible |
| `drag` that selected cells or text | select the same range with `select` |
| `drag` on a canvas, or a click with no name at all | find the element in a fresh `snapshot` (it often has a name or id there), or reach the same result through the app's automation, its files or a command |
| copy (`ctrl+c`) with `clipboard` | read the source (`get`, or the `find` match) and `set-value` it where it goes |

## 7. JavaScript (the default)

Copy `scripts/desktop-script.mjs` from this skill next to the script. An agent without the skill folder gets it with `agent-desktop skills get recording-to-script desktop-script.mjs` (MCP: `desktop_skills_get` with `{"name": "recording-to-script", "reference": "desktop-script.mjs"}`) and saves the `content`. It has no dependencies and needs Node 18 or later.

```js
// create-account.mjs: node create-account.mjs --full-name "Lucia Fernandez" --country Peru
import { task } from './desktop-script.mjs';

await task({
  title: 'Create an account on Northwind Cloud',
  params: {
    full_name: { ask: "The person's first and last name", example: 'Lucia Fernandez' },
    country: { ask: 'Their country', options: ['Spain', 'Peru', 'Mexico'], example: 'Peru' },
    plan: { ask: 'The plan', options: ['Basic', 'Pro'], default: 'Pro' },
  },
}, async ({ params, step, openBrowser, confirm }) => {
  let page;
  await step(1, 'Open the sign-up page', async () => {
    page = await openBrowser('https://northwind.example/signup', { title: 'Create your account' });
  });
  await step(2, 'Type the full name', async () => {
    await (await page.find({ nativeId: 'fullName' })).setValue(params.full_name);
  });
  await step(3, 'Choose the country', async () => {
    await (await page.find({ nativeId: 'country' })).select(params.country);
  });
  await step(4, 'Choose the plan', async () => {
    await (await page.find({ role: 'radiobutton', name: params.plan, exact: true })).click();
  });
  await step(5, 'Accept the terms', async () => {
    await (await page.find({ nativeId: 'terms' })).check();
  });
  await step(6, 'Create the account', async () => {
    await confirm(`Create the account for ${params.full_name}?`);
    await (await page.find({ role: 'button', name: 'Create account' })).click();
    await page.waitForText('Account created');
  });
  return page.read({ name: 'Reference' });
});
```

What `desktop-script.mjs` gives:

| | |
|---|---|
| `task({ title, params }, body)` | Reads `--name value` (snake_case names also as `--kebab-case`), prints `--help` with each question, refuses missing values (exit 2), runs the body, prints `Step N (what) failed: why` (exit 1) or `Done` |
| `step(n, what, fn)` | One numbered step; `--stop-before N` stops there and leaves the screen |
| `confirm(question)` | Asks y/n in the terminal; `--yes` answers for the person |
| `openApp(program, { title, args })` | `launch`, then its window |
| `openBrowser(url, { title, profile, browser })` | Chrome (or Edge) with a profile of the script's own and DevTools; closed when the task finishes (`--keep-open` keeps it) |
| `findWindow(title, { app, pid })` | A window already open |
| `window.find(query)` | An element to act on, waiting for it. `query`: `nativeId`, `role`, `name`, `text`, `exact` |
| `window.read(query)` | The text of an element that is only read |
| `window.has(query)` | The element if it is there now, else `null`: for a dialog that only sometimes appears |
| `window.waitForText(text)` | Waits until the window shows it |
| `element.click()`, `.setValue(v)`, `.clear()`, `.select(option)`, `.check()`, `.uncheck()`, `.fillSecret(name)`, `.value()`, `.is(state)` | The actions, each checked |
| `run(...args)` | Any other agent-desktop command; returns `data` or throws |

The person runs it with `node create-account.mjs --full-name "..." --country Peru`, and `node create-account.mjs --help` lists what it asks for.

## 8. Another language

Write the same few helpers the library has, in that language, and keep the rules of section 5. For example, in Python:

```python
import json, os, subprocess, sys, time
from pathlib import Path

AD = os.environ.get("AGENT_DESKTOP_BIN") or str(Path.home() / ".desktop-recorder-mcp" / "bin" / "agent-desktop.exe")

def run(*args):
    answer = json.loads(subprocess.run([AD, *map(str, args)], capture_output=True, text=True).stdout)
    if not answer["ok"]:
        error = answer["error"]
        raise RuntimeError(f"{args[0]}: {error['code']} {error['message']}")
    return answer["data"]

def find(window, timeout=15, **query):            # find(page, native_id="email") or role=, name=
    flags = [f"--{k.replace('_', '-')}" if v is True else f"--{k.replace('_', '-')}={v}" for k, v in query.items()]
    end = time.time() + timeout
    while time.time() < end:
        match = run("find", "--window-id", window["id"], "--first", *flags)["match"]
        if match and match.get("ref_id"):
            return match["ref_id"]
        time.sleep(0.4)
    raise RuntimeError(f"No {query} in {window['title']}")

def step(number, what, action):
    print(f"Step {number}. {what}")
    try:
        action()
    except Exception as error:
        sys.exit(f"Step {number} ({what}) failed: {error}")
```

Then each step reads like the plain-words list: `step(2, "Type the full name", lambda: run("set-value", find(page, native_id="fullName"), full_name))`. Add the parameters (`argparse`), the confirmation, `--stop-before`, and the browser opening and closing as in section 5.

## 9. Try it before handing it over

Run it once from a clean state (the app closed, the page not open), with the recorded values, and with `--yes` only if it has no irreversible step or you are on a test page. It must reach its final check. If a step fails, fix that step (see `script-repair`) and run it again from the start until it passes end to end. Do not run real public actions (send, post, pay) on a real site to test it: ask the person, or stop before that step.

Then give the person:

- the script and how to run it (with an example command),
- its parameters with their questions and examples (`--help`),
- the saved passwords it uses, by name, and that the first run will ask for them,
- what it does not cover, and why.
