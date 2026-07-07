use crate::Message;
use iced::Task;

pub trait EditableForm {
    const MIN_OPTIONS: usize = 2;
    const MAX_OPTIONS: usize;
    fn options_mut(&mut self) -> &mut Vec<String>;
    fn set_duration(&mut self, d: usize);
    fn set_title(&mut self, name: String);
}

#[derive(Debug, Clone)]
pub enum BaseFormMessage {
    TitleChanged(String),
    OptionChanged(usize, String),
    AddOption,
    RemoveOption(usize),
    DurationChanged(usize),
}

pub fn handle_base_changes<T: EditableForm>(
    form: &mut T,
    message: BaseFormMessage,
) -> Task<Message> {
    match message {
        BaseFormMessage::TitleChanged(t) => {
            form.set_title(t);
            Task::none()
        }
        BaseFormMessage::OptionChanged(idx, val) => {
            if let Some(o) = form.options_mut().get_mut(idx) {
                *o = val;
            }
            Task::none()
        }
        BaseFormMessage::AddOption => {
            form.options_mut().push(String::new());
            Task::none()
        }
        BaseFormMessage::RemoveOption(idx) => {
            let options = form.options_mut();
            if options.len() > T::MIN_OPTIONS {
                options.remove(idx);
            }
            Task::none()
        }
        BaseFormMessage::DurationChanged(d) => {
            form.set_duration(d);
            Task::none()
        }
    }
}
