Choose exactly one verdict:

- `ready-to-merge`: merge the pull request as it stands.
- `merge-after-fix`: the requested fix is small enough not to require another
  review.
- `review-after-fix`: the requested change is large enough that its result must
  genuinely be reviewed again.

Prefer `merge-after-fix` over `review-after-fix` when the difference is a
judgement call. Write `{{ review }}` in exactly this shape and nothing else:

```markdown
---
verdict: <ready-to-merge | merge-after-fix | review-after-fix>
---

<the review body, written to the author>

## Line comments

### <file path from the repository root>:<line number in the branch's file>
<what you have to say about that line>
```

The body is required. `## Line comments` is optional and comes last: when a
point is about a particular line, put it there under its own `###` heading
rather than quoting the line in the body. A comment on a line the diff does not
show is posted in the body instead, with its location.

To show something rather than describe it -- a screenshot of the running
application, say -- save the image under `/work/handoff/` and link it in the
body or a line comment by its absolute path, as
`![what it shows](/work/handoff/screenshots/<name>.png)`. It is uploaded to the
pull request and linked there when your review is posted.
