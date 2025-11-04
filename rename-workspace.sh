#!/usr/bin/env bash
# Script to rename the current niri workspace using zenity

set -euo pipefail

# Get current workspace info
current_workspace=$(niri msg --json workspaces | jq -r '.[] | select(.is_focused == true)')

if [ -z "$current_workspace" ]; then
    zenity --error --text="Could not find focused workspace"
    exit 1
fi

current_name=$(echo "$current_workspace" | jq -r '.name // ""')

# Show zenity dialog to get new name
if ! new_name=$(zenity --entry \
    --title="Rename Workspace" \
    --text="Enter a new name for workspace:" \
    --entry-text="$current_name" \
    2>/dev/null); then
    # User cancelled
    exit 0
fi

# Trim whitespace
new_name=$(echo "$new_name" | xargs)

if [ -z "$new_name" ]; then
    # If name is empty, clear the workspace name
    niri msg action unset-workspace-name
else
    niri msg action set-workspace-name "$new_name"
fi
