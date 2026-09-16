{{> includes/identity.md }}

Make the requested change to `{{ story }}` and nothing else. Read the story,
the guidance at `.localcode/stories/README.md`, and relevant current code.
Integrate the change where it belongs, preserve settled decisions and voice,
and keep the story implementable and verifiable.

Keep the story consistent with the project's requirements in
`.localcode/requirements.md`, when that file exists: cite a requirement's
`REQ-<n>` id where the edited story meets it, and if the requested change would
contradict a requirement, call `ask_user` rather than choosing between them.

Edit only `{{ story }}`. Do not implement, commit, push, or change branches. If
the request and repository cannot settle a material product decision, call
`ask_user` with one focused question and incorporate the answer.

## Requested edit

<edit-request>
{{ instruction }}
</edit-request>
