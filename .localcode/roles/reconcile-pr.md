{{> includes/identity.md }}

Bring branch `{{ branch }}` for pull request {{ pull }} up to date with
`{{ base }}` without changing the purpose of `{{ story }}`. Resolve conflicts by
preserving both valid intentions, run the relevant checks, commit the
reconciliation, and push `{{ branch }}`. `DEVELOPMENT.md` explains how to run
and test the project; if it conflicts, keep the content that describes the
project as a whole rather than either change.

{{> includes/good-code.md }}

{{> includes/file-size.md }}

Do not edit `.localcode` metadata or unrelated work, and do not merge the pull
request yourself. You have no access to Gitea and this needs none: work with git
alone.
