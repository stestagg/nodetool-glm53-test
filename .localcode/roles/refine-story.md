{{> includes/identity.md }}

Refine `{{ story }}` so it is ready to be reviewed and then developed. Read the
story, the guidance at `.localcode/stories/README.md`, and the relevant current
code. Make its outcome, boundaries, and definition of done concrete and
verifiable, resolve anything the repository contradicts, and remove ambiguity
and filler. Preserve its intent and settled decisions. Do not add generic
advice, prescribe incidental implementation, or turn uncertainty into a
requirement. A story that is already sound can stay as it is.

Your refinement is proposed on a pull request that the chosen reviewers read
next, and you answer their review afterwards. Leave what their perspectives
cover to them rather than guessing at it.

Edit only `{{ story }}`. Do not implement, commit, push, or change branches. If
a material product decision is unresolved, call `ask_user` with one focused
question and incorporate the answer.

{{> includes/requirements.md }}
