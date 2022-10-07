use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{bail, ensure, Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use stats::Stats;
use time::{Duration, Instant};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Span, Spans},
    widgets::{
        Axis, Block, BorderType, Borders, Chart, Clear, Dataset, Gauge, GraphType, Paragraph, Tabs,
    },
    Terminal,
};

use crate::{
    ffmpeg::{Progress, ProgressIter},
    ffprobe::Format,
    values::{ChartValues, SparklineValues},
};

mod array;
mod ffmpeg;
mod ffprobe;
mod stats;
mod values;

/// Visualizer for the FFmpeg encoding process.
#[derive(Parser)]
#[command(about, author, version, arg_required_else_help(true))]
struct Args {
    /// Same input media file that is used in the FFmpeg arguments.
    #[arg(short, long)]
    input: PathBuf,
    /// Overwrite the output file if it already exists.
    #[arg(short = 'y', long)]
    overwrite: bool,
    /// Only load the statistics and display them, skipping any encoding.
    #[arg(short = 's', long)]
    load_stats: bool,
    /// Show the statistics screen after the encoding is done.
    #[arg(long)]
    show_stats: bool,
    /// Save the statistics to a file, so they can be loaded afterwards.
    #[arg(long)]
    save_stats: bool,
    /// Arguments to pass to FFmpeg.
    #[arg(raw = true)]
    args: Vec<String>,
    #[command(subcommand)]
    cmd: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate auto-completion scripts for various shells.
    Completions {
        /// Shell to generate an auto-completion script for.
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Generate man pages into the given directory.
    Manpages {
        /// Target directory, that must already exist and be empty. If the any file with the same
        /// name as any of the man pages already exist, it'll not be overwritten, but instead an
        /// error be returned.
        #[arg(value_hint = ValueHint::DirPath)]
        dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(cmd) = args.cmd {
        match cmd {
            Command::Completions { shell } => completions(shell),
            Command::Manpages { dir } => manpages(&dir)?,
        }

        Ok(())
    } else {
        let mut terminal = create_terminal()?;

        // Don't exit with an error here, first restore the terminal to normal mode and
        // then fail with the error.
        let result = run(&mut terminal, &args);

        // Ignore any errors while restoring the terminal. If we fail, there is no way of getting
        // back to normal mode. Therefore, we skip this error and return the result from the
        // main execution instead.
        destroy_terminal(terminal).ok();

        result
    }
}

fn run(terminal: &mut Terminal<impl Backend + Write>, args: &Args) -> Result<()> {
    let stats = if args.load_stats {
        stats::load(&args.input)?
    } else {
        let ffprobe = ffprobe::run(&args.input)?;
        let ffmpeg = ffmpeg::spawn(&args.args, args.overwrite)?;

        let result = show_progress(terminal, &ffprobe, ffmpeg);

        let history = result?;
        let stats = Stats {
            import: ffprobe,
            history,
        };

        if args.save_stats {
            stats::save(&stats, &args.input)?;
        }

        stats
    };

    if args.load_stats || args.show_stats {
        show_stats(terminal, stats)?;
    }

    Ok(())
}

fn create_terminal() -> Result<Terminal<impl Backend + Write>> {
    terminal::enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);

    Terminal::new(backend).map_err(Into::into)
}

fn destroy_terminal(mut terminal: Terminal<impl Backend + Write>) -> Result<()> {
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal::disable_raw_mode()?;
    terminal.show_cursor()?;

    Ok(())
}

fn show_progress(
    terminal: &mut Terminal<impl Backend>,
    ffprobe: &Format,
    mut ffmpeg: ProgressIter,
) -> Result<Vec<(Duration, Progress)>> {
    let mut progress = Progress::default();
    let mut history = Vec::new();
    let mut fps = SparklineValues::new(|v| format!("FPS: {v:.1}"));
    let mut speed = SparklineValues::new(|v| format!("Speed: {v:.2}x"));
    let mut bitrate = ChartValues::new(ffprobe.bit_rate as f64, |v| {
        format!("Bitrate: {:.1} kbits/s", v / 1000.0)
    });
    let start_time = Instant::now();
    let mut timestamp = Duration::ZERO;

    terminal.draw(|f| f.render_widget(Clear, f.size()))?;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(5), Constraint::Percentage(100)])
                .split(f.size());

            let lr = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            let left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Percentage(100),
                ])
                .split(lr[0]);

            let left_r1 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Ratio(1, 3); 3])
                .split(left[0]);

            f.render_widget(
                Gauge::default()
                    .block(
                        Block::default()
                            .title(Span::styled(
                                format!(
                                    "Progress / Run-time: {} / Out-time: {}",
                                    format_duration(timestamp),
                                    format_duration(progress.out_time)
                                ),
                                Style::default().fg(Color::Blue),
                            ))
                            .title_alignment(Alignment::Center)
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded),
                    )
                    .gauge_style(Style::default().fg(Color::White).bg(Color::Black))
                    .ratio(
                        (progress.out_time.as_seconds_f64() / ffprobe.duration.as_seconds_f64())
                            .max(0.0)
                            .min(1.0),
                    ),
                chunks[0],
            );

            f.render_widget(
                Paragraph::new(progress.frame.to_string()).block(
                    Block::default()
                        .title(Span::styled("Frame", Style::default().fg(Color::Blue)))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                ),
                left_r1[0],
            );
            f.render_widget(
                Paragraph::new({
                    if progress.total_size > 1_000_000_000 {
                        format!("{:.2} GiB", progress.total_size as f64 / 1_000_000_000.0)
                    } else if progress.total_size > 1_000_000 {
                        format!("{:.2} MiB", progress.total_size as f64 / 1_000_000.0)
                    } else if progress.total_size > 1_000 {
                        format!("{:.2} KiB", progress.total_size as f64 / 1_000.0)
                    } else {
                        format!("{} B", progress.total_size)
                    }
                })
                .block(
                    Block::default()
                        .title(Span::styled("Total size", Style::default().fg(Color::Blue)))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                ),
                left_r1[1],
            );
            f.render_widget(
                Paragraph::new(format!(
                    "{} / {}",
                    progress.dup_frames, progress.drop_frames
                ))
                .block(
                    Block::default()
                        .title(Span::styled(
                            "Dup / Drop frames",
                            Style::default().fg(Color::Blue),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                ),
                left_r1[2],
            );

            f.render_widget(fps.create(left[1]), left[1]);
            f.render_widget(speed.create(left[2]), left[2]);

            f.render_widget(bitrate.create(), lr[1]);
        })?;

        while event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(event) = event::read()? {
                match event.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        bail!("encoding cancelled by user");
                    }
                    KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                        bail!("encoding cancelled by user");
                    }
                    _ => {}
                }
            }
        }

        match ffmpeg.next() {
            Some(res) => {
                progress = res?;
                timestamp = start_time.elapsed();
                history.push((timestamp, progress.clone()));
            }
            None => return Ok(history),
        }

        fps.update(progress.fps);
        bitrate.update(progress.bitrate as f64);
        speed.update(progress.speed);
    }
}

