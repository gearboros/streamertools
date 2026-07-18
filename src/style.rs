use crate::settings::Separator;
use iced::widget::button::Style;
use iced::widget::{button, Text};
use iced::{Background, Color, Theme};
use iced_aw::style::{tab_bar, Status};

pub const TWITCH_PURPLE: Color = Color {
    r: 0x91 as f32 / 255.0,
    g: 0x46 as f32 / 255.0,
    b: 0xFF as f32 / 255.0,
    a: 1.0,
};

fn darken(color: Color) -> Color {
    darken_by_factor(color, 0.5f32)
}

fn slightly_darken(color: Color) -> Color {
    darken_by_factor(color, 0.3f32)
}

fn darken_by_factor(color: Color, factor: f32) -> Color {
    let scale = 1.0 - factor;
    Color {
        r: color.r * scale,
        g: color.g * scale,
        b: color.b * scale,
        ..color
    }
}

pub fn no_background_button(theme: &Theme, _status: button::Status) -> Style {
    Style {
        background: None,
        text_color: theme.palette().text,
        ..Style::default()
    }
}

pub fn neutral_button(theme: &Theme, status: button::Status) -> Style {
    let secondary = theme.extended_palette().secondary;
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => secondary.strong.color,
        button::Status::Disabled => secondary.weak.color,
        button::Status::Active => secondary.base.color,
    };
    Style {
        background: Some(background.into()),
        text_color: secondary.base.text,
        ..Style::default()
    }
}

pub fn dbg_button(_theme: &Theme, status: button::Status) -> Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(0xFF, 0xF9, 0x7D),
        button::Status::Pressed => Color::from_rgb8(0xF5, 0x7C, 0x00),
        button::Status::Disabled => Color::from_rgb8(0xFF, 0xE5, 0x8D),
        button::Status::Active => Color::from_rgb8(0xFF, 0xEB, 0x3B),
    };
    Style {
        background: Some(background.into()),
        text_color: Color::BLACK,
        border: iced::border::Border {
            color: Color::BLACK,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Style::default()
    }
}

pub fn red_button(theme: &Theme, status: button::Status) -> Style {
    let danger = theme.extended_palette().danger;
    let background = match status {
        button::Status::Hovered => danger.strong.color,
        button::Status::Pressed => slightly_darken(danger.base.color),
        button::Status::Disabled => danger.weak.color,
        button::Status::Active => danger.base.color,
    };
    Style {
        background: Some(background.into()),
        text_color: danger.base.text,
        ..Style::default()
    }
}

pub fn twitch_button(_theme: &Theme, status: button::Status) -> Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(0x77, 0x2C, 0xE8),
        button::Status::Pressed => Color::from_rgb8(0x5C, 0x16, 0xC5),
        button::Status::Disabled => Color::from_rgb8(0x6A, 0x4B, 0xA8),
        button::Status::Active => TWITCH_PURPLE,
    };
    Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        ..Style::default()
    }
}

pub fn twitch_tab(_theme: &Theme, status: Status) -> tab_bar::Style {
    let inactive = darken(TWITCH_PURPLE);
    let lit = matches!(status, Status::Active | Status::Hovered | Status::Pressed);
    tab_bar::Style {
        tab_label_background: Background::Color(if lit { TWITCH_PURPLE } else { inactive }),
        text_color: Color::WHITE,
        ..tab_bar::Style::default()
    }
}

pub fn bold_text<'a>(text: String) -> Text<'a> {
    Text::new(text).font(iced::Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    })
}

/// Groups digits with the thousands mark the user picked in the settings.
pub fn thousand_separator(number: i64) -> String {
    thousand_separator_with(number, Separator::active())
}

fn thousand_separator_with(number: i64, sep: Separator) -> String {
    let s = number.to_string();
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", s.as_str()),
    };
    let len = digits.len();
    let mut out = String::with_capacity(sign.len() + len + len / 3);
    out.push_str(sign);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(if sep == Separator::CommaDecimalDotThousand {
                '.'
            } else {
                ','
            });
        }
        out.push(c);
    }
    out
}

/// Formats with two decimals using the decimal mark the user picked in the settings.
pub fn decimal(number: f64) -> String {
    decimal_with(number, Separator::active())
}

fn decimal_with(number: f64, sep: Separator) -> String {
    let formatted = format!("{number:.2}");
    match sep {
        Separator::DotDecimalCommaThousand => formatted,
        Separator::CommaDecimalDotThousand => formatted.replace('.', ","),
    }
}

pub fn percent(number: f64) -> String {
    percent_with(number, Separator::active())
}

