# re_schedule

`re_schedule` is a lightweight Windows utility written in Rust that automatically manages the priority, CPU affinity, IO priority, memory priority, and power throttling of specified processes. It runs in the background and periodically checks for target processes, applying the configured settings if they are not already set.

## Features

- **CPU Priority**: Set process priority class (Idle, Below Normal, Normal, Above Normal, High, Realtime).
- **CPU Affinity**: Pin processes to specific CPU cores.
- **IO Priority**: Set Input/Output priority (Very Low, Low, Normal, High, Critical).
- **Memory Priority**: Set memory priority (Lowest, Very Low, Low, Medium, Below Normal, Normal).
- **Power Throttling**: Enable or disable Windows power throttling (EcoQoS).
- **Efficient**: Only applies settings if the current values differ from the target configuration.

## Configuration

Configuration is done via `config.toml` in the same directory as the executable.

### Settings

- `refresh_interval_secs`: How often (in seconds) to check for processes.

### Targets

Define one or more `[[targets]]` blocks.

- `name`: The process image name (e.g., "notepad.exe"). Case-insensitive.
- `priority`: (Optional) "idle", "below_normal", "normal", "above_normal", "high", "realtime".
- `affinity`: (Optional) Array of CPU core indices (0-based). E.g., `[0, 1, 2, 3]`.
- `io_priority`: (Optional) "very_low", "low", "normal", "high", "critical".
- `memory_priority`: (Optional) "lowest", "very_low", "low", "medium", "below_normal", "normal".
- `power_throttling`: (Optional) "enabled" or "disabled".

### Example `config.toml`

```toml
[settings]
refresh_interval_secs = 5

[[targets]]
name = "game.exe"
priority = "high"
affinity = [0, 1, 2, 3, 4, 5, 6, 7] # First 8 cores
io_priority = "high"
power_throttling = "disabled"

[[targets]]
name = "background_task.exe"
priority = "idle"
io_priority = "very_low"
power_throttling = "enabled"
```

## Building

Ensure you have Rust installed.

```bash
cargo build --release
```

The executable will be located at `target/release/re_schedule.exe`.

## Usage

1.  Create or modify `config.toml`.
2.  Run `re_schedule.exe`.
3.  Keep the terminal window open (or run it in a way that hides the window).
4.  Press `Ctrl+C` to stop.

## Requirements

- Windows 10/11
- Administrator privileges might be required to set priorities for some processes (e.g., system processes or processes owned by other users).
