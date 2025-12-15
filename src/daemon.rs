use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::{Local, NaiveDate};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;

use crate::data::{get_data_dir, get_today_log_path};

const LOG_INTERVAL_SECS: u64 = 4;
const CSV_HEADER: &str = "Time,Status,Capacity(%),Power(W)";

struct BatteryInfo {
    timestamp: String,
    status: String,
    capacity: u8,
    power_watts: f64,
}

fn find_battery_path() -> Option<PathBuf> {
    for name in ["BAT0", "BAT1", "BATT"] {
        let path = PathBuf::from(format!("/sys/class/power_supply/{}", name));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn read_battery_info(battery_path: &PathBuf) -> io::Result<BatteryInfo> {
    let status = fs::read_to_string(battery_path.join("status"))?
        .trim()
        .to_string();

    let capacity: u8 = fs::read_to_string(battery_path.join("capacity"))?
        .trim()
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let power_uw: u64 = fs::read_to_string(battery_path.join("power_now"))
        .unwrap_or_else(|_| "0".to_string())
        .trim()
        .parse()
        .unwrap_or(0);
    let power_watts = power_uw as f64 / 1_000_000.0;

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    Ok(BatteryInfo {
        timestamp,
        status,
        capacity,
        power_watts,
    })
}

fn get_pid_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("watt-monitor.pid")
    } else {
        PathBuf::from("/tmp/watt-monitor.pid")
    }
}

fn is_already_running(pid_path: &PathBuf) -> bool {
    if let Ok(content) = fs::read_to_string(pid_path) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            return PathBuf::from(format!("/proc/{}", pid)).exists();
        }
    }
    false
}

fn create_pid_file(pid_path: &PathBuf) -> io::Result<()> {
    let pid = std::process::id();
    fs::write(pid_path, pid.to_string())?;
    Ok(())
}

fn remove_pid_file(pid_path: &PathBuf) {
    fs::remove_file(pid_path).ok();
}

fn write_csv_record(info: &BatteryInfo) -> io::Result<()> {
    let log_path = get_today_log_path();

    if !log_path.exists() {
        let mut file = File::create(&log_path)?;
        writeln!(file, "{}", CSV_HEADER)?;
    }

    let mut file = OpenOptions::new().append(true).open(&log_path)?;

    writeln!(
        file,
        "{},{},{},{:.2}",
        info.timestamp, info.status, info.capacity, info.power_watts
    )?;

    Ok(())
}

fn rotate_archive(date: NaiveDate) -> io::Result<()> {
    let log_path = get_today_log_path();
    if !log_path.exists() {
        return Ok(());
    }

    let data_dir = get_data_dir();
    fs::create_dir_all(&data_dir)?;

    let date_str = date.format("%Y-%m-%d").to_string();
    let archive_path = data_dir.join(format!("{}.csv", date_str));

    let content = fs::read_to_string(&log_path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() <= 1 {
        return Ok(());
    }

    if !archive_path.exists() {
        let mut file = File::create(&archive_path)?;
        writeln!(file, "{}", CSV_HEADER)?;
    }

    let mut archive_file = OpenOptions::new().append(true).open(&archive_path)?;

    for line in lines.iter().skip(1) {
        if line.starts_with(&date_str) {
            writeln!(archive_file, "{}", line)?;
        }
    }

    let mut file = File::create(&log_path)?;
    writeln!(file, "{}", CSV_HEADER)?;

    eprintln!("Archived data for {}", date_str);

    Ok(())
}

pub fn run() -> io::Result<()> {
    let pid_path = get_pid_path();

    if is_already_running(&pid_path) {
        eprintln!("Error: Daemon is already running");
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Daemon is already running",
        ));
    }

    let battery_path = find_battery_path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No battery found in system"))?;

    create_pid_file(&pid_path)?;

    let running = Arc::new(AtomicBool::new(true));
    flag::register(SIGTERM, Arc::clone(&running))?;
    flag::register(SIGINT, Arc::clone(&running))?;

    let mut current_date = Local::now().date_naive();

    eprintln!(
        "Daemon started (PID: {}), logging every {} seconds",
        std::process::id(),
        LOG_INTERVAL_SECS
    );
    eprintln!("Battery path: {:?}", battery_path);
    eprintln!("Log file: {:?}", get_today_log_path());
    eprintln!("Press Ctrl+C or send SIGTERM to stop");

    while running.load(Ordering::Relaxed) {
        let today = Local::now().date_naive();

        if today != current_date {
            if let Err(e) = rotate_archive(current_date) {
                eprintln!("Failed to rotate archive: {}", e);
            }
            current_date = today;
        }

        match read_battery_info(&battery_path) {
            Ok(info) => {
                if let Err(e) = write_csv_record(&info) {
                    eprintln!("Failed to write log: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to read battery info: {}", e);
            }
        }

        thread::sleep(Duration::from_secs(LOG_INTERVAL_SECS));
    }

    eprintln!("\nShutting down...");

    if let Err(e) = rotate_archive(current_date) {
        eprintln!("Failed to rotate archive on shutdown: {}", e);
    }

    remove_pid_file(&pid_path);
    eprintln!("Daemon stopped");

    Ok(())
}
