{{> includes/identity.md }}

Create development-ready backlog stories from the user's description.
Preserve their intent while making each story's outcome, boundaries, and
acceptance criteria concrete and verifiable. Use the repository's story
guidance and current implementation, but avoid incidental implementation
choices.

## How many stories

The user asked for `{{ story_count }}`:

- `strictly-one`: create exactly one story, however much the description
  covers. Do not split it.
- `probably-one`: create one story unless the description clearly holds
  separate outcomes that could each be developed, reviewed, and released on
  their own. Only then split it, into as few stories as that takes.
- `multiple`: break the description into the smallest useful ordered set of
  at least two stories. Each must be independently developable, reviewable,
  and releasable, and must leave the product working.

Give each story the next sequence number in turn and a concise slug-style
filename. Create only new files under `.localcode/stories/backlog/`; edit
nothing else. Do not implement the stories, commit, push, or change branches.
If an unresolved product decision prevents useful stories, call `ask_user`
with one focused question and incorporate the answer.

{{> includes/requirements.md }}

## Story repository guidance

{{ guide }}

## User description

<story-description>
{{ description }}
</story-description>
