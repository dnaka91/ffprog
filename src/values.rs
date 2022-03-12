use tui::{
    layout::Rect,
    style::{Color, Style},
    symbols::Marker,
    text::Span,
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Sparkline},
};

use crate::array::Array;

pub struct SparklineValues<F> {
    history: Array<u64, 500>,
    max: u64,
    current: f64,
    labeler: F,
}

impl<F> SparklineValues<F>
where
    F: Fn(f64) -> String,
{
    pub fn new(labeler: F) -> Self {
        Self {
            history: Array::new(0),
            max: 0,
            current: 0.0,
            labeler,
        }
    }

    pub fn create(&self, area: Rect) -> Sparkline {
        let data = self.history.as_slice();
        let data = &data[data
            .len()
            .saturating_sub(area.width.saturating_sub(2) as usize)..];

        Sparkline::default()
            .block(
                Block::default()
                    .title(Span::styled(
                        (self.labeler)(self.current),
                        Style::default().fg(Color::Blue),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .style(Style::default().fg(Color::Yellow))
            .data(data)
            .max(self.max)
    }

    pub fn update(&mut self, value: f64) {
        self.current = value;

        let value = (value * 100.0).round() as u64;
        self.history.push(value);
        self.max = self.max.max(value);
    }
}

pub struct ChartValues<F> {
    history: Array<(f64, f64), 1000>,
    baseline: [(f64, f64); 2],
    current: f64,
    min: f64,
    max: f64,
    labeler: F,
}

impl<F> ChartValues<F>
where
    F: Fn(f64) -> String,
{
    pub fn new(baseline: f64, labeler: F) -> Self {
        Self {
            history: Array::default(),
            baseline: [(0.0, baseline); 2],
            current: 0.0,
            min: 0.0,
            max: 0.0,
            labeler,
        }
    }

    pub fn create(&self) -> Chart<'_> {
        let baseline = Dataset::default()
            .marker(Marker::Block)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Red))
            .data(&self.baseline);
        let history = Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(self.history.as_slice());

        let y_min = self.min.min(self.baseline[0].1 * 0.9).max(0.0);
        let y_max = self.max.max(self.baseline[0].1 * 1.1);

        Chart::new(vec![baseline, history])
            .block(
                Block::default()
                    .title(Span::styled(
                        (self.labeler)(self.current),
                        Style::default().fg(Color::Blue),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .x_axis(Axis::default().bounds([self.baseline[0].0, self.baseline[1].0]))
            .y_axis(
                Axis::default()
                    .style(Style::default().fg(Color::White))
                    .bounds([y_min, y_max])
                    .labels(
                        [self.min, self.min + (y_max - self.min) / 2.0, y_max]
                            .into_iter()
                            .map(|value| Span::from(format!("{:.1}", value / 1000.0)))
                            .collect(),
                    ),
            )
    }

    pub fn update(&mut self, value: f64) {
        self.current = value;

        self.history.push((self.history.last().0 + 1.0, value));
        self.baseline[0].0 = self.history.first().0;
        self.baseline[1].0 = self.history.last().0;

        self.min = f64::MAX;
        self.max = 0.0f64;

        for (_, v) in self.history.as_slice().iter().copied() {
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        }
    }
}
