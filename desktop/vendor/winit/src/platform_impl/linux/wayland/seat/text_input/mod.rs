use std::ops::Deref;

use sctk::globals::GlobalData;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};

use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{delegate_dispatch, Dispatch};
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    ContentHint, ContentPurpose, Event as TextInputEvent, ZwpTextInputV3,
};

use crate::event::{Ime, WindowEvent};
use crate::platform_impl::wayland;
use crate::platform_impl::wayland::state::WinitState;
use crate::window::ImePurpose;

pub struct TextInputState {
    text_input_manager: ZwpTextInputManagerV3,
}

impl TextInputState {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let text_input_manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { text_input_manager })
    }
}

impl Deref for TextInputState {
    type Target = ZwpTextInputManagerV3;

    fn deref(&self) -> &Self::Target {
        &self.text_input_manager
    }
}

impl Dispatch<ZwpTextInputManagerV3, GlobalData, WinitState> for TextInputState {
    fn event(
        _state: &mut WinitState,
        _proxy: &ZwpTextInputManagerV3,
        _event: <ZwpTextInputManagerV3 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, TextInputData, WinitState> for TextInputState {
    fn event(
        state: &mut WinitState,
        text_input: &ZwpTextInputV3,
        event: <ZwpTextInputV3 as Proxy>::Event,
        data: &TextInputData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let windows = state.windows.get_mut();
        let mut text_input_data = data.inner.lock().unwrap();
        match event {
            TextInputEvent::Enter { surface } => {
                let window_id = wayland::make_wid(&surface);
                text_input_data.surface = Some(surface);

                let mut window = match windows.get(&window_id) {
                    Some(window) => window.lock().unwrap(),
                    None => return,
                };

                if window.ime_allowed() {
                    text_input.enable();
                    text_input.set_content_type_by_purpose(window.ime_purpose());
                    text_input.commit();
                    state.events_sink.push_window_event(WindowEvent::Ime(Ime::Enabled), window_id);
                }

                window.text_input_entered(text_input);
            },
            TextInputEvent::Leave { surface } => {
                text_input_data.surface = None;
                text_input_data.preedit_shown = false;

                // Always issue a disable.
                text_input.disable();
                text_input.commit();

                let window_id = wayland::make_wid(&surface);

                // XXX this check is essential, because `leave` could have a
                // reference to nil surface...
                let mut window = match windows.get(&window_id) {
                    Some(window) => window.lock().unwrap(),
                    None => return,
                };

                window.text_input_left(text_input);

                state.events_sink.push_window_event(WindowEvent::Ime(Ime::Disabled), window_id);
            },
            TextInputEvent::PreeditString { text, cursor_begin, cursor_end } => {
                let text = text.unwrap_or_default();
                let cursor_begin = usize::try_from(cursor_begin)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));
                let cursor_end = usize::try_from(cursor_end)
                    .ok()
                    .and_then(|idx| text.is_char_boundary(idx).then_some(idx));

                text_input_data.pending_preedit = Some(Preedit { text, cursor_begin, cursor_end })
            },
            TextInputEvent::CommitString { text } => {
                text_input_data.pending_preedit = None;
                text_input_data.pending_commit = text;
            },
            TextInputEvent::Done { .. } => {
                let window_id = match text_input_data.surface.as_ref() {
                    Some(surface) => wayland::make_wid(surface),
                    None => return,
                };

                for ime in text_input_data.take_done() {
                    state.events_sink.push_window_event(WindowEvent::Ime(ime), window_id);
                }
            },
            TextInputEvent::DeleteSurroundingText { .. } => {
                // Not handled.
            },
            _ => {},
        }
    }
}

pub trait ZwpTextInputV3Ext {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose);
}

impl ZwpTextInputV3Ext for ZwpTextInputV3 {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose) {
        let (hint, purpose) = match purpose {
            ImePurpose::Normal => (ContentHint::None, ContentPurpose::Normal),
            ImePurpose::Password => (ContentHint::SensitiveData, ContentPurpose::Password),
            ImePurpose::Terminal => (ContentHint::None, ContentPurpose::Terminal),
        };
        self.set_content_type(hint, purpose);
    }
}

/// The Data associated with the text input.
#[derive(Default)]
pub struct TextInputData {
    inner: std::sync::Mutex<TextInputDataInner>,
}

