use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, GraphType, Paragraph},
    Frame,
};

use crate::app::App;

pub fn format_duration(secs: f64) -> String {
    let total_secs = secs as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;

    if hours > 0 {
        format!("{}h{:02}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([Constraint::Min(10), Constraint::Length(3)]).split(frame.area());

    draw_chart(frame, app, chunks[0]);
    draw_status_bar(frame, app, chunks[1]);

    if app.show_service_warning {
        draw_warning_popup(frame);
    }

    if app.show_about {
        draw_about_popup(frame);
    }
}

fn draw_chart(frame: &mut Frame, app: &App, area: Rect) {
    let chart_data = app.chart_data();

    let (time_min, time_max) = chart_data.time_range;
    let (_, power_max) = app.power_range();
    let y_max = 100.0_f64.max(power_max);

    let x_labels = chart_data.x_labels.clone();

    let sleep_lines: Vec<Vec<(f64, f64)>> = chart_data
        .sleep_markers
        .iter()
        .map(|(x, _)| (0..=20).map(|i| (*x, y_max * i as f64 / 20.0)).collect())
        .collect();

    let mut datasets: Vec<Dataset> = Vec::new();
    for (i, line_data) in sleep_lines.iter().enumerate() {
        let name = if i == 0 { "Sleep" } else { "" };
        datasets.push(
            Dataset::default()
                .name(name)
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Magenta))
                .data(line_data),
        );
    }

    datasets.push(
        Dataset::default()
            .name("Capacity (%)")
            .marker(Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&chart_data.capacity_data),
    );
    datasets.push(
        Dataset::default()
            .name("Power (W)")
            .marker(Marker::Braille)
            .style(Style::default().fg(Color::Yellow))
            .data(&chart_data.power_data),
    );

    let date_str = app.current_date.format("%Y-%m-%d").to_string();
    let today_marker = if app.is_today() { " (Live)" } else { "" };
    let title = format!(
        " Watt Monitor - {} [{}]{} ",
        date_str,
        app.view_mode_label(),
        today_marker
    );
    let chart = Chart::new(datasets)
        .block(Block::bordered().title(title))
        .x_axis(
            Axis::default()
                .title("Time")
                .style(Style::default().fg(Color::Gray))
                .bounds([time_min, time_max])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("Capacity(%)")
                .style(Style::default().fg(Color::Cyan))
                .bounds([0.0, y_max])
                .labels(vec!["0".cyan().bold(), "50".cyan(), "100".cyan().bold()]),
        );

    frame.render_widget(chart, area);

    let plot_top = area.y + 1;
    let plot_bottom = area.y + area.height - 3;
    let plot_height = plot_bottom.saturating_sub(plot_top);
    let right_x = area.x + area.width - 1;

    let power_labels = [
        (plot_bottom, "0W".to_string()),
        (
            plot_top + plot_height / 2,
            format!("{:.0}W", power_max / 2.0),
        ),
        (plot_top, format!("{:.0}W", power_max)),
    ];

    for (y_pos, label) in power_labels {
        let label_len = label.len() as u16;
        let label_x = right_x.saturating_sub(label_len);
        if label_x >= area.x && y_pos >= area.y && y_pos < area.y + area.height {
            let label_area = Rect::new(label_x, y_pos, label_len, 1);
            let label_widget = Paragraph::new(label).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );
            frame.render_widget(label_widget, label_area);
        }
    }

    let plot_left = area.x + 7;
    let plot_right = area.x + area.width - 2;
    let plot_width = plot_right.saturating_sub(plot_left) as f64;
    let label_y = area.y + area.height - 2;

    for (compressed_x, sp) in &chart_data.sleep_markers {
        let x_ratio = if time_max > time_min {
            (compressed_x - time_min) / (time_max - time_min)
        } else {
            0.0
        };
        let screen_x = plot_left + (plot_width * x_ratio) as u16;

        let wake_label = chrono::DateTime::from_timestamp(sp.end_time, 0)
            .map(|dt| dt.with_timezone(&chrono::Local))
            .map(|dt| format!("↑{}", dt.format("%H:%M")))
            .unwrap_or_default();

        let label_len = wake_label.len() as u16;
        let label_x = screen_x.saturating_sub(label_len / 2);

        if label_x >= area.x
            && label_x + label_len <= area.x + area.width
            && label_y < area.y + area.height
        {
            let label_area = Rect::new(label_x, label_y, label_len, 1);
            let label_widget =
                Paragraph::new(wake_label).style(Style::default().fg(Color::Magenta));
            frame.render_widget(label_widget, label_area);
        }
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let width = area.width as usize;

    let status = app.latest_status().unwrap_or("N/A");
    let status_span: Span = match status {
        "Charging" => status.green().bold(),
        "Discharging" => status.red().bold(),
        "Full" => status.cyan().bold(),
        _ => status.bold(),
    };

    let capacity = app
        .latest_capacity()
        .map(|c| format!("{:.1}%", c))
        .unwrap_or_else(|| "N/A".to_string());
    let power = app
        .latest_power()
        .map(|p| format!("{:.2}W", p))
        .unwrap_or_else(|| "N/A".to_string());

    let spans: Vec<Span> = if width >= 110 {
        // 전체 표시
        let mut s = vec![
            " Status: ".into(),
            status_span,
            " | Capacity: ".into(),
            capacity.cyan().bold(),
            " | Power: ".into(),
            power.yellow().bold(),
            " | ".into(),
            app.view_mode_label().green(),
        ];
        if let Some(sleep) = app.last_sleep_period() {
            let duration = format_duration(sleep.duration_secs as f64);
            let diff_str = if sleep.capacity_diff >= 0.0 {
                format!("+{:.1}%", sleep.capacity_diff)
            } else {
                format!("{:.1}%", sleep.capacity_diff)
            };
            let hours = sleep.duration_secs as f64 / 3600.0;
            let rate = if hours > 0.0 {
                sleep.capacity_diff / hours
            } else {
                0.0
            };
            let rate_str = if rate >= 0.0 {
                format!("+{:.1}%/h", rate)
            } else {
                format!("{:.1}%/h", rate)
            };
            s.extend(vec![
                " | Last Sleep: ".into(),
                duration.magenta(),
                " (".into(),
                diff_str.magenta().bold(),
                ", ".into(),
                rate_str.magenta(),
                ")".into(),
            ]);
        }
        s.push(" | ←→ Tab h q ".dark_gray());
        s
    } else if width >= 80 {
        // Sleep 축약
        let mut s = vec![
            " ".into(),
            status_span,
            " | ".into(),
            capacity.cyan().bold(),
            " | ".into(),
            power.yellow().bold(),
            " | ".into(),
            app.view_mode_label().green(),
        ];
        if let Some(sleep) = app.last_sleep_period() {
            let duration = format_duration(sleep.duration_secs as f64);
            let diff_str = if sleep.capacity_diff >= 0.0 {
                format!("+{:.1}%", sleep.capacity_diff)
            } else {
                format!("{:.1}%", sleep.capacity_diff)
            };
            s.extend(vec![
                " | Sleep: ".into(),
                duration.magenta(),
                " (".into(),
                diff_str.magenta().bold(),
                ")".into(),
            ]);
        }
        s.push(" | ←→ Tab h q ".dark_gray());
        s
    } else if width >= 60 {
        vec![
            " ".into(),
            status_span,
            " | ".into(),
            capacity.cyan().bold(),
            " | ".into(),
            power.yellow().bold(),
            " | ".into(),
            app.view_mode_label().green(),
            " | ←→ h q ".dark_gray(),
        ]
    } else {
        vec![
            " ".into(),
            status_span,
            " | ".into(),
            capacity.cyan().bold(),
            " | ".into(),
            power.yellow().bold(),
        ]
    };

    let status_line = Line::from(spans);
    let paragraph = Paragraph::new(status_line).block(Block::bordered());

    frame.render_widget(paragraph, area);
}

