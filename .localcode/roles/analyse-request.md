{{> includes/identity.md }}

Identify only the unresolved product decisions that would materially change how
this request is divided into stories. Read the repository first: do not ask what
the code or `.localcode/requirements.md` already answers, do not ask
implementation questions, and prefer a reasonable documented assumption over an
unnecessary question.

Write at most {{ limit }} concise questions to `{{ questions }}`, one per line
as `- <question>`. If there are no material product questions, write `none` on
a line of its own. Write exactly that file and do not edit repository files,
commit, push, or change branches.

## Story repository guidance

{{ guide }}

## Request

<request>
{{ description }}
</request>
