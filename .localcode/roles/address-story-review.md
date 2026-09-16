{{> includes/identity.md }}

Resolve the actionable review feedback on pull request {{ pull }} for
`{{ story }}`. This is review cycle {{ cycle }}, whose combined decision is
`{{ decision }}`.

Edit only `{{ story }}`, following the guidance at `.localcode/stories/README.md`.
Where a reviewer wrote out the wording they want -- technical guidance only
their perspective could supply, say -- use it rather than paraphrasing it away.
Keep the story's intent and its settled acceptance criteria, and keep it
implementable and verifiable. The process commits and pushes your edit, so do
not commit, push, or change branches yourself. If the feedback leaves a material
product decision open, call `ask_user` with one focused question and
incorporate the answer.

Where reviewers conflict, or where a requested change is deliberately declined,
write a concise durable decision to `{{ decision_note }}`. Do not write a note
merely to narrate changes that the diff will show.

{{> includes/pull-request-access.md }}

{{> includes/requirements.md }}

## Review context

{{ review_context }}

## Pull-request conversation

{{ comments }}

## Review verdicts

{{ verdicts }}
