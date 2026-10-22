//! Keyboard shortcuts.

use std::borrow::Cow;

use egui::{Key, Modifiers};

use crate::app::App;
use crate::i18n::{Locale, gettext, pgettext};
use crate::model::{Action, Dialog, Page};

pub(super) const fn platform_shortcut<'a>(ctrl: &'a str, cmd: &'a str) -> &'a str {
    if cfg!(target_os = "macos") { cmd } else { ctrl }
}

pub(super) const SIDEBAR_SHORTCUT: &str = platform_shortcut("Ctrl+B", "Cmd+B");
pub(super) const QUIT_SHORTCUT: &str = platform_shortcut("Ctrl+Q", "Cmd+Q");
pub(super) const WINAMP_SHORTCUT: &str = platform_shortcut("Ctrl+M", "Cmd+Shift+M");
pub(super) const MILKDROP_SHORTCUT: &str = platform_shortcut("Ctrl+Shift+K", "Cmd+Shift+K");

pub fn handle(app: &mut App, ctx: &egui::Context) {
    let typing = ctx.memory(|memory| memory.focused().is_some());
    // A focused song row still takes Ctrl+arrow to change songs; a text
    // field uses those keys to move its caret.
    let editing_text = ctx.text_edit_focused();
    let mut actions = Vec::new();
    let vim = app.settings.vim_keys;
    let help = !typing && ctx.input_mut(|input| consume_shortcut_help(&mut input.events));
    if help {
        actions.push(Action::ShowDialog(Dialog::Shortcuts));
    }
    let blocked =
        typing || help || app.dialog.is_some() || app.show_devices || egui::Popup::is_any_open(ctx);
    if vim && !blocked {
        let (commands, prefix) =
            ctx.input_mut(|input| vim_commands(&mut input.events, input.time, app.navigation.g_at));
        actions.extend(commands.into_iter().map(Action::Navigate));
        actions.push(Action::VimPrefix(prefix));
    } else if app.navigation.g_at.is_some() {
        actions.push(Action::VimPrefix(None));
    }
    ctx.input_mut(|input| {
        let mut key = |modifiers: Modifiers, key: Key, action: Action| {
            if input.consume_key(modifiers, key) {
                actions.push(action);
            }
        };
        // egui ignores an extra Shift when it matches, so a Shift shortcut
        // is looked for before the plain one it extends: Ctrl+B would
        // otherwise take Ctrl+Shift+B, and Ctrl+Q Ctrl+Shift+Q.
        key(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::A,
            Action::OpenUri("artist".into()),
        );
        key(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::B,
            Action::OpenUri("album".into()),
        );
        // Cmd+Shift+Q is Log Out, taken by the window server.
        if cfg!(target_os = "macos") {
            key(Modifiers::COMMAND, Key::U, Action::ToggleQueuePanel);
        } else {
            key(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Q,
                Action::ToggleQueuePanel,
            );
        }
        key(Modifiers::COMMAND, Key::F, Action::FocusSearch);
        key(Modifiers::COMMAND, Key::B, Action::ToggleSidebar);
        key(Modifiers::COMMAND, Key::Comma, Action::Open(Page::Settings));
        key(Modifiers::COMMAND, Key::Q, Action::Quit);
        // The platform's close key. macOS only closes a window from its
        // menu, which winit does not install, and the mini player has no
        // title bar for the system to close it by.
        key(Modifiers::COMMAND, Key::W, Action::CloseWindow);
        // winit installs its own macOS app menu, whose Hide item owns Cmd+H
        // before the window is offered the key.
        if cfg!(target_os = "macos") {
            key(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::H,
                Action::Open(Page::Home),
            );
        } else if !vim {
            key(Modifiers::COMMAND, Key::H, Action::Open(Page::Home));
        }
        // Ctrl+h/l belong to pane navigation in Vim mode, but remain entirely
        // untouched while an input has focus. Do not fall back to Home/Liked.
        if !vim || cfg!(target_os = "macos") {
            key(Modifiers::COMMAND, Key::L, Action::Open(Page::LikedSongs));
        }
        // Cmd+M minimises on macOS.
        if cfg!(target_os = "macos") {
            key(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::M,
                Action::ToggleWinampWindow,
            );
        } else {
            key(Modifiers::COMMAND, Key::M, Action::ToggleWinampWindow);
        }
        // Winamp's key for starting and stopping the visualisation plug-in.
        key(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::K,
            Action::ToggleWinampMilkdrop,
        );
        key(
            Modifiers::COMMAND,
            Key::Slash,
            Action::ShowDialog(Dialog::Shortcuts),
        );
        // In a text field these keys move the caret: Ctrl or Alt by a word,
        // Cmd to the end of the line, Up and Down to the end of the text.
        // Taking them here skipped songs while a search was being typed.
        if !editing_text {
            // Space always controls the current song, even when a song row
            // or button has focus. Enter still activates the focused control.
            key(Modifiers::NONE, Key::Space, Action::TogglePlay);
            key(Modifiers::ALT, Key::ArrowLeft, Action::Back);
            key(Modifiers::ALT, Key::ArrowRight, Action::Forward);
            key(Modifiers::COMMAND, Key::ArrowLeft, Action::Previous);
            key(Modifiers::COMMAND, Key::ArrowRight, Action::Next);
            key(Modifiers::COMMAND, Key::ArrowUp, Action::VolumeBy(5));
            key(Modifiers::COMMAND, Key::ArrowDown, Action::VolumeBy(-5));
        }
        if !blocked {
            key(Modifiers::ALT, Key::H, Action::SeekBy(-10_000));
            key(Modifiers::ALT, Key::L, Action::SeekBy(10_000));
        }
        if !typing {
            key(Modifiers::SHIFT, Key::ArrowLeft, Action::SeekBy(-10_000));
            key(Modifiers::SHIFT, Key::ArrowRight, Action::SeekBy(10_000));
            key(Modifiers::NONE, Key::M, Action::ToggleMute);
            key(Modifiers::NONE, Key::S, Action::ToggleShuffle);
            key(Modifiers::NONE, Key::R, Action::CycleRepeat);
            key(Modifiers::NONE, Key::Q, Action::ToggleQueuePanel);
            key(
                if vim {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
                Key::L,
                Action::ToggleLyricsPanel,
            );
            key(Modifiers::NONE, Key::Slash, Action::FocusSearch);
            key(
                Modifiers::NONE,
                Key::F,
                Action::SetLyricsFullscreen(app.lyrics_fullscreen.is_none()),
            );
        }
    });
    if !typing
        && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::B))
        && let Some(now) = app.now_playing().filter(|now| !now.is_episode)
    {
        actions.push(Action::ToggleSaved(now.uri));
    }
    // Resolve the "open current artist/album" placeholders.
    for action in actions {
        match action {
            Action::OpenUri(kind) if kind == "artist" => {
                if let Some(id) = app
                    .now_playing()
                    .and_then(|now| now.artists.first().and_then(|artist| artist.id.clone()))
                {
                    app.actions.push(Action::Open(Page::Artist(id)));
                }
            }
            Action::OpenUri(kind) if kind == "album" => {
                if let Some(now) = app.now_playing() {
                    if let Some(id) = now.album_id {
                        app.actions.push(Action::Open(Page::Album(id)));
                    } else if let Some(id) = now.show_id {
                        app.actions.push(Action::Open(Page::Show(id)));
                    }
                }
            }
            other => app.actions.push(other),
        }
    }
    // Map mouse back and forward buttons to navigation.
    let (back, forward) = ctx.input(|input| {
        (
            input.pointer.button_pressed(egui::PointerButton::Extra1),
            input.pointer.button_pressed(egui::PointerButton::Extra2),
        )
    });
    if back {
        app.actions.push(Action::Back);
    }
    if forward {
        app.actions.push(Action::Forward);
    }
    if ctx.input(|input| input.key_pressed(Key::Escape)) {
        if app.dialog.is_some() {
            app.actions.push(Action::CloseDialog);
        } else if app.show_devices {
            app.show_devices = false;
        } else if app.lyrics_fullscreen.is_some() {
            app.actions.push(Action::SetLyricsFullscreen(false));
        }
    }
}

