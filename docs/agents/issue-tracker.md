# Issue tracker: GitHub

Issues and specs live in GitHub Issues for `zhugez/pingloom`.
Use the `gh` CLI from this clone; it infers the repo from the remote.
Outside the clone, pass `--repo zhugez/pingloom`.

## Operations

- Create: `gh issue create --title "..." --body "..."`
  Use a heredoc for multi-line bodies.
- Read: `gh issue view <number> --comments`
  Fetch labels too when evaluating triage state.
- List: `gh issue list --state open --json number,title,body,labels,comments`
  Add label and state filters as appropriate.
- Comment: `gh issue comment <number> --body "..."`
- Label: `gh issue edit <number> --add-label "..."`
- Remove label: `gh issue edit <number> --remove-label "..."`
- Close: `gh issue close <number> --comment "..."`

“Publish to the issue tracker” means create a GitHub issue.
“Fetch the relevant ticket” means read the issue and its comments.

## Pull requests as a triage surface

**PRs as a request surface: no.**

GitHub shares issue and PR numbers. Resolve ambiguous references with
`gh pr view <number>`, falling back to `gh issue view <number>`.