fn show_stats(terminal: &mut Terminal<impl Backend>, stats: Stats) -> Result<()> {
    let titles = ["Bitrate", "FPS", "Speed"]
        .into_iter()
        .map(Spans::from)
        .collect::<Vec<_>>();
    let mut selection = 0;

    let bitrate_stats = BitrateStats::new(
        stats.import.bit_rate as f64,
        stats
            .history
            .iter()
            .map(|(d, p)| (d.as_seconds_f64(), p.bitrate as f64)),
    );
    let fps_stats = OneLineStats::new(
        stats
            .history
            .iter()
            .map(|(d, p)| (d.as_seconds_f64(), p.fps)),
        |fps| format!("{fps:.1}"),
    );
    let speed_stats = OneLineStats::new(
        stats
            .history
            .iter()
            .map(|(d, p)| (d.as_seconds_f64(), p.speed)),
        |speed| format!("{speed:.2}x"),
    );

    terminal.draw(|f| f.render_widget(Clear, f.size()))?;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Percentage(100)])
                .split(f.size());

            let tabs = Tabs::new(titles.clone())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .style(Style::default().fg(Color::White))
                .highlight_style(
                    Style::default()
                        .fg(Color::Green)
                        .bg(Color::Black)
                        .add_modifier(Modifier::UNDERLINED),
                )
                .divider("|")
                .select(selection);

            let chart = match selection {
                0 => bitrate_stats.create(),
                1 => fps_stats.create(),
                2 => speed_stats.create(),
                _ => unreachable!(),
            };

            f.render_widget(tabs, chunks[0]);
            f.render_widget(chart, chunks[1]);
        })?;

        if let Event::Key(event) = event::read()? {
            match event.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(())
                }
                KeyCode::Left => selection = selection.saturating_sub(1),
                KeyCode::Right => selection = 2.min(selection + 1),
                _ => {}
            }
        }
    }
}

struct BitrateStats {
    baseline_data: Vec<(f64, f64)>,
    bitrate_data: Vec<(f64, f64)>,
    x_max: f64,
    x_labels: Vec<Span<'static>>,
    y_min: f64,
    y_max: f64,
    y_labels: Vec<Span<'static>>,
}

