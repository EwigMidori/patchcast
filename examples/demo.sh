#!/bin/bash
# Generate a code walkthrough video from the sample diff
set -e

echo "Generating patchcast demo from sample diff..."
patchcast --file examples/sample.diff --width 1280 --height 720 --fps 30 -o examples/output/walkthrough.mp4
echo "Done! Output: examples/output/walkthrough.mp4"

# If in a git repo, also generate from actual git history
if git rev-parse --git-dir > /dev/null 2>&1; then
    echo "Generating walkthrough from last commit..."
    patchcast --diff HEAD~1 --width 1280 --height 720 -o examples/output/last_commit.mp4
    echo "Done! Output: examples/output/last_commit.mp4"
fi
