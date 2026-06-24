use iced::widget::button;
use iced::{Background, Color, Theme};
use iced_aw::style::{tab_bar, Status};

pub const TWITCH_PURPLE: Color = Color {
    r: 0x91 as f32 / 255.0,
    g: 0x46 as f32 / 255.0,
    b: 0xFF as f32 / 255.0,
    a: 1.0,
};

fn darken(color: Color) -> Color {
    let scale = 1.0 - 0.5;
    Color {
        r: color.r * scale,
        g: color.g * scale,
        b: color.b * scale,
        ..color
    }
}

pub fn neutral_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(0x9E, 0x9E, 0x9E),
        button::Status::Pressed => Color::from_rgb8(0x75, 0x75, 0x75),
        button::Status::Disabled => Color::from_rgb8(0xBD, 0xBD, 0xBD),
        button::Status::Active => Color::from_rgb8(0x88, 0x88, 0x88),
    };
    button::Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        ..button::Style::default()
    }
}

pub fn red_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(0xE5, 0x39, 0x35),
        button::Status::Pressed => Color::from_rgb8(0xB7, 0x1C, 0x1C),
        button::Status::Disabled => Color::from_rgb8(0xE9, 0x9A, 0x9A),
        button::Status::Active => Color::from_rgb8(0xD3, 0x2F, 0x2F),
    };
    button::Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        ..button::Style::default()
    }
}

pub fn twitch_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Color::from_rgb8(0x77, 0x2C, 0xE8),
        button::Status::Pressed => Color::from_rgb8(0x5C, 0x16, 0xC5),
        button::Status::Disabled => Color::from_rgb8(0x6A, 0x4B, 0xA8),
        button::Status::Active => TWITCH_PURPLE,
    };
    button::Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        ..button::Style::default()
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
