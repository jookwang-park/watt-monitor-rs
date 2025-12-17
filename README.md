<div align="center">
  <img src="https://raw.githubusercontent.com/jookwang-park/watt-monitor-rs/refs/heads/main/assets/preview.png" alt="Watt Monitor Screenshot" width="100%" />

  <h1>Watt Monitor</h1>

  <p>
    <strong>A lightweight, terminal-based power monitoring tool for Linux laptops.</strong>
  </p>

  <p>
    Visualize battery consumption, power draw (Watts), and sleep efficiency in real-time.
  </p>
</div>

---

## Introduction

**Watt Monitor** is a TUI (Text User Interface) application written in Rust. It helps Linux users understand their laptop's energy usage patterns by plotting Battery Capacity (%) and Power Draw (W) on a real-time chart.

Unlike simple battery applets, Watt Monitor specifically captures and analyzes **Sleep/Suspend periods**. It visualizes when your laptop was asleep and calculates the battery drain rate during those times, helping you identify "sleep drain" issues.

### Key Features

*   **Real-time Dashboard**: Dual-axis chart showing Capacity (Cyan) and Power (Yellow).
*   **Intelligent Sleep Detection**: Automatically detects sleep periods, removes empty gaps from the chart, and markers wake-up times.
*   **Sleep Analysis**: displays duration and battery percentage lost during sleep.
*   **Flexible View Modes**: Switch between Recent (30m, 1h, 4h, 12h) and Full Day views.
*   **History Navigation**: Browse past daily logs archived automatically.
*   **Lightweight Daemon**: Uses a background service (systemd or OpenRC) to log data with minimal resource impact.

## Installation

### Prerequisites
*   Rust (Cargo)
*   Make
*   Systemd or OpenRC (for the background logger)

### Building from Source

Clone the repository and install using the provided Makefile:

```shell
git clone https://github.com/jookwang-park/watt-monitor-rs.git
cd watt-monitor-rs
sudo make install
```

This will install the binary to `/usr/local/bin` and service files to the appropriate system directories.

## Usage

Watt Monitor consists of two parts: a background **daemon** that collects data, and the **TUI** interface to view it.

### 1. Enable the Background Daemon

To start collecting battery data, enable the service. This runs silently in the background, logging status every 4 seconds.

**For Systemd:**
```shell
sudo make enable
# Or manually: sudo systemctl enable --now watt-monitor.service
```

> **Note**: Data is initially written to `/tmp` to protect SSD lifespan and rotated to `~/.local/share/watt-monitor/` at midnight.

### 2. Launch the Monitor

Open your terminal and run:

```shell
watt-monitor
```

### 3. Key Controls

| Key | Action |
| :--- | :--- |
| `Tab` | Cycle view modes (30m → 1h → 4h → 12h → Full) |
| `h` or `←` | View previous day's log |
| `l` or `→` | View next day's log |
| `q` or `Esc` | Quit application |

## 🤝 Contributing

Contributions are welcome! Whether it's reporting a bug, suggesting a feature, or submitting a Pull Request, your input is valued.

1.  Fork the repository
2.  Create your feature branch (`git checkout -b feature/amazing-feature`)
3.  Commit your changes
4.  Push to the branch
5.  Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
