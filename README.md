# Time Tracking CLI

A command-line utility for tracking and analyzing your work time with support for multiple output formats. This tool creates daily time tracking files, opens them in your default editor, and then parses the data to provide detailed summaries.

## Installation

### Recommended: Install with symlink setup

```bash
git clone <repo-url>
cd time-tracking-cli
bash install.sh
```

#### Manual steps without symlink setup

```bash
# Clone and install
git clone <your-repo-url>
cd time-tracking-cli
cd site
yarn
yarn build
cd ..
# The previous steps build the web assets for the site accessible via `ttcli --serve`
cargo install --path .
```

This will install the full `ttcli` binary and website

## Usage

```bash
# These are equivalent:
ttcli --help

# Use today's date
ttcli

# Specify a date (positional argument)
ttcli 2025-10-03

# Specify a date (using flag)
ttcli --date 2025-10-03
```

## Output Formats

The CLI supports multiple output formats via the `--formatter` option:

```bash
# Default format (emoji-rich, colorful)
ttcli 2025-10-03

# Plain text format (no emoji)
ttcli --formatter plain 2025-10-03

# Markdown format
ttcli --formatter markdown 2025-10-03
```

### Format Examples

**Default format:**

```
📅 TIME OVERVIEW
⏱️  WORKING TIME
📋 PROJECTS
  📌 code1 - 5:00 (5.00 hrs)
```

**Plain format:**

```
TIME OVERVIEW
WORKING TIME  
PROJECTS
  * code1 - 5:00 (5.00 hrs)
```

**Markdown format:**

```
**Total Time:** 8.25 hours
**Projects:**
  - **code1** - 5.00 hours
```

### How it works

1. **Directory Creation**: Creates `~/.time-tracking/` directory if it doesn't exist
2. **File Creation**: Creates or opens a `YYYY-MM-DD.md` file for the specified date
3. **Editor Launch**: Opens the file in your default `$EDITOR` (or `$VISUAL`)
4. **Parse & Display**: After you save and exit the editor, parses the time data and displays a summary

### Time Tracking Format

In the editor, enter your time tracking data using this format:

```
# Time Tracking - 2025-10-03

11:45-12:15 project_code1
- Comment about what you worked on
12:15-1:30 project_code2
- Another comment about your work
1:30-2 project_code1
- More work on the first project
2-4 project_code3
- Different project work
```

### Time Format

- **Time ranges**: Use 12-hour format (e.g., `11:45-12:15`, `2-4`, `2:30-3`). There's really no concept of "AM" or "PM" here; just use the times that make sense for your day.
- **Project codes**: Any alphanumeric identifier (e.g., `code1`, `client-bd`, `admin`)
- **Comments**: Lines following a time entry are treated as notes for that entry

### Features

- **Time Aggregation**: Time spent on the same project code is automatically summed up
- **Dead Time Detection**: Identifies gaps in your time tracking
- **Multiple Formats**: Supports various time formats (with/without minutes, 24-hour format)
- **Detailed Reports**: Shows start/end times, total working time, and per-project breakdowns

### Example Output

```
============================================================
TIME TRACKING SUMMARY
============================================================

📅 TIME OVERVIEW
Start Time: 11:45
End Time:   4:00

⏱️  WORKING TIME
Total: 4:15 (4.25 hours)

⏸️  DEAD TIME
✅ No dead time (gaps) found

📋 PROJECTS

  📌 project_code1 - 1:00 (1.00 hrs)
     • Comment about what you worked on
     • More work on the first project

  📌 project_code2 - 1:15 (1.25 hrs)
     • Another comment about your work

  📌 project_code3 - 2:00 (2.00 hrs)
     • Different project work
```

### Editor Configuration

The tool uses your system's default editor. You can configure it by setting environment variables:

```bash
export EDITOR=vim
export VISUAL=code
```

Common editors:

- `vim` or `nvim` for Vim/Neovim
- `code` for VS Code
- `nano` for a simple terminal editor
- `emacs` for Emacs

### Files Location

All time tracking files are stored in `~/.time-tracking/` with the naming convention `YYYY-MM-DD.md`.

## Dependencies

- `clap` - Command-line argument parsing
- `chrono` - Date/time handling
- `dirs` - Platform-agnostic directory access
- `time-tracking-parser` - Time parsing and aggregation logic

## License

This project uses the same license as the `time-tracking-parser` dependency.