/// Layouts can report ? as a named key, Shift+Slash, or only a text event.
/// Consume the slash event too so help cannot accidentally focus search.
fn consume_shortcut_help(events: &mut Vec<egui::Event>) -> bool {
    let text_question = events
        .iter()
        .any(|event| matches!(event, egui::Event::Text(text) if text == "?"));
    let question_key = |event: &egui::Event| match event {
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            !modifiers.ctrl
                && !modifiers.mac_cmd
                && !modifiers.command
                && !modifiers.alt
                && (*key == Key::Questionmark
                    || (*key == Key::Slash && (modifiers.shift || text_question)))
        }
        _ => false,
    };
    let requested = text_question || events.iter().any(question_key);
    if requested {
        events.retain(|event| {
            !question_key(event) && !matches!(event, egui::Event::Text(text) if text == "?")
        });
    }
    requested
}

/// Consume exact modifiers so Shift+G cannot become the first half of gg,
/// and command shortcuts never become bare navigation keys.
fn vim_commands(
    events: &mut Vec<egui::Event>,
    now: f64,
    prefix: Option<f64>,
) -> (Vec<super::navigation::Command>, Option<f64>) {
    use super::navigation::Command;
    let mut prefix = prefix.filter(|at| now - at <= 0.75);
    let mut commands = Vec::new();
    events.retain(|event| {
        let egui::Event::Key {
            key,
            pressed: true,
            repeat,
            modifiers,
            ..
        } = event
        else {
            return true;
        };
        if *modifiers == Modifiers::NONE && *key == Key::G {
            if !repeat {
                if prefix.take().is_some() {
                    commands.push(Command::First);
                } else {
                    prefix = Some(now);
                }
            }
            return false;
        }
        if *modifiers == Modifiers::NONE && *key == Key::H && !repeat && prefix.is_some() {
            prefix = None;
            commands.push(Command::Home);
            return false;
        }
        prefix = None;
        let command = if *modifiers == Modifiers::NONE {
            match key {
                Key::H => Some(Command::Left),
                Key::L => Some(Command::Right),
                Key::J => Some(Command::Down),
                Key::K => Some(Command::Up),
                Key::Enter => Some(Command::Play),
                Key::O => Some(Command::Open),
                Key::Escape => Some(Command::Clear),
                _ => None,
            }
        } else if *modifiers == Modifiers::SHIFT && *key == Key::G {
            Some(Command::Last)
        } else if modifiers.ctrl && !modifiers.alt && !modifiers.shift && !modifiers.mac_cmd {
            match key {
                Key::D => Some(Command::HalfDown),
                Key::U => Some(Command::HalfUp),
                Key::H => Some(Command::PaneLeft),
                Key::L => Some(Command::PaneRight),
                _ => None,
            }
        } else {
            None
        };
        if let Some(command) = command {
            commands.push(command);
            false
        } else {
            true
        }
    });
    (commands, prefix)
}

