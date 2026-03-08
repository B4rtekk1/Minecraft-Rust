#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Menu,
    Playing,
    Connecting,
}

impl Default for GameState {
    fn default() -> Self {
        GameState::Menu
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuField {
    ServerAddress,
    Username,
    None,
}

impl Default for MenuField {
    fn default() -> Self {
        MenuField::None
    }
}

#[derive(Debug, Clone)]
pub struct MenuState {
    pub server_address: String,
    pub username: String,
    pub selected_field: MenuField,
    pub error_message: Option<String>,
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

#[allow(dead_code)]
impl MenuState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_char(&mut self, ch: char) {
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

    pub fn next_field(&mut self) {
        self.selected_field = match self.selected_field {
            MenuField::None => MenuField::ServerAddress,
            MenuField::ServerAddress => MenuField::Username,
            MenuField::Username => MenuField::None,
        };
    }

    pub fn select_field(&mut self, field: MenuField) {
        self.selected_field = field;
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub fn set_error(&mut self, msg: &str) {
        self.error_message = Some(msg.to_string());
    }

    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some(msg.to_string());
    }

    pub fn is_editing(&self) -> bool {
        self.selected_field != MenuField::None
    }
}
