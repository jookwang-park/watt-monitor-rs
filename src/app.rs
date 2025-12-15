use std::process::Command;

use chrono::{Local, NaiveDate};

use crate::data::{
    get_archive_path_for_date, get_csv_path_for_date, list_available_dates, parse_csv,
    parse_csv_from_line, BatteryRecord,
};

const SLEEP_THRESHOLD_SECS: i64 = 10 * 60;
const MAX_SLEEP_DRAIN_RATE_PER_HOUR: f64 = 5.0;

#[derive(Debug, Clone)]
pub struct SleepPeriod {
    pub start_time: i64,
    pub end_time: i64,
    pub duration_secs: i64,
    pub capacity_diff: f64,
}

pub struct ChartData {
    pub capacity_data: Vec<(f64, f64)>,
    pub power_data: Vec<(f64, f64)>,
    pub time_range: (f64, f64),
    pub sleep_markers: Vec<(f64, SleepPeriod)>,
    pub x_labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Recent30m,
    Recent1h,
    Recent4h,
    Recent12h,
    Full,
}

impl ViewMode {
    pub fn toggle(&self) -> Self {
        match self {
            ViewMode::Recent30m => ViewMode::Recent1h,
            ViewMode::Recent1h => ViewMode::Recent4h,
            ViewMode::Recent4h => ViewMode::Recent12h,
            ViewMode::Recent12h => ViewMode::Full,
            ViewMode::Full => ViewMode::Recent30m,
        }
    }

    pub fn expand(&self) -> Option<Self> {
        match self {
            ViewMode::Recent30m => Some(ViewMode::Recent1h),
            ViewMode::Recent1h => Some(ViewMode::Recent4h),
            ViewMode::Recent4h => Some(ViewMode::Recent12h),
            ViewMode::Recent12h => Some(ViewMode::Full),
            ViewMode::Full => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ViewMode::Recent30m => "30m",
            ViewMode::Recent1h => "1h",
            ViewMode::Recent4h => "4h",
            ViewMode::Recent12h => "12h",
            ViewMode::Full => "Full",
        }
    }

    pub fn window_secs(&self) -> Option<i64> {
        match self {
            ViewMode::Recent30m => Some(30 * 60),
            ViewMode::Recent1h => Some(60 * 60),
            ViewMode::Recent4h => Some(4 * 60 * 60),
            ViewMode::Recent12h => Some(12 * 60 * 60),
            ViewMode::Full => None,
        }
    }

    pub fn min_data_secs(&self) -> i64 {
        match self {
            ViewMode::Recent30m => 5 * 60,
            ViewMode::Recent1h => 10 * 60,
            ViewMode::Recent4h => 30 * 60,
            ViewMode::Recent12h => 60 * 60,
            ViewMode::Full => 0,
        }
    }
}

pub struct App {
    pub records: Vec<BatteryRecord>,
    pub current_date: NaiveDate,
    pub available_dates: Vec<NaiveDate>,
    pub last_read_count: usize,
    pub should_quit: bool,
    pub view_mode: ViewMode,
    pub show_service_warning: bool,
    pub show_about: bool,
}

impl App {
    pub fn new(initial_date: NaiveDate, available_dates: Vec<NaiveDate>) -> Self {
        let records = Self::load_records_for_date(initial_date);
        let last_read_count = records.len();
        let show_service_warning = !Self::is_logger_service_active();

        App {
            records,
            current_date: initial_date,
            available_dates,
            last_read_count,
            should_quit: false,
            view_mode: ViewMode::Recent30m,
            show_service_warning,
            show_about: false,
        }
    }

