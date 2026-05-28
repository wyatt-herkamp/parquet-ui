//! "Editorial data instrument" theme — dark-mode-first palette, custom fonts,
//! and shared style helpers for the rest of the UI.
//!
//! Some palette tokens and helpers are exposed for downstream use even if not
//! consumed yet (e.g. `surface_2`, `accent_button`, `FONT_MONO_MEDIUM`).
#![allow(dead_code)]

use iced::font::{Family, Weight};
use iced::widget::{button, container, text};
use iced::{Background, Border, Color, Font, Theme};

// -- Fonts --------------------------------------------------------------------

pub const GEIST_REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/Geist-Regular.ttf");
pub const GEIST_MEDIUM_BYTES: &[u8] = include_bytes!("../assets/fonts/Geist-Medium.ttf");
pub const GEIST_SEMIBOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/Geist-SemiBold.ttf");
pub const JETBRAINS_MONO_REGULAR_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
pub const JETBRAINS_MONO_MEDIUM_BYTES: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf");

pub const FONT_UI: Font = Font {
    family: Family::Name("Geist"),
    weight: Weight::Normal,
    ..Font::DEFAULT
};

pub const FONT_UI_MEDIUM: Font = Font {
    family: Family::Name("Geist"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

pub const FONT_UI_SEMIBOLD: Font = Font {
    family: Family::Name("Geist"),
    weight: Weight::Semibold,
    ..Font::DEFAULT
};

pub const FONT_MONO: Font = Font {
    family: Family::Name("JetBrains Mono"),
    weight: Weight::Normal,
    ..Font::DEFAULT
};

pub const FONT_MONO_MEDIUM: Font = Font {
    family: Family::Name("JetBrains Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

// -- Palette ------------------------------------------------------------------

pub mod palette {
    use iced::Color;

    const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }
    const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a,
        }
    }

    pub const BG_DEEP: Color = rgb(0x0E, 0x0E, 0x10);
    pub const BG_SURFACE: Color = rgb(0x17, 0x17, 0x1A);
    pub const BG_SURFACE_2: Color = rgb(0x1F, 0x1F, 0x23);
    pub const BG_HOVER: Color = rgb(0x22, 0x22, 0x27);

    pub const BORDER_SUBTLE: Color = rgb(0x27, 0x27, 0x2D);
    pub const BORDER_STRONG: Color = rgb(0x3A, 0x3A, 0x42);

    pub const FG_PRIMARY: Color = rgb(0xE8, 0xE6, 0xE0);
    pub const FG_MUTED: Color = rgb(0x8A, 0x8A, 0x93);
    pub const FG_DIM: Color = rgb(0x5A, 0x5A, 0x63);

    pub const ACCENT_WARM: Color = rgb(0xE6, 0xA3, 0x3A);
    pub const ACCENT_WARM_SOFT: Color = rgba(0xE6, 0xA3, 0x3A, 0.18);
    pub const ACCENT_COOL: Color = rgb(0x3F, 0xB6, 0xA8);
    pub const ACCENT_COOL_SOFT: Color = rgba(0x3F, 0xB6, 0xA8, 0.16);
    pub const ACCENT_VIOLET: Color = rgb(0x9B, 0x8C, 0xF7);
    pub const ACCENT_VIOLET_SOFT: Color = rgba(0x9B, 0x8C, 0xF7, 0.16);
    pub const ACCENT_ROSE: Color = rgb(0xE3, 0x64, 0x64);

    pub const DIFF_CHANGED_BG: Color = rgba(0xE6, 0xA3, 0x3A, 0.14);
    pub const DIFF_CHANGED_BAR: Color = ACCENT_WARM;

    pub const BACKDROP_DIM: Color = rgba(0, 0, 0, 0.55);
}

// -- Theme --------------------------------------------------------------------

pub fn instrument_theme() -> Theme {
    use palette::*;
    Theme::custom(
        "Instrument".to_string(),
        iced::theme::Palette {
            background: BG_DEEP,
            text: FG_PRIMARY,
            primary: ACCENT_WARM,
            success: ACCENT_COOL,
            warning: ACCENT_WARM,
            danger: ACCENT_ROSE,
        },
    )
}

// -- Text helpers -------------------------------------------------------------

pub fn ui<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).font(FONT_UI).size(13)
}

pub fn ui_medium<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).font(FONT_UI_MEDIUM).size(13)
}

pub fn mono<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).font(FONT_MONO).size(12)
}

pub fn mono_sm<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).font(FONT_MONO).size(11)
}

pub fn display_strong<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).font(FONT_UI_SEMIBOLD).size(18)
}