/// The rows of the keyboard shortcuts dialog: the keys, then what they do.
/// Key names stay as the keyboard prints them; words around them, and
/// every description, are translated.
pub fn shortcuts(locale: Locale) -> Vec<(Cow<'static, str>, Cow<'static, str>)> {
    let keys = |text: &'static str| Cow::Borrowed(text);
    vec![
        (
            pgettext(locale, "key", "Space"),
            gettext(locale, "Play or pause"),
        ),
        (
            keys(platform_shortcut("Ctrl+←  /  Ctrl+→", "Cmd+←  /  Cmd+→")),
            gettext(locale, "Previous or next"),
        ),
        (
            keys("Alt+H / Alt+L or Shift+← / Shift+→"),
            gettext(locale, "Seek 10 seconds"),
        ),
        (
            keys(platform_shortcut("Ctrl+↑  /  Ctrl+↓", "Cmd+↑  /  Cmd+↓")),
            gettext(locale, "Volume up or down"),
        ),
        (keys("M"), gettext(locale, "Mute or unmute")),
        (
            keys("B"),
            gettext(locale, "Like or unlike the playing song"),
        ),
        (keys("S"), gettext(locale, "Toggle shuffle")),
        (keys("R"), gettext(locale, "Cycle repeat")),
        (keys("Q"), gettext(locale, "Show the queue")),
        (
            keys("L (Shift+L with Vim keys)"),
            gettext(locale, "Show the lyrics"),
        ),
        (
            keys("h / j / k / l"),
            keys("Vim keys: spatial card selection; h/l switch panes in track lists"),
        ),
        (keys("j / k"), keys("Vim keys: select next / previous row")),
        (keys("gh"), keys("Vim keys: open Home")),
        (
            keys("l (sidebar)"),
            keys("Vim keys: open selected entry and focus its content"),
        ),
        (
            keys("Control+h / Control+l"),
            keys("Vim keys: switch panes (not in inputs)"),
        ),
        (keys("gg / Shift+G"), keys("Vim keys: first / last row")),
        (
            keys("Control+d / Control+u"),
            keys("Vim keys: half-page down / up"),
        ),
        (
            keys("Enter / o"),
            keys("Vim keys: open card; in lists, play / open album"),
        ),
        (keys("Esc"), keys("Vim keys: clear selection")),
        (keys("F"), gettext(locale, "Full screen lyrics")),
        (keys("Esc"), gettext(locale, "Lyrics: leave full screen")),
        (
            keys("Shift+↑  /  Shift+↓"),
            gettext(locale, "Song list: extend or shrink the selection"),
        ),
        (keys("Delete"), gettext(locale, "Remove from this playlist")),
        (
            keys(platform_shortcut("Ctrl+A", "Cmd+A")),
            gettext(locale, "Song list: select all"),
        ),
        (
            keys(platform_shortcut("Ctrl+C", "Cmd+C")),
            gettext(locale, "Song list: copy the selected songs' links"),
        ),
        (
            keys(platform_shortcut("Ctrl+X", "Cmd+X")),
            gettext(locale, "Playlist: cut the selected songs"),
        ),
        (
            keys(platform_shortcut("Ctrl+V", "Cmd+V")),
            gettext(locale, "Playlist: add the pasted song links"),
        ),
        (
            if cfg!(target_os = "macos") {
                // Translators: Keep the key names. Only the word "or" is translated.
                gettext(locale, "Cmd+F  or  /")
            } else {
                // Translators: Keep the key names. Only the word "or" is translated.
                gettext(locale, "Ctrl+F  or  /")
            },
            gettext(locale, "Search"),
        ),
        (
            keys(SIDEBAR_SHORTCUT),
            gettext(locale, "Show or hide the sidebar"),
        ),
        (keys("Alt+←  /  Alt+→"), gettext(locale, "Back or forward")),
        (
            keys(platform_shortcut("Ctrl+H", "Cmd+Shift+H")),
            gettext(locale, "Home"),
        ),
        (
            keys(platform_shortcut("Ctrl+L", "Cmd+L")),
            gettext(locale, "Liked Songs"),
        ),
        (
            keys(platform_shortcut("Ctrl+Shift+A", "Cmd+Shift+A")),
            gettext(locale, "Go to the playing artist"),
        ),
        (
            keys(platform_shortcut("Ctrl+Shift+B", "Cmd+Shift+B")),
            gettext(locale, "Go to the playing album"),
        ),
        (keys(WINAMP_SHORTCUT), gettext(locale, "Winamp mini player")),
        (
            keys(MILKDROP_SHORTCUT),
            gettext(locale, "MilkDrop, under the mini player"),
        ),
        (
            // Translators: Keep the key name F. Translate "or" and "double-click".
            gettext(locale, "F  or  double-click"),
            gettext(locale, "MilkDrop: fill the screen"),
        ),
        (keys("→  /  N"), gettext(locale, "MilkDrop: next preset")),
        (
            keys("←  /  P"),
            gettext(locale, "MilkDrop: previous preset"),
        ),
        (keys("L"), gettext(locale, "MilkDrop: keep this preset")),
        (
            keys("Esc"),
            gettext(locale, "MilkDrop: leave full screen, or close"),
        ),
        (
            keys(platform_shortcut("Ctrl+,", "Cmd+,")),
            gettext(locale, "Settings"),
        ),
        (
            if cfg!(target_os = "macos") {
                // Translators: Keep the key names. Only the word "or" is translated.
                gettext(locale, "Cmd+/ or ?")
            } else {
                // Translators: Keep the key names. Only the word "or" is translated.
                gettext(locale, "Ctrl+/ or ?")
            },
            gettext(locale, "Keyboard shortcuts"),
        ),
        (
            keys(platform_shortcut("Ctrl+W", "Cmd+W")),
            gettext(locale, "Close the window"),
        ),
        (keys(QUIT_SHORTCUT), gettext(locale, "Quit")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppOptions;
    use crate::paths::AppDirs;
    use crate::settings::Settings;

    fn event(key: Key, modifiers: Modifiers, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        }
    }

    #[test]
    fn vim_prefix_requires_two_presses_and_expires() {
        use super::super::navigation::Command;
        let (_, prefix) = vim_commands(&mut vec![event(Key::G, Modifiers::NONE, false)], 1.0, None);
        assert_eq!(prefix, Some(1.0));
        let (commands, _) =
            vim_commands(&mut vec![event(Key::G, Modifiers::NONE, true)], 1.1, prefix);
        assert!(commands.is_empty());
        let (commands, prefix) = vim_commands(
            &mut vec![event(Key::G, Modifiers::NONE, false)],
            1.2,
            prefix,
        );
        assert_eq!(commands, vec![Command::First]);
        assert_eq!(prefix, None);
        let (commands, prefix) = vim_commands(
            &mut vec![event(Key::G, Modifiers::NONE, false)],
            3.0,
            Some(1.0),
        );
        assert!(commands.is_empty());
        assert_eq!(prefix, Some(3.0));
        let (commands, prefix) = vim_commands(
            &mut vec![
                event(Key::J, Modifiers::NONE, false),
                event(Key::G, Modifiers::NONE, false),
            ],
            3.1,
            prefix,
        );
        assert_eq!(commands, vec![Command::Down]);
        assert_eq!(prefix, Some(3.1));
    }

    #[test]
    fn vim_modifiers_are_exact_and_unrelated_shortcuts_survive() {
        use super::super::navigation::Command;
        let mut events = vec![
            event(Key::G, Modifiers::SHIFT, false),
            event(Key::D, Modifiers::CTRL, false),
            event(Key::U, Modifiers::CTRL, false),
            event(Key::L, Modifiers::SHIFT, false),
            event(Key::L, Modifiers::COMMAND, false),
            event(Key::K, Modifiers::ALT, false),
        ];
        let (commands, prefix) = vim_commands(&mut events, 0.0, Some(0.0));
        assert_eq!(
            commands,
            vec![Command::Last, Command::HalfDown, Command::HalfUp]
        );
        assert_eq!(events.len(), 3);
        assert_eq!(prefix, None);
    }

    #[test]
    fn vim_pane_shortcuts_require_control_with_or_without_the_command_flag() {
        use super::super::navigation::Command;
        for modifiers in [Modifiers::CTRL, Modifiers::CTRL | Modifiers::COMMAND] {
            let mut events = vec![
                event(Key::H, modifiers, false),
                event(Key::L, modifiers, false),
            ];
            let (commands, _) = vim_commands(&mut events, 0.0, None);
            assert_eq!(commands, vec![Command::PaneLeft, Command::PaneRight]);
            assert!(events.is_empty());
        }
    }

    #[test]
    fn gh_opens_home_without_changing_gg_or_control_h() {
        use super::super::navigation::Command;
        let (commands, prefix) = vim_commands(
            &mut vec![
                event(Key::G, Modifiers::NONE, false),
                event(Key::H, Modifiers::NONE, false),
            ],
            1.0,
            None,
        );
        assert_eq!(commands, vec![Command::Home]);
        assert_eq!(prefix, None);
        let (commands, _) = vim_commands(
            &mut vec![event(Key::H, Modifiers::CTRL, false)],
            1.1,
            Some(1.0),
        );
        assert_eq!(commands, vec![Command::PaneLeft]);
        let (commands, _) = vim_commands(
            &mut vec![event(Key::H, Modifiers::NONE, false)],
            2.0,
            Some(1.0),
        );
        assert_eq!(
            commands,
            vec![Command::Left],
            "expired g must not open Home"
        );
    }

    #[test]
    fn question_mark_help_handles_key_and_text_layouts_without_stealing_slash() {
        for mut events in [
            vec![event(Key::Questionmark, Modifiers::NONE, false)],
            vec![event(Key::Questionmark, Modifiers::SHIFT, false)],
            vec![event(Key::Slash, Modifiers::SHIFT, false)],
            vec![egui::Event::Text("?".into())],
            vec![
                event(Key::Slash, Modifiers::NONE, false),
                egui::Event::Text("?".into()),
            ],
        ] {
            assert!(consume_shortcut_help(&mut events));
            assert!(events.is_empty());
        }
        let mut slash = vec![
            event(Key::Slash, Modifiers::NONE, false),
            egui::Event::Text("/".into()),
        ];
        assert!(!consume_shortcut_help(&mut slash));
        assert_eq!(slash.len(), 2);
    }

    /// A text field edits with the arrow keys: Ctrl or Alt moves a word,
    /// Cmd moves to the end of the line, and Ctrl or Cmd with Up or Down
    /// to either end of the text. While a field has focus those keys are
    /// the field's, and the shortcuts on them wait until it lets go.
    #[test]
    fn a_focused_text_field_keeps_the_arrow_keys_it_edits_with() {
        let root =
            std::env::temp_dir().join(format!("spotifast-text-arrows-test-{}", std::process::id()));
        let dirs = AppDirs {
            config: root.join("config"),
            state: root.join("state"),
            cache: root.join("cache"),
        };
        let mut app = App::new(
            &crate::backend::Waker::default(),
            dirs,
            Settings::default(),
            AppOptions {
                media_controls: false,
                restore_sign_in: false,
                tray: false,
                autostart: false,
            },
        );
        crate::demo::populate(&mut app);

        // The modifiers as the platform reports its command key.
        let command = if cfg!(target_os = "macos") {
            Modifiers::MAC_CMD | Modifiers::COMMAND
        } else {
            Modifiers::CTRL | Modifiers::COMMAND
        };
        let ctx = egui::Context::default();
        let field = egui::Id::new("keys-test-field");
        let row = egui::Id::new("keys-test-row");
        let mut text = String::from("find this song");
        let end = text.chars().count();
        // Shortcuts first, then the page, as `ui::show` draws a frame.
        let frame = |app: &mut App, text: &mut String, events: Vec<egui::Event>| {
            app.actions.clear();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    handle(app, ui.ctx());
                    ui.add(egui::TextEdit::singleline(text).id(field));
                    if ui
                        .interact(
                            egui::Rect::from_min_size(
                                egui::pos2(0.0, 100.0),
                                egui::vec2(200.0, 28.0),
                            ),
                            row,
                            egui::Sense::click(),
                        )
                        .clicked()
                    {
                        app.actions.push(Action::Open(Page::Home));
                    }
                },
            );
            output.textures_delta.clear();
            format!("{:?}", app.actions)
        };
        let press = |key: Key, modifiers: Modifiers| {
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }]
        };
        let caret_at = |index: usize| {
            let mut state = egui::TextEdit::load_state(&ctx, field).expect("the field's state");
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(index),
                )));
            state.store(&ctx, field);
        };
        let caret = || {
            egui::TextEdit::load_state(&ctx, field)
                .and_then(|state| state.cursor.char_range())
                .map(|range| range.primary.index)
        };

        frame(&mut app, &mut text, vec![]);
        ctx.memory_mut(|memory| memory.request_focus(field));
        frame(&mut app, &mut text, vec![]);
        assert!(ctx.memory(|memory| memory.has_focus(field)));

        // Cmd+Left goes to the start of the line; Ctrl+Left back a word.
        let command_left = if cfg!(target_os = "macos") { 0 } else { 10 };
        for (keys, modifiers, lands, what) in [
            (
                Key::ArrowLeft,
                command,
                command_left,
                "the previous-song shortcut",
            ),
            (Key::ArrowLeft, Modifiers::ALT, 10, "the back shortcut"),
            (Key::ArrowUp, command, 0, "the volume shortcut"),
        ] {
            caret_at(end);
            assert_eq!(
                frame(&mut app, &mut text, press(keys, modifiers)),
                "[]",
                "{what} took {keys:?} from the focused field"
            );
            assert_eq!(
                caret(),
                Some(egui::text::CCursor::new(lands).index),
                "{keys:?} with {modifiers:?} moves the caret"
            );
        }
        assert_eq!(text, "find this song");

        // Space belongs to text editing while the field has focus.
        caret_at(end);
        let mut space = press(Key::Space, Modifiers::NONE);
        space.push(egui::Event::Text(" ".into()));
        assert_eq!(frame(&mut app, &mut text, space), "[]");
        assert_eq!(text, "find this song ");

        // With nothing being typed into, the shortcuts are the app's again.
        ctx.memory_mut(|memory| memory.surrender_focus(field));
        frame(&mut app, &mut text, vec![]);
        assert_eq!(
            frame(&mut app, &mut text, press(Key::ArrowRight, command)),
            format!("{:?}", [Action::Next])
        );
        assert_eq!(
            frame(&mut app, &mut text, press(Key::ArrowLeft, Modifiers::ALT)),
            format!("{:?}", [Action::Back])
        );
        // A focused non-text row still receives player/navigation shortcuts.
        ctx.memory_mut(|memory| memory.request_focus(row));
        frame(&mut app, &mut text, vec![]);
        assert!(ctx.memory(|memory| memory.has_focus(row)));
        assert!(!ctx.text_edit_focused());
        assert_eq!(
            frame(&mut app, &mut text, press(Key::Space, Modifiers::NONE)),
            format!("{:?}", [Action::TogglePlay]),
            "a focused row must not steal Space from play/pause"
        );
        assert_eq!(
            frame(&mut app, &mut text, press(Key::Enter, Modifiers::NONE)),
            format!("{:?}", [Action::Open(Page::Home)]),
            "Enter still activates the focused row"
        );
        assert_eq!(
            frame(&mut app, &mut text, press(Key::ArrowRight, command)),
            format!("{:?}", [Action::Next])
        );
        app.backend.shutdown();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn shortcut_constants_name_the_platform_modifier() {
        let expected = if cfg!(target_os = "macos") {
            ["Cmd+B", "Cmd+Q", "Cmd+Shift+M", "Cmd+Shift+K"]
        } else {
            ["Ctrl+B", "Ctrl+Q", "Ctrl+M", "Ctrl+Shift+K"]
        };
        assert_eq!(
            [
                SIDEBAR_SHORTCUT,
                QUIT_SHORTCUT,
                WINAMP_SHORTCUT,
                MILKDROP_SHORTCUT,
            ],
            expected
        );
    }

    #[test]
    fn shortcut_dialog_never_names_the_other_command_modifier() {
        let other = if cfg!(target_os = "macos") {
            "Ctrl+"
        } else {
            "Cmd+"
        };
        for (keys, _) in shortcuts(Locale::English) {
            assert!(!keys.contains(other), "wrong modifier in {keys}");
        }
    }

    #[test]
    fn shortcut_dialog_names_platform_reserved_alternatives() {
        let label = |description: &str| {
            shortcuts(Locale::English)
                .into_iter()
                .find(|(_, candidate)| candidate == description)
                .map(|(keys, _)| keys)
                .unwrap()
        };
        if cfg!(target_os = "macos") {
            assert_eq!(label("Home"), "Cmd+Shift+H");
            assert_eq!(label("Winamp mini player"), "Cmd+Shift+M");
        } else {
            assert_eq!(label("Home"), "Ctrl+H");
            assert_eq!(label("Winamp mini player"), "Ctrl+M");
        }
    }

    #[test]
    fn alt_h_l_seek_in_both_modes_without_stealing_focused_or_modal_input() {
        let root = std::env::temp_dir().join(format!(
            "spotifast-seek-shortcut-test-{}",
            std::process::id()
        ));
        let mut app = App::new(
            &crate::backend::Waker::default(),
            AppDirs {
                config: root.join("config"),
                state: root.join("state"),
                cache: root.join("cache"),
            },
            Settings::default(),
            AppOptions {
                media_controls: false,
                restore_sign_in: false,
                tray: false,
                autostart: false,
            },
        );
        for vim in [false, true] {
            app.settings.vim_keys = vim;
            for (key, delta) in [(Key::H, -10_000), (Key::L, 10_000)] {
                // Normal, focused control, dialog, device picker, and help.
                for blocked_case in 0..5 {
                    let ctx = egui::Context::default();
                    if blocked_case == 1 {
                        ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("input")));
                    }
                    app.dialog = (blocked_case == 2).then_some(Dialog::Shortcuts);
                    app.show_devices = blocked_case == 3;
                    app.actions.clear();
                    let mut events = vec![event(key, Modifiers::ALT, false)];
                    if blocked_case == 4 {
                        events.push(event(Key::Questionmark, Modifiers::NONE, false));
                    }
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            events,
                            ..Default::default()
                        },
                        |_ui| handle(&mut app, &ctx),
                    );
                    output.textures_delta.clear();
                    if blocked_case == 0 {
                        assert!(
                            matches!(
                                app.actions.last(),
                                Some(Action::SeekBy(value)) if *value == delta
                            ),
                            "vim={vim}, key={key:?}"
                        );
                        assert!(!app.actions.iter().any(|action| matches!(
                            action,
                            Action::Navigate(_) | Action::ToggleLyricsPanel
                        )));
                    } else {
                        assert!(
                            !app.actions
                                .iter()
                                .any(|action| matches!(action, Action::SeekBy(_))),
                            "vim={vim}, key={key:?}, blocked={blocked_case}"
                        );
                    }
                }
            }
        }
        app.backend.shutdown();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn b_toggles_the_playing_song_in_liked_songs() {
        let root = std::env::temp_dir().join(format!(
            "spotifast-like-shortcut-test-{}",
            std::process::id()
        ));
        let dirs = AppDirs {
            config: root.join("config"),
            state: root.join("state"),
            cache: root.join("cache"),
        };
        let mut app = App::new(
            &crate::backend::Waker::default(),
            dirs,
            Settings::default(),
            AppOptions {
                media_controls: false,
                restore_sign_in: false,
                tray: false,
                autostart: false,
            },
        );
        crate::demo::populate(&mut app);

        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key: Key::B,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |_ui| handle(&mut app, &ctx));
        output.textures_delta.clear();

        assert!(matches!(
            app.actions.as_slice(),
            [Action::ToggleSaved(uri)] if uri == "spotify:track:trk0"
        ));
        app.backend.shutdown();
        let _ = std::fs::remove_dir_all(root);
    }

    /// egui ignores an extra Shift when it matches a shortcut, so the
    /// plain Ctrl+Q would also take Ctrl+Shift+Q if it were looked for
    /// first.
    #[test]
    fn a_shift_shortcut_is_not_taken_by_the_plain_one_it_extends() {
        let root = std::env::temp_dir().join(format!(
            "spotifast-shift-shortcut-test-{}",
            std::process::id()
        ));
        let dirs = AppDirs {
            config: root.join("config"),
            state: root.join("state"),
            cache: root.join("cache"),
        };
        let mut app = App::new(
            &crate::backend::Waker::default(),
            dirs,
            Settings::default(),
            AppOptions {
                media_controls: false,
                restore_sign_in: false,
                tray: false,
                autostart: false,
            },
        );
        crate::demo::populate(&mut app);
        let album = app.now_playing().and_then(|now| now.album_id);
        assert!(album.is_some(), "the demo song has an album to go to");

        // The modifiers as the platform reports its command key.
        let command = if cfg!(target_os = "macos") {
            Modifiers::MAC_CMD | Modifiers::COMMAND
        } else {
            Modifiers::CTRL | Modifiers::COMMAND
        };
        let ctx = egui::Context::default();
        let press = |app: &mut App, key: Key, modifiers: Modifiers| {
            app.actions.clear();
            let input = egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| handle(app, ui.ctx()));
            output.textures_delta.clear();
            format!("{:?}", app.actions)
        };

        let shift = command | Modifiers::SHIFT;
        // macOS shows the queue on Cmd+U; Cmd+Shift+Q is Log Out.
        if !cfg!(target_os = "macos") {
            assert_eq!(
                press(&mut app, Key::Q, shift),
                format!("{:?}", [Action::ToggleQueuePanel]),
                "the queue shortcut shows the queue"
            );
        }
        assert_eq!(
            press(&mut app, Key::Q, command),
            format!("{:?}", [Action::Quit]),
            "the quit shortcut still quits"
        );
        assert_eq!(
            press(&mut app, Key::B, shift),
            format!("{:?}", [Action::Open(Page::Album(album.unwrap()))]),
            "the album shortcut goes to the album"
        );
        assert_eq!(
            press(&mut app, Key::B, command),
            format!("{:?}", [Action::ToggleSidebar]),
            "the sidebar shortcut still toggles the sidebar"
        );
        assert_eq!(
            press(&mut app, Key::F, command),
            format!("{:?}", [Action::FocusSearch]),
            "the search shortcut is not taken by plain F"
        );
        assert_eq!(
            press(&mut app, Key::F, Modifiers::NONE),
            format!("{:?}", [Action::SetLyricsFullscreen(true)]),
            "F enters lyrics full screen"
        );
        app.lyrics_fullscreen = Some(false);
        assert_eq!(
            press(&mut app, Key::F, Modifiers::NONE),
            format!("{:?}", [Action::SetLyricsFullscreen(false)]),
            "F leaves lyrics full screen"
        );
        app.backend.shutdown();
        let _ = std::fs::remove_dir_all(root);
    }
}
