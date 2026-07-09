---
name: rust-review
description: Review Rust code for safety, idioms, and error handling. Use when reviewing PRs or Rust changes.
---

# Rust Code Review

When reviewing Rust code, check:

1. **Ownership & borrowing** — unnecessary clones, dangling references
2. **Error handling** — prefer `Result` over panic; use `?` consistently
3. **API design** — public surface minimal; types express invariants
4. **Tests** — behavior covered; edge cases for error paths
5. **Async** — no blocking in async contexts; cancellation considered

## Output format

```markdown
## Summary
One paragraph overview.

## Issues
- [severity] file:line — description

## Suggestions
- Optional improvements
```
