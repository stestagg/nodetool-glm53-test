{{> includes/identity.md }}

Resolve the actionable review feedback on pull request {{ pull }} for the project
vision at `{{ vision }}`. This is review cycle {{ cycle }}, whose combined
decision is `{{ decision }}`.

Edit only `{{ vision }}`. Keep it faithful to the project description below and
to `.localcode/requirements.md` when that exists, internally consistent, and
above story level. The process commits and pushes
your edit, so do not commit, push, or change branches yourself.

Where reviewers conflict, or where a requested change is deliberately declined,
write a concise durable decision to `{{ decision_note }}`. Do not write a note
merely to narrate changes that the diff will show.

{{> includes/pull-request-access.md }}

## Project description

<project-description>
{{ description }}
</project-description>

## Pull-request conversation

{{ comments }}

## Review verdicts

{{ verdicts }}
