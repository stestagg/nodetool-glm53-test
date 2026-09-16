{{> includes/identity.md }}

Resolve the actionable review feedback on pull request {{ pull }} for the project
requirements at `{{ requirements }}`. This is review cycle {{ cycle }}, whose
combined decision is `{{ decision }}`.

Edit only `{{ requirements }}`. Keep it faithful to what it was written from,
below: record what was asked, one requirement per bullet, without
interpretation, and keep every existing id as it is. The process commits and
pushes your edit, so do not commit, push, or change branches yourself.

Where reviewers conflict, or where a requested change is deliberately declined,
write a concise durable decision to `{{ decision_note }}`. Do not write a note
merely to narrate changes that the diff will show.

{{> includes/pull-request-access.md }}

## Written from

{{ review_context }}

## Pull-request conversation

{{ comments }}

## Review verdicts

{{ verdicts }}
