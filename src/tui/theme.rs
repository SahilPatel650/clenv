use ratatui::style::Color;

pub struct Theme {
    pub accent: Color,       // Yellow — active tab, table header, emphasis
    pub highlight: Color,    // Cyan — labels, active input cursor, sort selection
    pub ok: Color,           // Green — healthy status, success messages
    pub warn: Color,         // Yellow — warning health status
    pub danger: Color,       // Red — broken health, delete confirm border
    pub muted: Color,        // DarkGray — secondary/inactive text
    pub text: Color,         // White — primary text in popups
    pub popup_bg: Color,     // Black — overlay background
    pub popup_border: Color, // White — popup border
    pub popup_title: Color,  // Cyan — popup title text
}

pub fn default_theme() -> Theme {
    Theme {
        accent: Color::Yellow,
        highlight: Color::Cyan,
        ok: Color::Green,
        warn: Color::Yellow,
        danger: Color::Red,
        muted: Color::DarkGray,
        text: Color::White,
        popup_bg: Color::Black,
        popup_border: Color::White,
        popup_title: Color::Cyan,
    }
}
