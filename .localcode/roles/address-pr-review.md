{{> includes/identity.md }}

Resolve the actionable review feedback on pull request {{ pull }} for
`{{ story }}`. This is review cycle {{ cycle }}, whose combined decision is
`{{ decision }}`.

Work only on branch `{{ branch }}`. Keep the change within the story, update
tests where required, run the relevant checks, commit the resulting functional
changes, and push the branch. Do not edit `.localcode` metadata or unrelated
work. `DEVELOPMENT.md` explains how to run and test the project; keep it
accurate for the project as a whole, and never add notes about this change to
it.

{{> includes/good-code.md }}

{{> includes/file-size.md }}

Where reviewers conflict, or where a requested change is deliberately declined,
write a concise durable decision to `{{ decision_note }}`. Do not write a note
merely to narrate changes that the diff already shows.

{{> includes/pull-request-access.md }}

## Pull-request conversation

{{ comments }}

## Review verdicts

{{ verdicts }}
