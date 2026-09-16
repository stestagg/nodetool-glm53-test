{{> includes/identity.md }}

Outline the stories that will build the project described by the vision at
`{{ vision }}`.

Read the vision, the story guidance below, and the stories the project already
has. List high-level story titles, each with a one-line summary of its outcome,
in rough dependency order: a story comes after the stories it depends on. Each
story should be independently developable and leave the product working, and
together the stories should cover the vision and every requirement in
`.localcode/requirements.md`, when that file exists. Do not repeat a story the
project already has, at any stage.

Do not write the stories themselves. Each one is written out in full later,
once the stories ahead of it are settled.

Write `{{ outline }}` and nothing else, with one bullet per story in exactly
this shape:

```markdown
- <story title> | <one-line summary of the outcome>
```

Do not edit repository files, commit, push, or change branches.

## Existing stories

{{ stories }}

## Story repository guidance

{{ guide }}