    fn is_logger_service_active() -> bool {
        Command::new("systemctl")
            .args(["--user", "is-active", "--quiet", "watt-monitor.service"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    pub fn dismiss_warning(&mut self) {
        self.show_service_warning = false;
    }

    pub fn toggle_about(&mut self) {
        self.show_about = !self.show_about;
    }

    pub fn dismiss_about(&mut self) {
        self.show_about = false;
    }

    fn load_records_for_date(date: NaiveDate) -> Vec<BatteryRecord> {
        let today = Local::now().date_naive();
        let mut records = Vec::new();

        if date == today {
            if let Some(yesterday) = date.pred_opt() {
                let yesterday_path = get_archive_path_for_date(yesterday);
                if yesterday_path.exists() {
                    if let Ok(yesterday_records) = parse_csv(&yesterday_path) {
                        records.extend(yesterday_records);
                    }
                }
            }
            let today_path = get_csv_path_for_date(date);
            if let Ok(today_records) = parse_csv(&today_path) {
                records.extend(today_records);
            }
        } else {
            let csv_path = get_csv_path_for_date(date);
            records = parse_csv(&csv_path).unwrap_or_default();
        }

        records
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = self.view_mode.toggle();
    }

    pub fn is_today(&self) -> bool {
        self.current_date == Local::now().date_naive()
    }

    pub fn navigate_date(&mut self, delta: i32) {
        if let Some(current_idx) = self
            .available_dates
            .iter()
            .position(|d| *d == self.current_date)
        {
            let new_idx = if delta > 0 {
                current_idx.saturating_sub(delta as usize)
            } else {
                (current_idx + (-delta) as usize).min(self.available_dates.len() - 1)
            };

            if new_idx != current_idx {
                self.current_date = self.available_dates[new_idx];
                self.load_date_data();
            }
        }
    }

    fn load_date_data(&mut self) {
        self.records = Self::load_records_for_date(self.current_date);
        self.last_read_count = self.records.len();
    }

    pub fn refresh_data(&mut self) {
        // Only refresh if viewing today's data
        if !self.is_today() {
            return;
        }

        self.available_dates = list_available_dates();

        let csv_path = get_csv_path_for_date(self.current_date);
        if let Ok(new_records) = parse_csv_from_line(&csv_path, self.last_read_count) {
            if !new_records.is_empty() {
                self.records.extend(new_records);
                self.last_read_count = self.records.len();
            }
        }
    }

    fn filtered_records_for_mode(&self, mode: ViewMode) -> Vec<&BatteryRecord> {
        if self.records.is_empty() {
            return vec![];
        }

        match mode.window_secs() {
            Some(window_secs) => {
                let latest_time = self.records.last().unwrap().time.timestamp();
                let cutoff = latest_time - window_secs;
                self.records
                    .iter()
                    .filter(|r| r.time.timestamp() >= cutoff)
                    .collect()
            }
            None => {
                let max_points = 500;
                if self.records.len() <= max_points {
                    self.records.iter().collect()
                } else {
                    let step = self.records.len() / max_points;
                    self.records
                        .iter()
                        .step_by(step)
                        .chain(std::iter::once(self.records.last().unwrap()))
                        .collect()
                }
            }
        }
    }

    pub fn effective_view_mode(&self) -> ViewMode {
        if self.records.is_empty() {
            return self.view_mode;
        }

        let mut current_mode = self.view_mode;

        loop {
            let filtered = self.filtered_records_for_mode(current_mode);
            if filtered.len() < 2 {
                if let Some(expanded) = current_mode.expand() {
                    current_mode = expanded;
                    continue;
                }
            } else {
                let first_time = filtered.first().unwrap().time.timestamp();
                let last_time = filtered.last().unwrap().time.timestamp();
                let data_duration = last_time - first_time;

                if data_duration < current_mode.min_data_secs() {
                    if let Some(expanded) = current_mode.expand() {
                        current_mode = expanded;
                        continue;
                    }
                }
            }
            break;
        }

        current_mode
    }

    pub fn view_mode_label(&self) -> String {
        let effective = self.effective_view_mode();
        if effective == self.view_mode {
            self.view_mode.label().to_string()
        } else {
            format!("{}→{}", self.view_mode.label(), effective.label())
        }
    }

    fn filtered_records(&self) -> Vec<&BatteryRecord> {
        self.filtered_records_for_mode(self.effective_view_mode())
    }

    pub fn latest_capacity(&self) -> Option<f64> {
        self.records.last().map(|r| r.capacity)
    }

    pub fn latest_power(&self) -> Option<f64> {
        self.records.last().map(|r| r.power)
    }

    pub fn latest_status(&self) -> Option<&str> {
        self.records.last().map(|r| r.status.as_str())
    }

    pub fn power_range(&self) -> (f64, f64) {
        let filtered = self.filtered_records();
        if filtered.is_empty() {
            return (0.0, 20.0);
        }

        let min = filtered
            .iter()
            .map(|r| r.power)
            .fold(f64::INFINITY, f64::min);
        let max = filtered
            .iter()
            .map(|r| r.power)
            .fold(f64::NEG_INFINITY, f64::max);

        let padding = (max - min) * 0.1;
        ((min - padding).max(0.0), max + padding)
    }

    pub fn detect_sleep_periods(&self) -> Vec<SleepPeriod> {
        if self.records.len() < 2 {
            return vec![];
        }

        let mut sleep_periods = Vec::new();

        for i in 1..self.records.len() {
            let prev = &self.records[i - 1];
            let curr = &self.records[i];

            let time_diff = curr.time.timestamp() - prev.time.timestamp();

            if time_diff < SLEEP_THRESHOLD_SECS {
                continue;
            }

            let capacity_drop = prev.capacity - curr.capacity;
            let hours = time_diff as f64 / 3600.0;
            let drain_rate = if hours > 0.0 {
                capacity_drop / hours
            } else {
                0.0
            };

            if drain_rate > MAX_SLEEP_DRAIN_RATE_PER_HOUR {
                continue;
            }

            sleep_periods.push(SleepPeriod {
                start_time: prev.time.timestamp(),
                end_time: curr.time.timestamp(),
                duration_secs: time_diff,
                capacity_diff: curr.capacity - prev.capacity,
            });
        }

        sleep_periods
    }

    pub fn last_sleep_period(&self) -> Option<SleepPeriod> {
        self.detect_sleep_periods().into_iter().last()
    }

    fn total_sleep_before(timestamp: i64, base_time: i64, sleep_periods: &[SleepPeriod]) -> i64 {
        sleep_periods
            .iter()
            .filter(|sp| sp.end_time <= timestamp)
            .map(|sp| {
                let effective_start = sp.start_time.max(base_time);
                let effective_end = sp.end_time;
                (effective_end - effective_start).max(0)
            })
            .sum()
    }

    fn to_compressed_x(timestamp: i64, base_time: i64, sleep_periods: &[SleepPeriod]) -> f64 {
        let elapsed = timestamp - base_time;
        let sleep_duration = Self::total_sleep_before(timestamp, base_time, sleep_periods);
        (elapsed - sleep_duration) as f64
    }

    fn find_record_at_compressed_x<'a>(
        compressed_x: f64,
        filtered: &[&'a BatteryRecord],
        base_time: i64,
        sleep_periods: &[SleepPeriod],
    ) -> Option<&'a BatteryRecord> {
        filtered
            .iter()
            .min_by(|a, b| {
                let ax = Self::to_compressed_x(a.time.timestamp(), base_time, sleep_periods);
                let bx = Self::to_compressed_x(b.time.timestamp(), base_time, sleep_periods);
                let da = (ax - compressed_x).abs();
                let db = (bx - compressed_x).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }

    pub fn chart_data(&self) -> ChartData {
        let filtered = self.filtered_records();
        if filtered.is_empty() {
            return ChartData {
                capacity_data: vec![],
                power_data: vec![],
                time_range: (0.0, 60.0),
                sleep_markers: vec![],
                x_labels: vec!["".to_string(), "".to_string(), "".to_string()],
            };
        }

        let base_time = filtered[0].time.timestamp();
        let view_start = base_time;
        let view_end = filtered.last().unwrap().time.timestamp();

        let sleep_in_view: Vec<SleepPeriod> = self
            .detect_sleep_periods()
            .into_iter()
            .filter(|sp| sp.end_time >= view_start && sp.start_time <= view_end)
            .collect();

        let capacity_data: Vec<(f64, f64)> = filtered
            .iter()
            .map(|r| {
                let x = Self::to_compressed_x(r.time.timestamp(), base_time, &sleep_in_view);
                (x, r.capacity)
            })
            .collect();

        let power_data: Vec<(f64, f64)> = filtered
            .iter()
            .map(|r| {
                let x = Self::to_compressed_x(r.time.timestamp(), base_time, &sleep_in_view);
                (x, r.power)
            })
            .collect();

        let total_sleep: i64 = sleep_in_view
            .iter()
            .map(|sp| {
                let effective_start = sp.start_time.max(base_time);
                let effective_end = sp.end_time.min(view_end);
                (effective_end - effective_start).max(0)
            })
            .sum();
        let real_duration = view_end - view_start;
        let compressed_duration = (real_duration - total_sleep) as f64;
        let time_range = (0.0, compressed_duration.max(60.0));

        let sleep_markers: Vec<(f64, SleepPeriod)> = sleep_in_view
            .iter()
            .map(|sp| {
                let x = Self::to_compressed_x(sp.end_time, base_time, &sleep_in_view);
                (x, sp.clone())
            })
            .collect();

        let start_label = filtered.first().unwrap().time.format("%H:%M").to_string();
        let end_label = filtered.last().unwrap().time.format("%H:%M").to_string();
        let mid_compressed = compressed_duration / 2.0;
        let mid_label =
            Self::find_record_at_compressed_x(mid_compressed, &filtered, base_time, &sleep_in_view)
                .map(|r| r.time.format("%H:%M").to_string())
                .unwrap_or_else(|| "".to_string());
        let x_labels = vec![start_label, mid_label, end_label];

        ChartData {
            capacity_data,
            power_data,
            time_range,
            sleep_markers,
            x_labels,
        }
    }
}
