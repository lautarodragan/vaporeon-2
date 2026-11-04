use std::{
    collections::HashMap,
    fs::read_to_string,
    hash::Hash,
    ops::{BitOr, BitOrAssign},
    path::PathBuf,
    sync::LazyLock,
};

use gtk::gdk;
use strum::EnumString;

use crate::toml_io::{TomlFileError, get_config_dir_path};

static DEFAULT_ACTIONS_STR: &str = include_str!("../assets/actions.ini");
static DEFAULT_ACTIONS: LazyLock<HashMap<KeyBinding, Vec<Action>>> =
    LazyLock::new(|| Actions::from_str(DEFAULT_ACTIONS_STR).actions);

/// A key, independent of modifiers. Mirrors the subset of keys that can be used in `actions.ini`.
#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash)]
pub enum KeyCode {
    Char(char),
    F(u8),
    Enter,
    Esc,
    Right,
    Left,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Backspace,
    Tab,
    BackTab,
    Insert,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const ALT: Self = Self(4);

    pub fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn toggle(&mut self, other: Self) {
        self.0 ^= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub fn from_gdk(state: gdk::ModifierType) -> Self {
        let mut modifiers = Self::NONE;
        if state.contains(gdk::ModifierType::SHIFT_MASK) {
            modifiers |= Self::SHIFT;
        }
        if state.contains(gdk::ModifierType::CONTROL_MASK) {
            modifiers |= Self::CONTROL;
        }
        if state.contains(gdk::ModifierType::ALT_MASK) {
            modifiers |= Self::ALT;
        }
        modifiers
    }
}

impl BitOr for KeyModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A key press, as received from GTK.
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    #[cfg(test)]
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// Converts a GDK key press into a [`KeyEvent`]. Returns `None` for keys we don't care about
    /// (lone modifiers, media keys, etc.).
    pub fn from_gdk(key: gdk::Key, state: gdk::ModifierType) -> Option<Self> {
        let mut modifiers = KeyModifiers::from_gdk(state);

        let code = match key {
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => KeyCode::Enter,
            gdk::Key::Escape => KeyCode::Esc,
            gdk::Key::Right | gdk::Key::KP_Right => KeyCode::Right,
            gdk::Key::Left | gdk::Key::KP_Left => KeyCode::Left,
            gdk::Key::Up | gdk::Key::KP_Up => KeyCode::Up,
            gdk::Key::Down | gdk::Key::KP_Down => KeyCode::Down,
            gdk::Key::Home | gdk::Key::KP_Home => KeyCode::Home,
            gdk::Key::End | gdk::Key::KP_End => KeyCode::End,
            gdk::Key::Page_Up | gdk::Key::KP_Page_Up => KeyCode::PageUp,
            gdk::Key::Page_Down | gdk::Key::KP_Page_Down => KeyCode::PageDown,
            gdk::Key::Delete | gdk::Key::KP_Delete => KeyCode::Delete,
            gdk::Key::BackSpace => KeyCode::Backspace,
            gdk::Key::Tab | gdk::Key::KP_Tab => KeyCode::Tab,
            gdk::Key::ISO_Left_Tab => KeyCode::BackTab,
            gdk::Key::Insert | gdk::Key::KP_Insert => KeyCode::Insert,
            gdk::Key::space | gdk::Key::KP_Space => KeyCode::Char(' '),
            _ => {
                if let Some(n) = function_key_number(key) {
                    KeyCode::F(n)
                } else {
                    let c = key.to_unicode().filter(|c| !c.is_control())?;
                    // Shift is "part of" non-letter characters ('!' is Shift+1 on most layouts),
                    // so we don't report it, otherwise bindings like `?` would never match.
                    if !c.is_alphabetic() {
                        modifiers.remove(KeyModifiers::SHIFT);
                    }
                    KeyCode::Char(c)
                }
            }
        };

        Some(Self { code, modifiers })
    }
}

fn function_key_number(key: gdk::Key) -> Option<u8> {
    use gtk::glib::translate::IntoGlib;
    let f1 = gdk::Key::F1.into_glib();
    let f35 = gdk::Key::F35.into_glib();
    let k = key.into_glib();
    (f1..=f35).contains(&k).then(|| (k - f1 + 1) as u8)
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash)]
pub struct KeyBinding {
    code: KeyCode,
    modifiers: KeyModifiers,
}

impl KeyBinding {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn code(&self) -> KeyCode {
        self.code
    }

    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }
}

impl From<KeyEvent> for KeyBinding {
    fn from(key: KeyEvent) -> Self {
        Self {
            code: key.code,
            modifiers: key.modifiers,
        }
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Hash, Ord, PartialOrd)]
pub enum Action {
    Application(ApplicationAction),
    Task(TaskAction),
    Navigation(NavigationAction),
    Text(TextAction),
    Day(DayAction),
}

