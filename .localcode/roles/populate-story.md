{{> includes/identity.md }}

Write out the placeholder story `{{ story }}` so it is ready for development, or
decide that it cannot be written yet.

Read the vision at `{{ vision }}`, the story guidance below, the other stories
listed, and the relevant current code. The placeholder names the story and
sketches its outcome. Replace its placeholder text with a complete description
and user-verifiable definition of done that fit the vision and the stories
around it, and remove the `placeholder: true` line from its frontmatter. Keep
its title unless the content makes it misleading.

While writing it, you may find that part of it belongs in a separate story, or
that the backlog is missing something. You may add at most {{ new_limit }} such
stories as new files under `.localcode/stories/backlog/`, numbered from
`{{ first }}`, with slug-style names. Each new file is a skeleton only, in
exactly the shape of the placeholder template below and keeping
`placeholder: true`. It is written out in its own turn later.

{{ deferral }}

Then write `{{ decision }}` in exactly this shape and nothing else:

```markdown
---
decision: <populated | deferred>
---

<one short paragraph: what you settled, or, for a deferral, which unfinished
stories hold the uncertainty and why>
```

Edit only `{{ story }}` and any new backlog skeletons. Do not implement anything,
commit, push, or change branches.

{{> includes/requirements.md }}

## Project description

<project-description>
{{ description }}
</project-description>

## Stories by stage

{{ stories }}

## Placeholder template

{{ template }}

## Story repository guidance

{{ guide }}
