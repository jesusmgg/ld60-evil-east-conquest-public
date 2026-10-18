use macroquad::color::Color;

#[derive(Clone)]
pub struct Styles {
    pub colors: Colors,
}

impl Styles {
    pub fn new() -> Self {
        let colors = Colors {
            black: Color::from_hex(0x000000),
            white: Color::from_hex(0xffffff),
            gray: Color::from_hex(0x666666),

            void: Color::from_hex(0x000000),
            cyan_bright: Color::from_hex(0x3cbfed),
            cyan_med: Color::from_hex(0x4a90a4),
            teal_accent: Color::from_hex(0x1a535c),
            blue_server: Color::from_hex(0x4a90a4),
            blue_shadow: Color::from_hex(0x1a535c),
            red_alarm: Color::from_hex(0xe63946),
            red_infection: Color::from_hex(0xe63946),
            red_corrupt: Color::from_hex(0xe63946),
            red_deep: Color::from_hex(0xe63946),
            orange_spark: Color::from_hex(0xff8c42),
            orange_warn: Color::from_hex(0xff8c42),
            yellow_hot: Color::from_hex(0xffd23f),
            corrupt_shadow: Color::from_hex(0x000000),
            corrupt_deep: Color::from_hex(0x000000),
            parasite_toxic: Color::from_hex(0x4a90a4),
            parasite_spread: Color::from_hex(0x4a90a4),
            parasite_core: Color::from_hex(0x4a90a4),
            gray_metal: Color::from_hex(0x666666),
            gray_ui: Color::from_hex(0x666666),
            gray_dark: Color::from_hex(0x000000),
        };

        Self { colors }
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct Colors {
    pub black: Color,
    pub white: Color,
    pub gray: Color,

    pub void: Color,
    pub cyan_bright: Color,
    pub cyan_med: Color,
    pub teal_accent: Color,
    pub blue_server: Color,
    pub blue_shadow: Color,
    pub red_alarm: Color,
    pub red_infection: Color,
    pub red_corrupt: Color,
    pub red_deep: Color,
    pub orange_spark: Color,
    pub orange_warn: Color,
    pub yellow_hot: Color,
    pub corrupt_shadow: Color,
    pub corrupt_deep: Color,
    pub parasite_toxic: Color,
    pub parasite_spread: Color,
    pub parasite_core: Color,
    pub gray_metal: Color,
    pub gray_ui: Color,
    pub gray_dark: Color,
}
