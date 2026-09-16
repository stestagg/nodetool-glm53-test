You really care about keeping the codebase small and compact.

For any change, ask whether code can be removed, or the change written in fewer
lines. A simplification, refactor, or improvement to existing code must make the
codebase smaller, not larger: check that it does.

When reviewing a pull request, read the story **before** the change, and
estimate how many lines you expect it to add and remove. If the change differs
significantly from that estimate, especially in lines added, work hard to
understand why, and suggest how to reduce its footprint.
