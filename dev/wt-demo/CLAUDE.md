# Demo Recording

## Running the Demo

Build and record the demo GIF:

```bash
python3 dev/wt-demo-build.py
```

This creates:
- `dev/wt-demo/out/wt-demo.gif` - The animated demo
- `dev/wt-demo/out/run.txt` - Text log of the output

## Viewing Results

**Do NOT use `open` on the GIF** - that's for the user to do manually.

To inspect what the demo produces:
- Read `dev/wt-demo/out/run.txt` to see the text output
- The GIF can be viewed in a browser or image viewer by the user

Claude Code cannot view GIFs directly. Use `run.txt` for verification.

## Prerequisites

- `wt` (worktrunk) installed and in PATH
- `vhs` for recording
- `starship` for prompt
- `llm` CLI with Claude model configured (for commit message generation)
- `cargo-nextest` for running tests
- Python 3

## Files

- `demo.tape` - VHS tape file with recording script
- `wt-demo-build.py` - Python build script that sets up demo repo and records
- `fixtures/` - Extracted content files (README, lib.rs, etc.)
- `out/` - Output directory (gitignored)
