use crate::style::thousand_separator;
use crate::Message;
use iced::alignment;
use iced::mouse::Cursor;
use iced::widget::canvas;
use iced::widget::canvas::Geometry;
use iced::widget::text;
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};
use std::cell::RefCell;

#[derive(Clone, PartialEq)]
pub struct BarData {
    pub title: String,
    pub value: i64,
    pub color: Color,
}

pub struct BarChart {
    pub data: Vec<BarData>,
}

/// Kept by iced across frames (`canvas::Program::State`)
#[derive(Default)]
pub struct ChartState {
    cache: canvas::Cache,
    drawn: RefCell<Vec<BarData>>,
    // need to invalidate chache on theme change
    drawn_theme: RefCell<Option<Theme>>,
}

///
/// Using canvas to draw a bar chat
///
impl canvas::Program<Message> for BarChart {
    type State = ChartState;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry<Renderer>> {
        if self.data.is_empty() {
            return vec![];
        }

        // avoid unnecessary recalculations: redraw when the data or the theme changed
        // (the theme's text/axis colors are baked into the cached geometry)
        if *state.drawn.borrow() != self.data || state.drawn_theme.borrow().as_ref() != Some(theme)
        {
            state.cache.clear();
            state.drawn.borrow_mut().clone_from(&self.data);
            *state.drawn_theme.borrow_mut() = Some(theme.clone());
        }

        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            // .max(1) to avoid dividing by zero
            let max = self.data.iter().map(|d| d.value).fold(0, i64::max).max(1);
            let bar_space = frame.width() / self.data.len() as f32;

            let bar_width = bar_space * 0.75;
            let gap = (bar_space - bar_width) / 2.0;

            let top_pad = 50.0;
            let bottom_pad = 25.0;
            // .max(1) to always be positive
            let plot_h = (frame.height() - top_pad - bottom_pad).max(1.0);
            let baseline = top_pad + plot_h;

            for (i, d) in self.data.iter().enumerate() {
                let h = (d.value as f32 / max as f32) * plot_h;
                let x = i as f32 * bar_space + gap;
                let y = baseline - h;
                let x_center = x + bar_width / 2.0;

                let bar = canvas::Path::rectangle(Point::new(x, y), Size::new(bar_width, h));
                // use color from Twitch response
                frame.fill(&bar, d.color);

                frame.fill_text(canvas::Text {
                    content: thousand_separator(d.value),
                    position: Point::new(x_center, y - 5.0),
                    color: theme.palette().text,
                    size: 16.0.into(),
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Bottom,
                    ..canvas::Text::default()
                });

                frame.fill_text(canvas::Text {
                    content: d.title.clone(),
                    position: Point::new(x_center, baseline + 5.0),
                    color: theme.extended_palette().secondary.strong.color,
                    size: 16.0.into(),
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Top,
                    ..canvas::Text::default()
                });
            }

            let axis = canvas::Path::line(
                Point::new(0.0, baseline),
                Point::new(frame.width(), baseline),
            );
            frame.stroke(
                &axis,
                canvas::Stroke::default()
                    .with_color(theme.extended_palette().secondary.strong.color)
                    .with_width(3.0),
            );
        });

        vec![geometry]
    }
}