impl BitrateStats {
    pub fn new(baseline: f64, history: impl Iterator<Item = (f64, f64)>) -> Self {
        let mut x_max = 0.0_f64;
        let mut y_min = f64::MAX;
        let mut y_max = 0.0_f64;

        let bitrate_data = history
            .inspect(|(duration, bitrate)| {
                x_max = x_max.max(*duration);
                y_min = y_min.min(*bitrate);
                y_max = y_max.max(*bitrate);
            })
            .collect();

        let baseline_data = vec![(0.0, baseline), (x_max, baseline)];

        let x_labels = [0.0, x_max * 0.25, x_max * 0.50, x_max * 0.75, x_max]
            .into_iter()
            .map(|label| {
                let d = Duration::seconds_f64(label);
                Span::from(format_duration(d))
            })
            .collect();

        let y_min = y_min.min(baseline * 0.9).max(0.0);
        let y_max = y_max.max(baseline * 1.1);
        let y_diff = y_max - y_min;
        let y_labels = [
            y_min,
            y_min + y_diff * 0.25,
            y_min + y_diff * 0.50,
            y_min + y_diff * 0.75,
            y_max,
        ]
        .into_iter()
        .map(|label| Span::from(format!("{:.1} kbits/s", label / 1000.0)))
        .collect();

        Self {
            baseline_data,
            bitrate_data,
            x_max,
            x_labels,
            y_min,
            y_max,
            y_labels,
        }
    }

    pub fn create(&self) -> Chart<'_> {
        let baseline = Dataset::default()
            .marker(Marker::Block)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Red))
            .data(&self.baseline_data);

        let dataset = Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Blue))
            .data(&self.bitrate_data);

        Chart::new(vec![baseline, dataset])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .x_axis(
                Axis::default()
                    .bounds([0.0, self.x_max])
                    .labels(self.x_labels.clone())
                    .labels_alignment(Alignment::Center),
            )
            .y_axis(
                Axis::default()
                    .bounds([self.y_min, self.y_max])
                    .labels(self.y_labels.clone())
                    .labels_alignment(Alignment::Right),
            )
    }
}

struct OneLineStats {
    data: Vec<(f64, f64)>,
    x_max: f64,
    x_labels: Vec<Span<'static>>,
    y_min: f64,
    y_max: f64,
    y_labels: Vec<Span<'static>>,
}

impl OneLineStats {
    pub fn new<F>(history: impl Iterator<Item = (f64, f64)>, labeler: F) -> Self
    where
        F: Fn(f64) -> String,
    {
        let mut x_max = 0.0_f64;
        let mut y_min = f64::MAX;
        let mut y_max = 0.0_f64;

        let data = history
            .inspect(|(duration, value)| {
                x_max = x_max.max(*duration);
                y_min = y_min.min(*value);
                y_max = y_max.max(*value);
            })
            .collect();

        let x_labels = [0.0, x_max * 0.25, x_max * 0.50, x_max * 0.75, x_max]
            .into_iter()
            .map(|label| {
                let d = Duration::seconds_f64(label);
                Span::from(format_duration(d))
            })
            .collect();

        let y_diff = y_max - y_min;
        let y_labels = [
            y_min,
            y_min + y_diff * 0.25,
            y_min + y_diff * 0.50,
            y_min + y_diff * 0.75,
            y_max,
        ]
        .into_iter()
        .map(|label| Span::from(labeler(label)))
        .collect::<Vec<_>>();

        Self {
            data,
            x_max,
            x_labels,
            y_min,
            y_max,
            y_labels,
        }
    }

    pub fn create(&self) -> Chart<'_> {
        let dataset = Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Blue))
            .data(&self.data);

        Chart::new(vec![dataset])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .x_axis(
                Axis::default()
                    .bounds([0.0, self.x_max])
                    .labels(self.x_labels.clone())
                    .labels_alignment(Alignment::Center),
            )
            .y_axis(
                Axis::default()
                    .bounds([self.y_min, self.y_max])
                    .labels(self.y_labels.clone())
                    .labels_alignment(Alignment::Right),
            )
    }
}

fn format_duration(d: Duration) -> String {
    let d = d.whole_seconds().abs();
    format!("{:02}:{:02}:{:02}", d / 3600, d / 60 % 60, d % 60)
}

/// Generate shell completions, written to the standard output.
#[allow(clippy::unnecessary_wraps)]
pub fn completions(shell: Shell) {
    clap_complete::generate(
        shell,
        &mut Args::command(),
        env!("CARGO_PKG_NAME"),
        &mut io::stdout().lock(),
    );
}

/// Generate man pages in the target directory. The directory must already exist and none of the
/// files exist, or an error is returned.
pub fn manpages(dir: &Path) -> Result<()> {
    fn print(dir: &Path, app: &clap::Command) -> Result<()> {
        let name = app.get_display_name().unwrap_or_else(|| app.get_name());
        let out = dir.join(format!("{name}.1"));
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out)
            .with_context(|| format!("the file `{}` already exists", out.display()))?;

        clap_mangen::Man::new(app.clone()).render(&mut out)?;
        out.flush()?;

        for sub in app.get_subcommands() {
            print(dir, sub)?;
        }

        Ok(())
    }

    ensure!(dir.try_exists()?, "target directory doesn't exist");

    let mut app = Args::command();
    app.build();

    print(dir, &app)
}

#[cfg(test)]
mod tests {
    use super::Args;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