fn percent_with(number: f64, sep: Separator) -> String {
    format!("{}%", decimal_with(number, sep))
}

pub fn get_base_color(color: &str) -> Color {
    match color {
        "BLUE" => Color::from_rgb8(0x38, 0x7A, 0xFF),
        "PINK" => Color::from_rgb8(0xf5, 0x00, 0x9b),
        &_ => Color::WHITE,
    }
}

pub fn poll_colors() -> [Color; 5] {
    [
        get_base_color("BLUE"),
        get_base_color("PINK"),
        Color::from_rgb8(0xed, 0xa1, 0x00),
        Color::from_rgb8(0x00, 0x83, 0x00),
        Color::from_rgb8(0x4a, 0x3a, 0xa7),
    ]
}

pub fn poll_tab_colors() -> [Color; 3] {
    [
        TWITCH_PURPLE,
        Color::from_rgb8(0xF5, 0xB3, 0x00),
        Color::from_rgb8(0x00, 0xB5, 0xAD),
    ]
}

pub fn prediction_button(color: &str, status: button::Status, is_active: bool) -> Style {
    let base_color = get_base_color(color);
    color_button(base_color, status, is_active)
}

pub fn color_button(base_color: Color, status: button::Status, is_active: bool) -> Style {
    let darker = slightly_darken(base_color);
    let background = match status {
        button::Status::Hovered => base_color,
        button::Status::Pressed => base_color,
        button::Status::Disabled => darken_by_factor(base_color, 0.6),
        button::Status::Active => {
            if is_active {
                base_color
            } else {
                darker
            }
        }
    };
    Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        border: iced::border::Border {
            color: Color::BLACK,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{decimal_with, percent_with, thousand_separator_with};
    use crate::settings::Separator::{CommaDecimalDotThousand, DotDecimalCommaThousand};

    /// The helpers under test take the separator explicitly so they never touch the
    /// process-global setting, which tests running in parallel would otherwise race on.
    fn cddt(number: i64) -> String {
        thousand_separator_with(number, CommaDecimalDotThousand)
    }

    #[test]
    fn zero() {
        assert_eq!(cddt(0), "0");
    }

    #[test]
    fn no_separator_below_one_thousand() {
        assert_eq!(cddt(1), "1");
        assert_eq!(cddt(999), "999");
    }

    #[test]
    fn single_separator() {
        assert_eq!(cddt(1000), "1.000");
        assert_eq!(cddt(999999), "999.999");
    }

    #[test]
    fn multiple_separators() {
        assert_eq!(cddt(1000000), "1.000.000");
        assert_eq!(cddt(1234567), "1.234.567");
    }

    #[test]
    fn negative_numbers() {
        assert_eq!(cddt(-1), "-1");
        assert_eq!(cddt(-1000), "-1.000");
        assert_eq!(cddt(-1234567), "-1.234.567");
    }

    #[test]
    fn extremes() {
        assert_eq!(cddt(i32::MAX as i64), "2.147.483.647");
        assert_eq!(cddt(i32::MIN as i64), "-2.147.483.648");
        assert_eq!(cddt(i64::MAX), "9.223.372.036.854.775.807");
        assert_eq!(cddt(i64::MIN), "-9.223.372.036.854.775.808");
    }

    #[test]
    fn thousands_follow_the_selected_format() {
        assert_eq!(
            thousand_separator_with(1234567, DotDecimalCommaThousand),
            "1,234,567"
        );
        assert_eq!(
            thousand_separator_with(-1234567, DotDecimalCommaThousand),
            "-1,234,567"
        );
    }

    #[test]
    fn decimal_always_has_two_places() {
        assert_eq!(decimal_with(1234.5, DotDecimalCommaThousand), "1234.50");
        assert_eq!(decimal_with(1234.5, CommaDecimalDotThousand), "1234,50");
        assert_eq!(decimal_with(0.0, DotDecimalCommaThousand), "0.00");
    }

    #[test]
    fn decimal_rounds_and_keeps_the_sign() {
        assert_eq!(decimal_with(0.567, DotDecimalCommaThousand), "0.57");
        assert_eq!(decimal_with(-12.345, CommaDecimalDotThousand), "-12,35");
    }

    #[test]
    fn percent_appends_exactly_one_sign() {
        assert_eq!(percent_with(41.666, DotDecimalCommaThousand), "41.67%");
        assert_eq!(percent_with(41.666, CommaDecimalDotThousand), "41,67%");
        assert_eq!(
            percent_with(50.0, DotDecimalCommaThousand)
                .matches('%')
                .count(),
            1
        );
    }
}
