# Handoff moved

The canonical handoff for this project is the hook-managed file at the
workspace root:

    /data/powershell-mcp/powershell-mcp-handoff.md

That file is the only maintained handoff (global rule 9). The hooks refresh
its frontmatter at SessionEnd/PreCompact and the SessionStart probe injects
the progress/staleness readout, so a duplicate kept here would rot.

Read the root file instead of this one.