#[derive(Default)]
pub struct TextInputDataInner {
    /// The `WlSurface` we're performing input to.
    surface: Option<WlSurface>,

    /// The commit to submit on `done`.
    pending_commit: Option<String>,

    /// The preedit to submit on `done`.
    pending_preedit: Option<Preedit>,

    /// Whether the last preedit sent to the window had text, so there is one to clear.
    preedit_shown: bool,
}

impl TextInputDataInner {
    /// The `Ime` events a `done` sends, from what the input method set before it.
    ///
    /// Quire fork (kurtlieber/quire): an empty preedit goes out only to clear one that shows.
    /// An input method answers each `commit` with a `done`, and the window commits a new cursor
    /// rectangle whenever the caret moves on screen, scrolling included. Upstream winit sent an
    /// empty preedit for every such `done`, and Rune hands it to the focused field as an edit,
    /// which scrolls the caret back into view: the mouse wheel stopped where the caret would
    /// leave the screen.
    fn take_done(&mut self) -> Vec<Ime> {
        let commit = self.pending_commit.take();
        let preedit = self.pending_preedit.take().filter(|preedit| !preedit.text.is_empty());
        let mut events = Vec::new();

        // Clear the preedit, unless all we'll be doing next is sending a new preedit.
        if self.preedit_shown && (commit.is_some() || preedit.is_none()) {
            events.push(Ime::Preedit(String::new(), None));
        }

        if let Some(text) = commit {
            events.push(Ime::Commit(text));
        }

        self.preedit_shown = preedit.is_some();
        if let Some(preedit) = preedit {
            let cursor_range =
                preedit.cursor_begin.map(|b| (b, preedit.cursor_end.unwrap_or(b)));
            events.push(Ime::Preedit(preedit.text, cursor_range));
        }

        events
    }
}

/// The state of the preedit.
struct Preedit {
    text: String,
    cursor_begin: Option<usize>,
    cursor_end: Option<usize>,
}

delegate_dispatch!(WinitState: [ZwpTextInputManagerV3: GlobalData] => TextInputState);
delegate_dispatch!(WinitState: [ZwpTextInputV3: TextInputData] => TextInputState);

#[cfg(test)]
mod tests {
    use super::{Preedit, TextInputDataInner};
    use crate::event::Ime;

    fn preedit(data: &mut TextInputDataInner, text: &str) {
        let end = Some(text.len());
        data.pending_preedit =
            Some(Preedit { text: text.into(), cursor_begin: end, cursor_end: end });
    }

    #[test]
    fn a_bare_done_sends_nothing_when_no_preedit_shows() {
        let mut data = TextInputDataInner::default();
        assert_eq!(data.take_done(), vec![]);
        preedit(&mut data, "");
        assert_eq!(data.take_done(), vec![]);
    }

    #[test]
    fn a_shown_preedit_is_cleared_once() {
        let mut data = TextInputDataInner::default();
        preedit(&mut data, "ni");
        assert_eq!(data.take_done(), vec![Ime::Preedit("ni".into(), Some((2, 2)))]);
        assert_eq!(data.take_done(), vec![Ime::Preedit(String::new(), None)]);
        assert_eq!(data.take_done(), vec![]);
    }

    #[test]
    fn an_empty_preedit_clears_a_shown_one() {
        let mut data = TextInputDataInner::default();
        preedit(&mut data, "ni");
        data.take_done();
        preedit(&mut data, "");
        assert_eq!(data.take_done(), vec![Ime::Preedit(String::new(), None)]);
    }

    #[test]
    fn a_commit_clears_a_shown_preedit_first() {
        let mut data = TextInputDataInner::default();
        preedit(&mut data, "ni");
        data.take_done();
        data.pending_commit = Some("你".into());
        assert_eq!(
            data.take_done(),
            vec![Ime::Preedit(String::new(), None), Ime::Commit("你".into())]
        );
        data.pending_commit = Some("a".into());
        assert_eq!(data.take_done(), vec![Ime::Commit("a".into())]);
    }

    #[test]
    fn a_new_preedit_replaces_the_shown_one_without_a_clear() {
        let mut data = TextInputDataInner::default();
        preedit(&mut data, "n");
        data.take_done();
        preedit(&mut data, "ni");
        assert_eq!(data.take_done(), vec![Ime::Preedit("ni".into(), Some((2, 2)))]);
    }
}
