use crate::{Message, SPACING};
use iced::widget::{button, center, column, container, row, stack, text, Container};
use iced::{Color, Element, Length, Renderer, Theme};

pub fn empty_panel(
    icon: &'static str,
    heading: &'static str,
) -> Element<'static, Message, Theme, Renderer> {
    center(
        column![text(icon).size(48), text(heading).size(20),]
            .spacing(SPACING)
            .align_x(iced::Center),
    )
    .into()
}

pub fn split_pane(
    form: impl Into<Element<'static, Message, Theme, Renderer>>,
    results: impl Into<Element<'static, Message, Theme, Renderer>>,
) -> Element<'static, Message, Theme, Renderer> {
    Container::new(
        row![
            container(form).width(Length::FillPortion(2)).max_width(600),
            container(results).width(Length::FillPortion(3)),
        ]
        .spacing(SPACING * 2),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

pub fn modal<'a>(
    base: impl Into<Element<'a, Message>>,
    content: Container<'a, Message, Theme, Renderer>,
    on_blur: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        button("")
            .style(|_, _| button::Style {
                background: Some(
                    Color {
                        a: 0.8,
                        ..Color::BLACK
                    }
                    .into()
                ),
                ..button::Style::default()
            })
            .on_press(on_blur)
            .width(Length::Fill)
            .height(Length::Fill),
        center(content),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
