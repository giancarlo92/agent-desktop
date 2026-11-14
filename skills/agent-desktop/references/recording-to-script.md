# From a recording to a script

Use this when a person recorded a task (`record`, `record start`/`record stop`) and wants to run it again without doing it by hand. The result is a script that replays the task with agent-desktop commands and no AI: it opens what the task needs, finds each element by what it is (not by where it was on the screen), does the same actions, checks each one worked, and stops with a clear message when something is not as expected.

A script replays the task as it was recorded. It does not adapt when the app or page changes; when that happens, repair it (`agent-desktop skills get desktop script-repair`).

## 1. Read the recording

The recording is a JSON Lines file (`out`): `recording_started`, `recording_ready`, one line per action, `recording_stopped`. Read every action in order and write it down in plain words first, one line each, before writing any code. For example:

```
1. Chrome, page demoqa.com/text-box: click the field "Full Name", type "Lucia Fernandez"
2. same page: type "lucia@example.com" in "Email"
3. click the button "Submit"
```

What each action line tells you is in `commands-system` (section `record`). The parts that matter here:

- `kind`: `click`, `type`, `select`, `press`, `scroll`, `drag`.
- `element`: `role`, `name`, `native_id` (the automation id), `help_text` (often a placeholder), `label` (the text beside a field with no name), `bounds`.
- `ancestors` and `match_count`: when `match_count` is above 1, several elements look the same and the nearest named ancestor tells them apart.
- `window`: `title`, `app_name`, `path` (the app's executable), `bounds`, and `url` for a browser page.
- `value` on `type` and `select`: what the field ended up holding. `secure: true` means a password: the text was not recorded.
- `state_after.checked` on a click: the checkbox or radio ended checked or not.
- `clipboard` on a copy: how many lines and columns it copied (the text is never recorded).
- `path` on a `drag`: every point of the stroke, for a canvas.
- `video.offset_ms` in the last line: an action at `t_ms` is at `t_ms - offset_ms` in the video. Watch the video around an action you do not understand.

## 2. Keep what the task needs

A person's recording carries things a replay must not repeat. Leave out:

- Clicks that only moved the focus or did nothing visible, and repeated attempts at the same thing (keep the last one that worked).
- Mistakes the person corrected: a value typed and then undone (`ctrl+z`) or typed again in the same field keeps only the last value; a menu or dialog opened and closed again (`escape`, Cancel) without choosing anything is left out.
- Clicks on the taskbar or the Start menu (`explorer.exe`): they only opened or switched to an app. The script opens the app itself.
- Minimize, maximize and restore.

Keep everything that changes the result: every value typed, every option chosen, every file opened or saved, every confirmation. The script must achieve what the recording achieved. If a part cannot be replayed (an element with no name and no stable place), tell the person which part and why instead of leaving it out silently.

If the person said something about the task (a step they forgot, a value that changes every time, something to ignore), that wins over the recording.

## 3. Decide the parameters

A value that will be different next time is a parameter, not a fixed value: a name, an email, an amount, a date, a file name, a search term. Ask the person when it is not clear. Give each parameter an English `snake_case` name, a short question that explains it, and the recorded value as its example.

- A password (`secure: true`) is never written in the script and never invented. Use a secret by name: the script runs `secret fill <field ref> <name>`, with an English name that says what it is for (`bank-login`). The first run opens a small window where the person types the password once; it is saved bound to that app and site, and every later run fills it on its own (see `commands-system`, section `secret`). A password box often has no name: find it by `native_id`. The script can be shared: whoever runs it is asked for their own password the first time.
- A choice among a fixed set (a size, a country) is a parameter whose question lists the valid options.

## 4. Start from a clean state

A replay starts from nothing: the app may be closed and the page may not be open. The first steps of the script put the screen where the recording started:

- A desktop app: `launch` with the `window.path` the recording noted (or the executable name), then wait for its window.
- A web page: open the browser on `window.url`. If the address changes during the recording, the page changed because of an action (a Submit, a link); do not open the new address directly unless the person navigated to it by typing it.
- If the task starts with a file, open that file, or make it a parameter.

## 5. Turn each action into a command

Find an element right before acting on it, every time: refs belong to one snapshot and go stale when the screen changes. Look for it inside the task's window, by the most stable identity it has, in this order: `native_id`, then `role` with `name`, then `help_text` or `label`, then the same with its nearest named ancestor when `match_count` was above 1.

| Recorded | Command in the script |
|---|---|
| `click` on a button, link, tab, menu item | `click` the element |
| `click` with `state_after.checked` | `check` or `uncheck` the element (it is idempotent) |
| `type` into a field | `set-value` the element with `value` (or the parameter). If the field refuses it, the step cannot run in the background: say so |
| `type` with `secure: true` (a password) | `secret fill` the element with the secret's name |
| `type` into a grid cell or a canvas editor ("Cell Editor") | Select the cell (`select` or `click`), then `set-value` the cell or its editor with the whole value. A grid that only accepts typing cannot be filled in the background: say so |
| `select` | `select` the combo box with `value` |
| `press` (`return`, `ctrl+s`, `tab`) | `click` the button that does the same (Save, OK, Send, the form's submit button). A key with no button cannot run in the background: say so |
| `scroll` | Usually nothing: find the element the next action needs; `scroll-to` it if it is not visible |
| `drag` that selected cells or text | Select the same range semantically (`select`) |
| `drag` on a canvas (no named element) | Not available: drawing needs the mouse. Say so |
| A click with no name at all (a drawn grid, a canvas) | Not available: this build never uses the mouse. Look for the element's name in a fresh snapshot, or leave the step out and say why |
| A copy (`ctrl+c`) with `clipboard` | Read the source with `get` and write it where it goes with `set-value` (or `clipboard-set`) |

Scripts run in the background: the person can keep working, so never bring a window forward (no `focus-window`, `press`, `type` or mouse commands).

Actions in the script happen faster than a person's. After an action that opens or changes something (a dialog, a new page, a list that loads), wait for the element the next step needs before acting: `wait --text`, `wait --window`, or a `find` retried until it answers.

## 6. Check every step

A script that only acts can click a button that did nothing and go on. After each step, check what it should have left:

- A field holds the value typed: `get <ref> --property value`.
- A checkbox is checked: `is <ref> --property checked`.
- A new window, dialog or page appeared: `wait --window` or `wait --text`.
- At the end, the result the person wanted is there: the confirmation text, the saved file, the new row.

When a check fails, stop and print the step number, what was expected and what was found. Never go on after a failed check: the next steps would act on the wrong screen.

## 7. Actions that cannot be undone

Sending, posting, deleting, buying and paying cannot be taken back. Before such a step the script asks the person to confirm (`Read-Host "Send the invoice? (y/n)"`), unless the person said it should run on its own. A script never retries one of these steps.

## 8. Try it before handing it over

Run the script once from a clean state (the app closed, the page not open) with the recorded values. It must reach the final check. If it fails, fix that step (see `script-repair`) and run it again from the start, until it passes once end to end. Then give the person:

- the script and how to run it,
- its parameters, with their questions and example values,
- what it does not cover (anything you had to leave out, and why).

## A script, step by step

On Windows, a PowerShell script reads agent-desktop's JSON answers well. This skeleton has the helpers every script needs; each step then reads like the plain-words list from section 1.

```powershell
param(
    [Parameter(Mandatory)] [string] $full_name,   # The person's full name (e.g. Lucia Fernandez)
    [Parameter(Mandatory)] [string] $email,       # Their email (e.g. lucia@example.com)
    [int] $StopBefore = 0                          # for a repair: stop before this step (0 runs all)
)
$ErrorActionPreference = 'Stop'
$ad = 'agent-desktop'                              # or the full path to agent-desktop.exe

function Run([string[]] $arguments) {
    $answer = & $ad @arguments | ConvertFrom-Json
    if (-not $answer.ok) { throw "$($arguments[0]): $($answer.error.code) $($answer.error.message)" }
    $answer.data
}

# The task's window, found again by title each time the script runs.
function Window([string] $app, [string] $title, [int] $timeoutMs = 30000) {
    $until = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $until) {
        $w = Run @('list-windows', '--app', $app) | Where-Object { $_.title -like "*$title*" } | Select-Object -First 1
        if ($w) { return $w }
        Start-Sleep -Milliseconds 500
    }
    throw "No window of $app titled '$title'"
}

# An element of that window, by role and name (or native id), waiting until it is there.
function Element([object] $window, [string] $role, [string] $name, [string] $nativeId, [int] $timeoutMs = 15000) {
    $query = @('find', '--app', $window.app_name, '--window-id', $window.id, '--first')
    if ($nativeId) { $query += @('--native-id', $nativeId) } else { $query += @('--role', $role, '--name', $name) }
    $until = (Get-Date).AddMilliseconds($timeoutMs)
    while ((Get-Date) -lt $until) {
        $found = Run $query
        $match = $found.match                              # --first answers with one match
        if ($match -and $match.ref_id) { return $match.ref_id }
        Start-Sleep -Milliseconds 500
    }
    throw "No $role '$name' in '$($window.title)'"
}

function Step([int] $number, [string] $what, [scriptblock] $action) {
    if ($StopBefore -gt 0 -and $number -ge $StopBefore) { Write-Host "Stopped before step $number."; exit 0 }
    Write-Host "Step $number. $what"
    try { & $action } catch { throw "Step $number ($what) failed: $_" }
}

# --- The task ---------------------------------------------------------------
Step 1 'Open the form' {
    # Started through WMI, Chrome opens behind the person's windows (Start-Process would bring it forward).
    $chrome = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
    $behind = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{ ShowWindow = [uint16]4 }
    Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
        CommandLine = "`"$chrome`" --new-window https://demoqa.com/text-box"; ProcessStartupInformation = $behind } | Out-Null
    $script:page = Window 'chrome.exe' 'demosite'                # the recording's window.title
}
Step 2 'Type the full name' {
    $field = Element $page 'textfield' 'Full Name'
    Run @('set-value', $field, $full_name) | Out-Null
    if ((Run @('get', $field, '--property', 'value')).value -ne $full_name) { throw 'the name was not kept' }
}
Step 3 'Type the email' {
    $field = Element $page 'textfield' 'name@example.com'      # its placeholder: the recording's help_text
    Run @('set-value', $field, $email) | Out-Null
}
Step 4 'Submit' {
    Run @('click', (Element $page 'button' 'Submit')) | Out-Null
    Run @('wait', '--text', $email, '--app', 'chrome.exe', '--timeout', '10000') | Out-Null
}
Write-Host 'Done: the form shows the data sent.'
```

Each step finds its element again, acts, and checks. A failure says which step, what it was doing and the error, which is where a repair starts.