#[derive(Eq, PartialEq, Clone, Debug, Hash, EnumString, Ord, PartialOrd)]
pub enum ApplicationAction {
    Quit, // App will assert, on startup, as early as possible, there is at least one key-binding for Quit and crash otherwise.
    Save,
    Paste,
    PasteText(String),
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash, EnumString, Ord, PartialOrd)]
pub enum TaskAction {
    Create,
    Delete,
    Duplicate,
    Copy,
    Cut,
    MoveForwardInsideProject,
    MoveBackwardInsideProject,
    MoveToNextStatus,
    MoveToPreviousStatus,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash, EnumString, Ord, PartialOrd)]
pub enum NavigationAction {
    FocusDayOrGlobal,
    FocusDay,
    FocusGlobal,
    ToggleGlobalPanel,
    ToggleDetailsPanel,
    FocusNext,
    FocusPrevious,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    GoToPreviousDay,
    GoToNextDay,
    SelectFirstTaskInDay,
    SelectLastTaskInDay,
    SelectFirstTaskInProject,
    SelectLastTaskInProject,
    GoToToday,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash, EnumString, Ord, PartialOrd)]
pub enum TextAction {
    EnterEditMode,
    ConfirmEdit,
    ConfirmEditMultiLine,
    CancelEdit,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Hash, EnumString, Ord, PartialOrd)]
pub enum DayAction {
    SaveAsTemplate,
    LoadTemplate,
    FilterProjectsNone,
    FilterProjects1,
    FilterProjects2,
    FilterProjects3,
    FilterProjects4,
    CopyPreviousDayTodoAndDoing,
}

impl TryFrom<&str> for Action {
    type Error = strum::ParseError;

    fn try_from(value: &str) -> Result<Self, strum::ParseError> {
        let parts: Vec<&str> = value.split('.').collect();
        let [parent, child]: [&str] = parts[..] else {
            return Err(strum::ParseError::VariantNotFound);
        };

        if parent == "Application" {
            ApplicationAction::try_from(child).map(Action::Application)
        } else if parent == "Task" {
            TaskAction::try_from(child).map(Action::Task)
        } else if parent == "Navigation" {
            NavigationAction::try_from(child).map(Action::Navigation)
        } else if parent == "Text" {
            TextAction::try_from(child).map(Action::Text)
        } else if parent == "Day" {
            DayAction::try_from(child).map(Action::Day)
        } else {
            Err(strum::ParseError::VariantNotFound)
        }
    }
}

#[derive(Debug, Default)]
pub struct Actions {
    actions: HashMap<KeyBinding, Vec<Action>>,
}

impl Actions {
    fn from_str(s: &str) -> Self {
        // log::trace!("from str {s}");

        let mut actions: HashMap<KeyBinding, Vec<Action>> = HashMap::new();

        s.lines()
            .filter(|line| line.len() >= 3 && !line.trim().starts_with('#'))
            .map(|line| line.split('=').collect::<Vec<&str>>())
            .filter_map(str_to_action_keys)
            .for_each(|(action, bindings)| {
                bindings.split(' ').filter_map(str_to_binding).for_each(|binding| {
                    actions
                        .entry(binding)
                        .and_modify(|actions| actions.push(action.clone()))
                        .or_insert(vec![action.clone()]);
                });
            });

        // log::trace!("actions '{:#?}'", actions);

        Self { actions }
    }

    pub fn path() -> PathBuf {
        get_config_dir_path()
            .unwrap_or_else(|_| PathBuf::from("~/.config/vaporeon"))
            .join("actions.ini")
    }

    pub fn from_file() -> Result<Self, TomlFileError> {
        let path = Self::path();
        let string = read_to_string(path)?;
        Ok(Self::from_str(string.as_str()))
    }

    pub fn from_file_or_default() -> Self {
        Self::from_file().unwrap_or_default()
    }

    pub fn actions_by_key(&self, key: KeyEvent) -> Vec<Action> {
        let kb = KeyBinding::from(key);
        self.actions
            .get(&kb)
            .or(DEFAULT_ACTIONS.get(&kb))
            .cloned()
            .unwrap_or_default()
    }

    pub fn contains(&self, action: Action) -> bool {
        self.actions
            .values()
            .chain(DEFAULT_ACTIONS.values())
            .flatten()
            .any(|a| *a == action)
    }
}

pub trait OnActions<T = Action, R = ()> {
    fn on_actions(self, actions: Vec<T>) -> R;
}

