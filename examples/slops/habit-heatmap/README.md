# Habit Heatmap

A bundled v1 habit tracker. Pick a habit and tap a day to mark or clear it.
Add and edit habits with the plus and Edit buttons. Choose from twelve colors.
Remove a habit and its check-in history from Edit, then confirm removal. The calendar shows twelve
Monday-based weeks, including the current week; future days are disabled.
The streak includes today when complete, or runs through yesterday until then.
Check-in totals refer to the visible period; streaks use all saved history.

The three starter habits have no completed days. Habit rows use Loro identity;
check-ins are integer records keyed by local calendar date. Selection and form
drafts are local UI state. No authored SQLite or legacy data migration is used.

From the repository root:

```sh
bun slop dev examples/slops/habit-heatmap
bun slop build examples/slops/habit-heatmap
bun slop register examples/slops/habit-heatmap
```

Create a writable copy to test saving. PNG/PDF exports include every habit;
the icon and Quick Look preview are generated during build.
