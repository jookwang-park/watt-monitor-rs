mod app;
mod daemon;
mod data;
mod ui;

use std::{io, time::Duration};

use chrono::{Local, NaiveDate};
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;

use app::App;
use data::{list_available_dates, parse_date_arg};

#[derive(Parser)]
#[command(name = "watt-monitor")]
#[command(about = "Battery power monitor with TUI and daemon modes")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    date: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    Daemon,
    List,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Daemon) => daemon::run(),
        Some(Commands::List) => {
            print_available_dates();
            Ok(())
        }
        None => run_tui(cli.date),
    }
}

fn print_available_dates() {
    let dates = list_available_dates();
    if dates.is_empty() {
        println!("No data files found in {:?}", data::get_data_dir());
        println!("Start the daemon: watt-monitor daemon");
        println!("Or enable systemd service: systemctl --user enable --now watt-monitor.service");
    } else {
        println!("Available dates:");
        for date in dates {
            println!("  {}", date.format("%Y-%m-%d"));
        }
    }
}

fn run_tui(date_arg: Option<String>) -> io::Result<()> {
    let target_date: NaiveDate = if let Some(ref date_str) = date_arg {
        parse_date_arg(date_str).unwrap_or_else(|| {
            eprintln!(
                "Invalid date format: {}. Use YYYY-MM-DD, 'today', or 'yesterday'",
                date_str
            );
            std::process::exit(1);
        })
    } else {
        Local::now().date_naive()
    };

    let available_dates = list_available_dates();

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, target_date, available_dates);
    ratatui::restore();

    result
}

fn run(
    terminal: &mut DefaultTerminal,
    initial_date: NaiveDate,
    available_dates: Vec<NaiveDate>,
) -> io::Result<()> {
    let mut app = App::new(initial_date, available_dates);
    let tick_rate = Duration::from_millis(500);

    loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.show_service_warning {
                        app.dismiss_warning();
                        continue;
                    }

                    if app.show_about {
                        app.dismiss_about();
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        KeyCode::Tab => {
                            app.toggle_view_mode();
                        }
                        KeyCode::Left => {
                            app.navigate_date(-1);
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            app.navigate_date(1);
                        }
                        KeyCode::Char('h') => {
                            app.toggle_about();
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }

        app.refresh_data();
    }

    Ok(())
}
