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

pub fn neutral_button(_theme: &Theme, status: button::Status) -> Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(0x9E, 0x9E, 0x9E),
        button::Status::Pressed => Color::from_rgb8(0x75, 0x75, 0x75),
        button::Status::Disabled => Color::from_rgb8(0xBD, 0xBD, 0xBD),
        button::Status::Active => Color::from_rgb8(0x88, 0x88, 0x88),
    };
    Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
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

pub fn red_button(_theme: &Theme, status: button::Status) -> Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(0xE5, 0x39, 0x35),
        button::Status::Pressed => Color::from_rgb8(0xB7, 0x1C, 0x1C),
        button::Status::Disabled => Color::from_rgb8(0xE9, 0x9A, 0x9A),
        button::Status::Active => Color::from_rgb8(0xD3, 0x2F, 0x2F),
    };
    Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
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

pub fn thousand_separator(number: i32) -> String {
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
            out.push('.');
        }
        out.push(c);
    }
    out
}

pub fn get_base_color(color: &str) -> Color {
    match color {
        "BLUE" => Color::from_rgb8(0x38, 0x7A, 0xFF),
        "PINK" => Color::from_rgb8(0xf5, 0x00, 0x9b),
        &_ => Color::WHITE,
    }
}

#[cfg(test)]
mod tests {
    use super::thousand_separator;

    #[test]
    fn zero() {
        assert_eq!(thousand_separator(0), "0");
    }

    #[test]
    fn no_separator_below_one_thousand() {
        assert_eq!(thousand_separator(1), "1");
        assert_eq!(thousand_separator(999), "999");
    }

    #[test]
    fn single_separator() {
        assert_eq!(thousand_separator(1000), "1.000");
        assert_eq!(thousand_separator(999999), "999.999");
    }

    #[test]
    fn multiple_separators() {
        assert_eq!(thousand_separator(1000000), "1.000.000");
        assert_eq!(thousand_separator(1234567), "1.234.567");
    }

    #[test]
    fn negative_numbers() {
        assert_eq!(thousand_separator(-1), "-1");
        assert_eq!(thousand_separator(-1000), "-1.000");
        assert_eq!(thousand_separator(-1234567), "-1.234.567");
    }

    #[test]
    fn extremes() {
        assert_eq!(thousand_separator(i32::MAX), "2.147.483.647");
        assert_eq!(thousand_separator(i32::MIN), "-2.147.483.648");
    }
}

pub fn prediction_button(color: &str, status: button::Status, is_active: bool) -> Style {
    let base_colour = get_base_color(color);
    let darker = slightly_darken(base_colour);
    let background = match status {
        button::Status::Hovered => base_colour,
        button::Status::Pressed => base_colour,
        button::Status::Disabled => Color::BLACK,
        button::Status::Active => {
            if is_active {
                base_colour
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
