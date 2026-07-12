use crate::config::ConfigList;
use crate::{style, Message, SPACING};
use iced::widget::{
    button, center, column, container, pick_list, row, stack, text, text_input, tooltip, Button,
    Column, Container, PickList, Row, Text, TextInput,
};
use iced::{Center, Color, Element, Length, Renderer, Theme};
use iced_aw::number_input;

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

pub fn config_bar(
    configs: &ConfigList,
    name: &str,
    on_select: impl Fn(String) -> Message + 'static,
    on_name_change: impl Fn(String) -> Message + 'static,
    on_new: Message,
    on_save: Message,
    is_favorite: bool,
    on_toggle_favorite: Option<Message>,
    on_delete: impl Fn(String) -> Message + 'static,
) -> Element<'static, Message> {
    let dropdown: PickList<'_, String, Vec<String>, String, Message> =
        pick_list(configs.items.clone(), configs.selected.clone(), on_select)
            .placeholder("Select a config to load");

    let mut name_input: TextInput<_> = text_input("Config Name", name);
    if !configs.loaded {
        name_input = name_input.on_input(on_name_change);
    }
    let new_btn: Button<_> = button("New").on_press(on_new).style(style::neutral_button);

    // Overwrite guard: block Save when a config of this name already exists, unless it was
    // explicitly loaded first (so editing-then-saving a loaded config is still allowed).
    let can_save = configs.loaded || !configs.items.iter().any(|i| i == name);

    let save_btn = button("Save").style(style::neutral_button);
    let save_elem: Element<'_, Message> = if can_save {
        save_btn.on_press(on_save).into()
    } else {
        tooltip(
            save_btn,
            container("Config with this name already exists, to change load the config first.")
                .padding(10)
                .style(container::dark),
            tooltip::Position::Bottom,
        )
        .into()
    };

    let star = if is_favorite { "★" } else { "☆" };
    let mut fav_btn = button(text(star).size(24).center())
        .padding([0, 4])
        .style(style::neutral_button);
    if let Some(msg) = on_toggle_favorite {
        fav_btn = fav_btn.on_press(msg);
    }

    let fav_tip = "Favorite config gets auto-loaded at startup.";
    let fav_elem: Element<'_, Message> = tooltip(
        fav_btn,
        container(text(fav_tip)).padding(10).style(container::dark),
        tooltip::Position::Bottom,
    )
    .into();

    let mut del_btn = button(text("✖").size(24).center())
        .padding([0, 4])
        .style(style::neutral_button);
    if configs.loaded {
        del_btn = del_btn.on_press(on_delete(name.to_string()));
    }

    let del_tip = "Delete a loaded config.";
    let del_elem: Element<'_, Message> = tooltip(
        del_btn,
        container(text(del_tip)).padding(10).style(container::dark),
        tooltip::Position::Bottom,
    )
    .into();

    row![dropdown, name_input, new_btn, save_elem, fav_elem, del_elem]
        .spacing(SPACING)
        .into()
}

pub fn option_editor(
    options: &[String],
    editable: bool,
    on_change: impl Fn(usize, String) -> Message + Clone + 'static,
    on_remove: impl Fn(usize) -> Message + 'static,
) -> Column<'static, Message> {
    let mut opt_col = column![].spacing(SPACING);
    for (idx, option) in options.iter().enumerate() {
        let on_change = on_change.clone();
        let mut input = text_input(format!("Option {}", idx + 1).as_str(), option);
        if editable {
            input = input.on_input(move |s| on_change(idx, s));
        }
        let mut rem_btn = button(text("-").center())
            .width(30)
            .style(style::red_button);
        if editable && options.len() > 2 {
            rem_btn = rem_btn.on_press(on_remove(idx));
        }
        opt_col = opt_col.push(row![rem_btn, input].spacing(SPACING));
    }
    opt_col
}

pub fn duration_row(
    editable: bool,
    duration: &usize,
    on_change: impl Fn(usize) -> Message + Copy + 'static,
) -> Row<'static, Message> {
    let duration_text = Text::new("Duration in mins: ");
    let mut duration_inp = number_input(duration, 1..=30, on_change);
    if !editable {
        duration_inp = duration_inp.on_input_maybe(None::<fn(usize) -> Message>)
    }

    row![duration_text, duration_inp].align_y(Center)
}
