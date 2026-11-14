# Repairing a script

Use this when a script made from a recording (`recording-to-script`) stops working: it fails at a step, or it runs to the end but the result is not complete. Apps and pages change: a button is renamed, a field moves, a new field appears, a dialog shows up that was not there. A repair finds the step that no longer matches, changes only that step, and checks the whole script again.

This is a manual repair, done once, with the person. The script itself does not learn or adapt while it runs.

## 1. Find the first step that is wrong

The step that failed is not always the one that changed. A field that silently kept only part of the text makes the script fail much later, at a step that was fine. So:

1. Run the script from a clean state (the app closed, the page not open) with the values that failed, and read its output: the last step it started, and the error.
2. If it failed at a check, or reached the end with a wrong result, go back through the steps before it: the first step whose check fails is where the repair starts. A script without checks after each step should get them now (section 6 of `recording-to-script`), because they are what point at the real cause.

## 2. Bring the screen to that step

Run the script again from a clean state, but stop right before the step that is wrong: every step before it still works, so let the script do them rather than doing them by hand. A script built from the skeleton in `recording-to-script` can take a `-StopBefore` number, or you can comment out the steps after it while you repair.

Do not repeat an action that cannot be undone (send, post, delete, buy) to get there. If the failing step comes after one, use test data, or ask the person.

## 3. Look at what is there now

With the screen at the failing step, look before changing anything:

- `snapshot` of the task's window (or `find` with a looser filter) to see the elements that are there now, with their roles, names and native ids.
- `screenshot` when the tree is not enough.
- The recording (and its video, if there is one) to see what the step found when it was recorded.

Then name the cause. The usual ones:

| What you see | What to change in the step |
|---|---|
| The element is there with another name | Find it by its `native_id`, which rarely changes, or by its new name |
| Several elements now match | Add the nearest named ancestor, or pick it by `native_id` |
| The element is inside a new tab, panel or menu | Add the step that opens it first, then find the element inside |
| A new field is required | Add a step for it. Its value comes from the person: make it a parameter with a question and an example, never an invented value |
| A field is gone | Remove its step and its parameter if nothing else uses it |
| A dialog or banner now appears (cookies, a tip, a confirmation) | Add a step that answers it the way a person would (the least consent for cookies), only when it is there |
| The element appears later than before | Wait for it before acting (`wait --text`, `wait --window`, or a longer `find` timeout) |
| The page or app moved (new address, new executable path) | Change the first steps that open it |
| The step works but the next screen is different | The task changed after this step: repair the next step the same way |

## 4. Change only that step

Change the smallest thing that makes the step work again: its way of finding the element, a wait, one added or removed step. Keep everything else as it was, and keep the values and parameters exactly; the person relies on them. If the repair needs a new value, ask the person for it.

After the change, check the step: run it on the screen you prepared and confirm what it left (the value in the field, the dialog that opened). Add a check after it if it had none.

When the app may be in either version (some computers updated, some not), the step can try the new way and fall back to the old one:

```powershell
Step 4 'Submit' {
    try   { $button = Element $page 'button' 'Send' -timeoutMs 5000 }      # new name
    catch { $button = Element $page 'button' 'Submit' }                     # as recorded
    Run @('click', $button) | Out-Null
}
```

Never try both ways of a step that cannot be undone: if the first one acted, the second one would act again.

## 5. Run it all again

Run the whole script from a clean state with the values that failed. If it fails at a later step, repair that one the same way. The repair is finished when the script runs once from start to end and its final check passes.

Keep the previous version of the script (a copy with the date, or version control) so the person can go back to it, and tell them:

- which step changed and why,
- any new parameter and its question,
- anything the script no longer does because the app no longer has it.

## 5b. When it is easier to record again

If most steps changed (a redesigned app, a new site), record the task again and build a new script from the new recording instead of repairing many steps. You can also record only the part that changed and replace those steps with the ones the new recording gives.
