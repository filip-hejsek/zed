# Manual testing: commit message trailers and `git.commit_message_format`

Create a scratch repo with test commits:

```sh
git init /tmp/trailers && cd /tmp/trailers
git commit --allow-empty -F - <<'MSG'
Fix crash when opening large files

The body is hard-wrapped at 72 columns, so it should reflow to the
width of the panel instead of keeping these line breaks.

- A list item
- Another one with `code`

Signed-off-by: Alice <alice@example.com>
Co-authored-by: Bob <bob@example.com>
Link: https://example.com/some/long/url
MSG
git commit --allow-empty -F - <<'MSG'
Trailer with continuation line

Body.

Reported-by: Someone with a very long name
  that continues on the next line
Fixes: #123
MSG
git commit --allow-empty -F - <<'MSG'
Mixed last paragraph is not treated as trailers

Body.

Signed-off-by: Alice <alice@example.com>
[Alice: code style fixes]
Signed-off-by: Bob <bob@example.com>
MSG
git commit --allow-empty -m "Subject only: no trailers"
```

## Markdown mode (default)

- [ ] Commit view: body reflows, list renders as a list, trailers show one per line
- [ ] Commit view: trailers sit right below the body with normal paragraph spacing (no gap missing or doubled)
- [ ] Commit view: the trailer URL is clickable
- [ ] Commit view: collapsed state still shows only the first line; expanding shows body and trailers
- [ ] Commit view: long message scrolls; scrollbar covers body and trailers together
- [ ] Git graph details panel: same checks as the commit view
- [ ] Git graph: text wraps to the panel width (no horizontal overflow) with body and trailers
- [ ] Blame tooltip (hover inline blame): same checks; tooltip stays within its max height and scrolls
- [ ] Blame tooltip: drag-selecting text near the edge autoscrolls the tooltip (body and trailers)
- [ ] Continuation-line commit: the continuation line stays under its trailer
- [ ] Mixed-paragraph commit: last paragraph is reflowed like normal text (25% rule not implemented)
- [ ] Subject-only commit: `Subject only: no trailers` is not treated as a trailer
- [ ] Selecting text and copying from body and trailers gives the original text

## Plain mode

- [ ] Set `"git": { "commit_message_format": "plain" }`
- [ ] Open commit view / git graph / tooltip: line breaks are kept exactly, no Markdown formatting (`-` and backticks shown literally)
- [ ] Long lines wrap to the panel width
- [ ] URLs are still clickable

## Setting changes

- [ ] With a commit view and git graph open, switch the setting back and forth: both update without reopening
- [ ] Settings UI: Version Control → Commit Messages → Commit Message Format dropdown shows both values and writes the setting
- [ ] Invalid value (e.g. `"html"`) in settings.json falls back to the default without breaking other git settings

## Regressions

- [ ] Stash view (commit view opened for a stash) still shows its message
- [ ] Splitting a commit view tab (clone of the item) keeps the message and its format
- [ ] Empty commit message (`git commit --allow-empty-message -m ""`) shows nothing / `<no commit message>` as before