/// Small caps label: uppercases + dim color + tight tracking-ish via wider size↓.
pub fn label_text(s: &str) -> text::Text<'static> {
    text(s.to_ascii_uppercase())
        .font(FONT_UI_MEDIUM)
        .size(10)
        .style(|_: &Theme| text::Style {
            color: Some(palette::FG_MUTED),
        })
}

pub fn muted<'a, T: text::IntoFragment<'a>>(t: text::Text<'a>) -> text::Text<'a> {
    let _ = std::marker::PhantomData::<T>;
    t.style(|_: &Theme| text::Style {
        color: Some(palette::FG_MUTED),
    })
}

// -- Container styles ---------------------------------------------------------

pub fn surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette::BG_SURFACE)),
        text_color: Some(palette::FG_PRIMARY),
        border: Border {
            color: palette::BORDER_SUBTLE,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn surface_2(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette::BG_SURFACE_2)),
        text_color: Some(palette::FG_PRIMARY),
        border: Border {
            color: palette::BORDER_STRONG,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn backdrop(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette::BACKDROP_DIM)),
        ..container::Style::default()
    }
}

pub fn top_bar(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette::BG_SURFACE)),
        text_color: Some(palette::FG_PRIMARY),
        border: Border {
            color: palette::BORDER_SUBTLE,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

pub fn top_bar_divider(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette::BORDER_SUBTLE)),
        ..container::Style::default()
    }
}

pub fn tab_underline(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette::ACCENT_WARM)),
        ..container::Style::default()
    }
}

pub fn notice_pill(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(palette::ACCENT_WARM_SOFT)),
        text_color: Some(palette::ACCENT_WARM),
        border: Border {
            color: palette::ACCENT_WARM,
            width: 1.0,
            radius: 999.0.into(),
        },
        ..container::Style::default()
    }
}

// -- Button styles ------------------------------------------------------------

pub fn ghost_button(_: &Theme, status: button::Status) -> button::Style {
    let (bg, fg) = match status {
        button::Status::Active => (Color::TRANSPARENT, palette::FG_MUTED),
        button::Status::Hovered => (palette::BG_HOVER, palette::FG_PRIMARY),
        button::Status::Pressed => (palette::BG_SURFACE_2, palette::ACCENT_WARM),
        button::Status::Disabled => (Color::TRANSPARENT, palette::FG_DIM),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: fg,
        border: Border {
            color: palette::BORDER_SUBTLE,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..button::Style::default()
    }
}

pub fn accent_button(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color {
            r: 1.0,
            g: 0.72,
            b: 0.32,
            a: 1.0,
        },
        button::Status::Pressed => Color {
            r: 0.78,
            g: 0.56,
            b: 0.20,
            a: 1.0,
        },
        button::Status::Disabled => Color {
            a: 0.4,
            ..palette::ACCENT_WARM
        },
        _ => palette::ACCENT_WARM,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: palette::BG_DEEP,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
        ..button::Style::default()
    }
}

pub fn tab_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    move |_: &Theme, status: button::Status| {
        let fg = if active {
            palette::FG_PRIMARY
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => palette::FG_PRIMARY,
                _ => palette::FG_MUTED,
            }
        };
        button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: fg,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..button::Style::default()
        }
    }
}

// -- Type-pill colors ---------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct PillColors {
    pub bg: Color,
    pub fg: Color,
}

pub fn pill_colors_for(kind: crate::wrangle::insights::ColumnKind) -> PillColors {
    use crate::wrangle::insights::ColumnKind;
    match kind {
        ColumnKind::Numeric => PillColors {
            bg: palette::ACCENT_COOL_SOFT,
            fg: palette::ACCENT_COOL,
        },
        ColumnKind::Temporal => PillColors {
            bg: palette::ACCENT_COOL_SOFT,
            fg: palette::ACCENT_COOL,
        },
        ColumnKind::String => PillColors {
            bg: Color {
                a: 0.10,
                ..palette::FG_PRIMARY
            },
            fg: palette::FG_PRIMARY,
        },
        ColumnKind::Boolean => PillColors {
            bg: palette::ACCENT_WARM_SOFT,
            fg: palette::ACCENT_WARM,
        },
        ColumnKind::Other => PillColors {
            bg: palette::ACCENT_VIOLET_SOFT,
            fg: palette::ACCENT_VIOLET,
        },
    }
}

/// Heuristic: is the data type nested (overrides ColumnKind for pill color)?
pub fn pill_colors_nested() -> PillColors {
    PillColors {
        bg: palette::ACCENT_VIOLET_SOFT,
        fg: palette::ACCENT_VIOLET,
    }
}

pub fn pill_style(colors: PillColors) -> impl Fn(&Theme) -> container::Style + Copy {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(colors.bg)),
        text_color: Some(colors.fg),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 3.0.into(),
        },
        ..container::Style::default()
    }
}