fn draw_warning_popup(frame: &mut Frame) {
    let area = frame.area();

    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 7;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(""),
        Line::from("Logger service is not running!".yellow().bold()),
        Line::from(""),
        Line::from("systemctl --user enable --now watt-monitor.target"),
        Line::from(""),
        Line::from("Press any key to dismiss".dark_gray()),
    ];

    let popup = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .title(" Warning ")
            .title_style(Style::default().fg(Color::Yellow).bold())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    frame.render_widget(popup, popup_area);
}

fn draw_about_popup(frame: &mut Frame) {
    let area = frame.area();

    let popup_width = 54.min(area.width.saturating_sub(4));
    let popup_height = 13;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            "W".cyan().bold(),
            "att ".cyan(),
            "M".cyan().bold(),
            "onitor".cyan(),
        ]),
        Line::from("v1.0.0"),
        Line::from("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dark_gray()),
        Line::from(""),
        Line::from(vec!["  Author  ".dark_gray()]),
        Line::from("Joo-Kwang Park".white().bold()),
        Line::from(""),
        Line::from("  GitHub  ".dark_gray()),
        Line::from(vec!["github.com/jookwang-park/watt-monitor-rs".blue()]),
        Line::from(""),
        Line::from("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".dark_gray()),
        Line::from(""),
        Line::from("Press any key to close".dark_gray()),
    ];

    let popup = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .title(" About ")
            .title_style(Style::default().fg(Color::Cyan).bold())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(popup, popup_area);
}
