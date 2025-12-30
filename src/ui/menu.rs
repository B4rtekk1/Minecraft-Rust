//! Game menu system for multiplayer connection

/// Current game state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    /// Main menu - game paused, cursor visible
    Menu,
    /// Playing - game active, cursor captured
    Playing,
    /// Connecting to server
    Connecting,
}

impl Default for GameState {
    fn default() -> Self {
        GameState::Menu
    }
}

/// Fields available for interaction in the user interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuField {
    /// Input field for the server IP and port
    ServerAddress,
    /// Input field for the player's name
    Username,
    /// No field is currently selected
    None,
}

impl Default for MenuField {
    fn default() -> Self {
        MenuField::None
    }
}

/// Represents the configuration and interactive state of the game menu.
#[derive(Debug, Clone)]
pub struct MenuState {
    /// The address of the server to connect to
    pub server_address: String,
    /// The username of the local player
    pub username: String,
    /// The currently active input field
    pub selected_field: MenuField,
    /// An optional error message to display to the user
    pub error_message: Option<String>,
    /// An optional status message (e.g., "Connecting...")
    pub status_message: Option<String>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            server_address: "127.0.0.1:25565".to_string(),
            username: "Player".to_string(),
            selected_field: MenuField::None,
            error_message: None,
            status_message: None,
        }
    }
}

impl MenuState {
    /// Creates a new `MenuState` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle character input for selected field
    pub fn handle_char(&mut self, ch: char) {
        // Only printable ASCII, no control chars
        if !ch.is_ascii_control() {
            match self.selected_field {
                MenuField::ServerAddress => {
                    if self.server_address.len() < 50 {
                        self.server_address.push(ch);
                    }
                }
                MenuField::Username => {
                    if self.username.len() < 16 {
                        self.username.push(ch);
                    }
                }
                MenuField::None => {}
            }
        }
    }

    /// Removes the last character from the currently selected field's value.
    pub fn handle_backspace(&mut self) {
        match self.selected_field {
            MenuField::ServerAddress => {
                self.server_address.pop();
            }
            MenuField::Username => {
                self.username.pop();
            }
            MenuField::None => {}
        }
    }

    /// Cycles the focus to the next available input field.
    pub fn next_field(&mut self) {
        self.selected_field = match self.selected_field {
            MenuField::None => MenuField::ServerAddress,
            MenuField::ServerAddress => MenuField::Username,
            MenuField::Username => MenuField::None,
        };
    }

    /// Select specific field
    pub fn select_field(&mut self, field: MenuField) {
        self.selected_field = field;
    }

    /// Clear error message
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Set error message
    pub fn set_error(&mut self, msg: &str) {
        self.error_message = Some(msg.to_string());
    }

    /// Set status message
    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some(msg.to_string());
    }

    /// Check if a field is selected
    pub fn is_editing(&self) -> bool {
        self.selected_field != MenuField::None
    }
}
