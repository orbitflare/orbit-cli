use ratatui::style::Color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeName {
    Ocean,
    Midnight,
    Dawn,
    Nord,
    Solarized,
    Dracula,
}

impl ThemeName {
    pub const ALL: [ThemeName; 6] = [
        ThemeName::Ocean,
        ThemeName::Midnight,
        ThemeName::Dawn,
        ThemeName::Nord,
        ThemeName::Solarized,
        ThemeName::Dracula,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeName::Ocean => "Ocean",
            ThemeName::Midnight => "Midnight",
            ThemeName::Dawn => "Dawn",
            ThemeName::Nord => "Nord",
            ThemeName::Solarized => "Solarized",
            ThemeName::Dracula => "Dracula",
        }
    }

    pub fn next(self) -> ThemeName {
        let all = Self::ALL;
        let idx = all.iter().position(|&t| t == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    pub fn prev(self) -> ThemeName {
        let all = Self::ALL;
        let idx = all.iter().position(|&t| t == self).unwrap_or(0);
        all[(idx + all.len() - 1) % all.len()]
    }

    pub fn from_str(s: &str) -> ThemeName {
        match s.to_lowercase().as_str() {
            "midnight" => ThemeName::Midnight,
            "dawn" => ThemeName::Dawn,
            "nord" => ThemeName::Nord,
            "solarized" => ThemeName::Solarized,
            "dracula" => ThemeName::Dracula,
            _ => ThemeName::Ocean,
        }
    }

    pub fn palette(self) -> Theme {
        match self {
            ThemeName::Ocean => OCEAN,
            ThemeName::Midnight => MIDNIGHT,
            ThemeName::Dawn => DAWN,
            ThemeName::Nord => NORD,
            ThemeName::Solarized => SOLARIZED,
            ThemeName::Dracula => DRACULA,
        }
    }

    pub fn dot_color(self) -> Color {
        match self {
            ThemeName::Ocean => Color::Rgb(94, 196, 197),
            ThemeName::Midnight => Color::Rgb(40, 200, 120),
            ThemeName::Dawn => Color::Rgb(229, 192, 123),
            ThemeName::Nord => Color::Rgb(136, 192, 208),
            ThemeName::Solarized => Color::Rgb(38, 139, 210),
            ThemeName::Dracula => Color::Rgb(189, 147, 249),
        }
    }

}

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub bg_card: Color,
    pub bg_sidebar: Color,
    pub bg_nav_active: Color,
    pub accent: Color,
    pub border_dim: Color,
    pub label: Color,
    pub value: Color,
    pub success: Color,
    pub yellow: Color,
    pub ticker_bg: Color,
    pub green_dot: Color,
}

const OCEAN: Theme = Theme {
    bg:            Color::Rgb(11, 14, 20),
    bg_card:       Color::Rgb(17, 21, 30),
    bg_sidebar:    Color::Rgb(13, 16, 24),
    bg_nav_active: Color::Rgb(20, 35, 45),
    accent:        Color::Rgb(94, 196, 197),
    border_dim:    Color::Rgb(50, 55, 65),
    label:         Color::Rgb(130, 130, 130),
    value:         Color::White,
    success:       Color::Rgb(152, 195, 121),
    yellow:        Color::Rgb(229, 192, 123),
    ticker_bg:     Color::Rgb(15, 18, 26),
    green_dot:     Color::Rgb(80, 200, 120),
};

const MIDNIGHT: Theme = Theme {
    bg:            Color::Rgb(10, 10, 16),
    bg_card:       Color::Rgb(18, 18, 26),
    bg_sidebar:    Color::Rgb(12, 12, 20),
    bg_nav_active: Color::Rgb(20, 35, 40),
    accent:        Color::Rgb(94, 196, 197),
    border_dim:    Color::Rgb(40, 42, 55),
    label:         Color::Rgb(120, 120, 135),
    value:         Color::Rgb(235, 235, 240),
    success:       Color::Rgb(152, 195, 121),
    yellow:        Color::Rgb(229, 192, 123),
    ticker_bg:     Color::Rgb(8, 8, 14),
    green_dot:     Color::Rgb(80, 200, 120),
};

const DAWN: Theme = Theme {
    bg:            Color::Rgb(250, 244, 237),
    bg_card:       Color::Rgb(255, 250, 245),
    bg_sidebar:    Color::Rgb(242, 236, 228),
    bg_nav_active: Color::Rgb(220, 238, 235),
    accent:        Color::Rgb(45, 125, 110),
    border_dim:    Color::Rgb(200, 195, 185),
    label:         Color::Rgb(120, 115, 105),
    value:         Color::Rgb(35, 30, 25),
    success:       Color::Rgb(45, 125, 110),
    yellow:        Color::Rgb(180, 130, 30),
    ticker_bg:     Color::Rgb(238, 232, 224),
    green_dot:     Color::Rgb(45, 125, 110),
};

const NORD: Theme = Theme {
    bg:            Color::Rgb(59, 66, 82),
    bg_card:       Color::Rgb(67, 76, 94),
    bg_sidebar:    Color::Rgb(46, 52, 64),
    bg_nav_active: Color::Rgb(76, 86, 106),
    accent:        Color::Rgb(136, 192, 208),
    border_dim:    Color::Rgb(76, 86, 106),
    label:         Color::Rgb(160, 172, 186),
    value:         Color::Rgb(236, 239, 244),
    success:       Color::Rgb(163, 190, 140),
    yellow:        Color::Rgb(235, 203, 139),
    ticker_bg:     Color::Rgb(46, 52, 64),
    green_dot:     Color::Rgb(163, 190, 140),
};

const SOLARIZED: Theme = Theme {
    bg:            Color::Rgb(0, 43, 54),
    bg_card:       Color::Rgb(7, 54, 66),
    bg_sidebar:    Color::Rgb(0, 36, 46),
    bg_nav_active: Color::Rgb(7, 54, 66),
    accent:        Color::Rgb(181, 137, 0),
    border_dim:    Color::Rgb(88, 110, 117),
    label:         Color::Rgb(131, 148, 150),
    value:         Color::Rgb(238, 232, 213),
    success:       Color::Rgb(133, 153, 0),
    yellow:        Color::Rgb(203, 75, 22),
    ticker_bg:     Color::Rgb(0, 36, 46),
    green_dot:     Color::Rgb(133, 153, 0),
};

const DRACULA: Theme = Theme {
    bg:            Color::Rgb(40, 42, 54),
    bg_card:       Color::Rgb(50, 52, 66),
    bg_sidebar:    Color::Rgb(40, 42, 54),
    bg_nav_active: Color::Rgb(68, 71, 90),
    accent:        Color::Rgb(189, 147, 249),
    border_dim:    Color::Rgb(98, 114, 164),
    label:         Color::Rgb(148, 150, 170),
    value:         Color::Rgb(248, 248, 242),
    success:       Color::Rgb(80, 250, 123),
    yellow:        Color::Rgb(241, 250, 140),
    ticker_bg:     Color::Rgb(35, 37, 48),
    green_dot:     Color::Rgb(80, 250, 123),
};
