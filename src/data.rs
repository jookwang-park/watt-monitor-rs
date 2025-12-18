use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};
use serde::Deserialize;
use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BatteryRecord {
    pub time: DateTime<Local>,
    pub status: String,
    pub capacity: f64,
    pub power: f64,
}

#[derive(Debug, Deserialize)]
struct CsvRecord {
    #[serde(rename = "Time")]
    time: String,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "Capacity(%)")]
    capacity: f64,
    #[serde(rename = "Power(W)")]
    power: f64,
}

impl TryFrom<CsvRecord> for BatteryRecord {
    type Error = chrono::ParseError;

    fn try_from(csv: CsvRecord) -> Result<Self, Self::Error> {
        let naive = NaiveDateTime::parse_from_str(&csv.time, "%Y-%m-%d %H:%M:%S")?;
        let time = naive.and_local_timezone(Local).unwrap();

        Ok(BatteryRecord {
            time,
            status: csv.status,
            capacity: csv.capacity,
            power: csv.power,
        })
    }
}

pub fn parse_csv<P: AsRef<Path>>(path: P) -> Result<Vec<BatteryRecord>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut reader = csv::Reader::from_reader(file);
    let mut records = Vec::new();

    for result in reader.deserialize() {
        let csv_record: CsvRecord = match result {
            Ok(r) => r,
            Err(_) => continue,
        };
        if let Ok(record) = BatteryRecord::try_from(csv_record) {
            records.push(record);
        }
    }

    Ok(records)
}

pub fn parse_csv_from_line<P: AsRef<Path>>(
    path: P,
    skip_lines: usize,
) -> Result<Vec<BatteryRecord>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut reader = csv::Reader::from_reader(file);
    let mut records = Vec::new();

    for (i, result) in reader.deserialize().enumerate() {
        if i < skip_lines {
            continue;
        }

        let csv_record: CsvRecord = match result {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Ok(record) = BatteryRecord::try_from(csv_record) {
            records.push(record);
        }
    }

    Ok(records)
}

pub fn get_data_dir() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            PathBuf::from(home).join(".local/share")
        });
    base.join("watt-monitor")
}

pub fn get_today_log_path() -> PathBuf {
    PathBuf::from("/tmp/battery_watt_history.csv")
}

pub fn get_archive_path_for_date(date: NaiveDate) -> PathBuf {
    get_data_dir().join(format!("{}.csv", date.format("%Y-%m-%d")))
}

pub fn get_csv_path_for_date(date: NaiveDate) -> PathBuf {
    let today = Local::now().date_naive();
    if date == today {
        get_today_log_path()
    } else {
        get_archive_path_for_date(date)
    }
}

pub fn list_available_dates() -> Vec<NaiveDate> {
    let data_dir = get_data_dir();
    let mut dates = Vec::new();
    let today = Local::now().date_naive();

    if get_today_log_path().exists() {
        dates.push(today);
    }

    if let Ok(entries) = fs::read_dir(&data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "csv").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                        // Don't duplicate today
                        if date != today {
                            dates.push(date);
                        }
                    }
                }
            }
        }
    }

    dates.sort_by(|a, b| b.cmp(a));
    dates
}

pub fn parse_date_arg(arg: &str) -> Option<NaiveDate> {
    let today = Local::now().date_naive();
    match arg.to_lowercase().as_str() {
        "today" => Some(today),
        "yesterday" => today.pred_opt(),
        _ => NaiveDate::parse_from_str(arg, "%Y-%m-%d").ok(),
    }
}
