# .localcode

Everything localcode knows about this project, except the source itself.

| path | what it is |
| --- | --- |
| `personas/` | reusable perspectives and general attitudes, one plain Markdown file each |
| `roles/` | complete activity prompt templates, with shared Markdown under `roles/includes/` |
| `stories/` | stories and their lifecycle, from backlog through completion |
| `requirements.md` | what the project has been asked to do, one `REQ-<n>` id per requirement for stories to cite |
| `vision.md` | the backdrop stories are written against: the problem, the product's shape, scope, and direction |
| `GOOD_CODE.md` | what good code, tests, comments, and documentation look like in this project |
| `tools/` | project-specific workflow tools the agents drive the work through |
| `opencode.json` | which LLM this project uses, in opencode's config format |
| `state/` | gitignored: gitea's database, the bare master repo, the runtime secret, and the provider keys |

Everything outside `state/` is committed on `main`, beside the code. Agents read
and write these files there, and a pull request that implements a story may not
change them.

Workflows choose an activity role and the persona that performs it. A top-level
role file is the complete prompt template: `{{ value }}` marks runtime data and
`{{> includes/name.md }}` inserts shared project-owned text. Files below
`roles/includes/` are not roles themselves.

The project name and Gitea repository name come from the repository directory.
The HTTP port is runtime-only and can be selected with `localcode run --port`.

`opencode.json` names the model but never the key: keys live in
`state/opencode-auth.json`, which is not committed. Add the model selection to
the supplied config, then use `localcode llm configure` to write the credential.

The supplied compaction setting reserves 65,536 tokens for OpenCode, so it
summarises a session well before it reaches a model's context limit. Keep this
unless the selected model has a particularly small context window; then lower
`compaction.reserved` to leave it enough room to work.