fn str_to_action_keys(split: Vec<&str>) -> Option<(Action, &str)> {
    if let [value, keys] = split[..]
        && let Ok(action) = Action::try_from(value)
    {
        Some((action, keys))
    } else {
        None
    }
}

fn str_to_binding(binding: &str) -> Option<KeyBinding> {
    str_to_modifiers(binding)
        .and_then(|(modifiers, key)| str_to_key(key, modifiers).map(|code| KeyBinding::new(code, modifiers)))
}

fn str_to_modifiers(key: &str) -> Option<(KeyModifiers, &str)> {
    let mut modifiers = KeyModifiers::NONE;
    let mut key = key;

    loop {
        if key.starts_with("Ctrl") {
            modifiers.toggle(KeyModifiers::CONTROL);
            key = &key[4..];
        } else if key.starts_with("Alt") {
            modifiers.toggle(KeyModifiers::ALT);
            key = &key[3..];
        } else if key.starts_with("Shift") {
            modifiers.toggle(KeyModifiers::SHIFT);
            key = &key[5..];
        } else {
            break;
        }
    }

    Some((modifiers, key))
}

fn str_to_key(key: &str, modifiers: KeyModifiers) -> Option<KeyCode> {
    let code: KeyCode;

    if key.len() == 1 {
        let char = key.chars().next()?;

        if char.is_ascii_alphabetic() {
            if modifiers.contains(KeyModifiers::SHIFT) {
                code = KeyCode::Char(char);
            } else {
                code = KeyCode::Char(char.to_ascii_lowercase());
            }
        } else {
            code = KeyCode::Char(char);
        }
    } else if (key.len() == 2 || key.len() == 3)
        && key.starts_with('F')
        && let Ok(num) = key[1..].parse::<u8>()
    {
        code = KeyCode::F(num);
    } else if key == "Enter" {
        code = KeyCode::Enter;
    } else if key == "Esc" {
        code = KeyCode::Esc;
    } else if key == "Space" {
        code = KeyCode::Char(' ');
    } else if key == "Right" {
        code = KeyCode::Right;
    } else if key == "Left" {
        code = KeyCode::Left;
    } else if key == "Up" {
        code = KeyCode::Up;
    } else if key == "Down" {
        code = KeyCode::Down;
    } else if key == "Home" {
        code = KeyCode::Home;
    } else if key == "End" {
        code = KeyCode::End;
    } else if key == "PageUp" {
        code = KeyCode::PageUp;
    } else if key == "PageDown" {
        code = KeyCode::PageDown;
    } else if key == "Delete" {
        code = KeyCode::Delete;
    } else if key == "Backspace" {
        code = KeyCode::Backspace;
    } else if key == "Tab" {
        code = KeyCode::Tab;
    } else if key == "BackTab" {
        code = KeyCode::BackTab;
    } else if key == "Insert" {
        code = KeyCode::Insert;
    } else {
        return None;
    }
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions_for(code: KeyCode, modifiers: KeyModifiers) -> Vec<Action> {
        Actions::default().actions_by_key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn default_bindings() {
        assert_eq!(actions_for(KeyCode::Char('q'), KeyModifiers::CONTROL), vec![
            Action::Application(ApplicationAction::Quit)
        ]);
        assert_eq!(actions_for(KeyCode::Char('g'), KeyModifiers::NONE), vec![
            Action::Navigation(NavigationAction::ToggleGlobalPanel),
        ]);
        assert_eq!(actions_for(KeyCode::Enter, KeyModifiers::CONTROL), vec![
            Action::Task(TaskAction::Create),
            Action::Text(TextAction::ConfirmEditMultiLine),
        ]);
        assert!(
            actions_for(KeyCode::BackTab, KeyModifiers::SHIFT)
                .contains(&Action::Navigation(NavigationAction::FocusPrevious))
        );
    }

    #[test]
    fn gdk_keys() {
        use gtk::gdk::{Key, ModifierType};
        assert_eq!(
            KeyEvent::from_gdk(Key::q, ModifierType::CONTROL_MASK),
            Some(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL))
        );
        assert_eq!(
            KeyEvent::from_gdk(Key::G, ModifierType::SHIFT_MASK),
            Some(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT))
        );
        assert_eq!(
            KeyEvent::from_gdk(Key::exclam, ModifierType::SHIFT_MASK),
            Some(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE))
        );
        assert_eq!(
            KeyEvent::from_gdk(Key::ISO_Left_Tab, ModifierType::SHIFT_MASK),
            Some(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
        );
        assert_eq!(
            KeyEvent::from_gdk(Key::F5, ModifierType::empty()),
            Some(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE))
        );
        assert_eq!(KeyEvent::from_gdk(Key::Shift_L, ModifierType::empty()), None);
    }
}
