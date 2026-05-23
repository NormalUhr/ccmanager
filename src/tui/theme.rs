use std::sync::OnceLock;

/// Global theme instance, initialized once at startup
static THEME: OnceLock<Theme> = OnceLock::new();

/// RGB color tuple
type Rgb = (u8, u8, u8);

/// Color theme for the TUI application
#[derive(Debug, Clone)]
pub struct Theme {
    // Primary accent (warm Claude orange)
    pub accent: Rgb,
    pub accent_dim: Rgb,

    // Text colors
    pub text_primary: Rgb,
    pub text_secondary: Rgb,
    pub text_muted: Rgb,
    /// More dimmed than text_muted. Used for tertiary metadata
    /// (placeholder text, footer key descriptions).
    pub text_tertiary: Rgb,

    // Structural
    pub border: Rgb,
    pub separator: Rgb,

    // Backgrounds
    pub status_bar_bg: Rgb,
    pub overlay_bg: Rgb,
    pub selection_bg: Rgb,

    // Semantic colors
    pub diff_add: Rgb,
    pub diff_remove: Rgb,
    pub code_color: Rgb,
    pub heading: Rgb,
    pub thinking_text: Rgb,
    pub tool_text: Rgb,

    // List view specific
    pub custom_title: Rgb,
    pub custom_title_highlight: Rgb,
    pub summary: Rgb,
    pub summary_highlight: Rgb,
    pub model_color: Rgb,
    pub duration_color: Rgb,
    pub preview: Rgb,
    pub context_base: Rgb,
    pub context_highlight: Rgb,

    // List metadata
    pub dot_separator: Rgb,
    pub msg_count: Rgb,
    pub header_summary: Rgb,
    pub timestamp_now: Rgb,
    pub timestamp_minutes: Rgb,
    pub timestamp_hours: Rgb,
    pub timestamp_days: Rgb,

    // Disabled/dim states
    pub dim_key: Rgb,
    pub dim_label: Rgb,

    // Search
    pub search_match_bg: Rgb,

    // Viewer colors
    pub green: Rgb,
    pub blue: Rgb,

    // Syntect theme name for code highlighting
    pub syntect_theme: &'static str,
}

impl Theme {
    /// Dark theme - the original color scheme
    pub fn dark() -> Self {
        Self {
            accent: (230, 136, 106),    // Claude orange, brighter for dark bg
            accent_dim: (166, 102, 80), // ~60% strength of accent

            text_primary: (255, 255, 255),
            text_secondary: (185, 185, 185),
            text_muted: (150, 150, 150),
            text_tertiary: (110, 110, 110), // more dim than text_muted

            border: (60, 60, 60),
            separator: (50, 50, 50),

            status_bar_bg: (30, 30, 35),
            overlay_bg: (25, 25, 30),
            selection_bg: (55, 45, 40),

            diff_add: (120, 200, 120),
            diff_remove: (220, 120, 120),
            code_color: (147, 161, 199),
            heading: (180, 190, 200),
            thinking_text: (170, 175, 180),
            tool_text: (170, 175, 180),

            custom_title: (200, 180, 120),
            custom_title_highlight: (230, 210, 150),
            summary: (140, 155, 175),
            summary_highlight: (180, 195, 215),
            model_color: (180, 140, 200),
            duration_color: (165, 125, 100),
            preview: (175, 175, 175),
            context_base: (150, 150, 150),
            context_highlight: (230, 136, 106),

            dot_separator: (95, 95, 95),
            msg_count: (150, 150, 150),
            header_summary: (185, 185, 185),
            timestamp_now: (230, 136, 106),    // = accent
            timestamp_minutes: (201, 122, 96), // Warm orange
            timestamp_hours: (168, 128, 117),  // Muted warm gray
            timestamp_days: (150, 150, 150),   // Neutral gray

            dim_key: (60, 60, 60),
            dim_label: (60, 60, 60),

            search_match_bg: (90, 53, 40), // Warm dim wash

            green: (0, 255, 0),
            blue: (100, 149, 237),

            syntect_theme: "base16-ocean.dark",
        }
    }

    /// Light theme - designed for light terminal backgrounds
    pub fn light() -> Self {
        Self {
            accent: (191, 92, 60),     // Claude orange, deeper for light bg
            accent_dim: (148, 71, 47), // ~75% strength of accent

            text_primary: (36, 45, 53),     // Deep slate for body text
            text_secondary: (62, 72, 82),   // Lifted for clearer reading
            text_muted: (100, 108, 116),    // Clear medium gray
            text_tertiary: (138, 146, 154), // Distinctly dimmer but still legible

            border: (188, 196, 200),    // Subtle cool gray borders
            separator: (200, 208, 212), // Lighter separators

            status_bar_bg: (244, 240, 237), // Very light warm gray
            overlay_bg: (249, 247, 245),    // Near-white warm tint for modals
            selection_bg: (245, 220, 208),  // Pale warm wash for selection

            diff_add: (40, 120, 60),      // Dark green for additions
            diff_remove: (180, 50, 50),   // Dark red for removals
            code_color: (80, 70, 130),    // Dark purple-blue
            heading: (52, 70, 100),       // Dark slate navy
            thinking_text: (88, 96, 106), // Darker cool gray
            tool_text: (78, 90, 100),     // Slightly cool darker gray

            custom_title: (140, 105, 30),           // Deep warm gold
            custom_title_highlight: (170, 130, 40), // Brighter gold
            summary: (80, 100, 125),                // Slate blue
            summary_highlight: (50, 75, 110),       // Deeper slate for highlights
            model_color: (115, 75, 145),            // Deep purple
            duration_color: (148, 71, 47),          // Matches accent_dim
            preview: (78, 86, 94),                  // Darker for legible preview
            context_base: (100, 108, 116),          // Matches text_muted
            context_highlight: (191, 92, 60),       // Same as accent

            dot_separator: (168, 176, 182),   // Cool light gray
            msg_count: (82, 92, 100),         // Darker for clearer message counts
            header_summary: (62, 72, 82),     // Matches text_secondary
            timestamp_now: (191, 92, 60),     // Same as accent
            timestamp_minutes: (165, 86, 54), // Warm orange
            timestamp_hours: (139, 96, 81),   // Muted warm gray
            timestamp_days: (62, 72, 82),     // Matches text_secondary

            dim_key: (180, 188, 194), // Light for disabled
            dim_label: (180, 188, 194),

            search_match_bg: (245, 220, 208), // Pale warm wash for matches

            green: (40, 130, 60), // Dark green for quotes
            blue: (36, 97, 160),  // Dark blue for links

            syntect_theme: "InspiredGitHub",
        }
    }
}

/// Detect terminal background luminance and return appropriate theme
pub fn detect_theme() -> &'static Theme {
    THEME.get_or_init(|| {
        match terminal_light::luma() {
            Ok(luma) if luma > 0.6 => Theme::light(),
            _ => Theme::dark(), // Default to dark on detection failure
        }
    })
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn accent_is_warm_orange_in_both_themes() {
        // Warm orange = red channel dominant, then green, then blue.
        // Guards against accidentally reverting to the old cool-blue
        // or legacy teal accent.
        for theme in [Theme::dark(), Theme::light()] {
            let (r, g, b) = theme.accent;
            assert!(
                r > g && g > b,
                "accent should be warm orange (r > g > b), got rgb({},{},{})",
                r,
                g,
                b
            );
        }
    }

    #[test]
    fn text_tertiary_is_dimmer_than_text_muted_in_dark() {
        // In a dark theme, "dimmer" means closer to the background.
        // We pick the avg-channel as a rough proxy.
        let t = Theme::dark();
        let avg = |(r, g, b): Rgb| (r as u32 + g as u32 + b as u32) / 3;
        assert!(
            avg(t.text_tertiary) < avg(t.text_muted),
            "text_tertiary must be dimmer than text_muted (dark theme)"
        );
    }

    #[test]
    fn text_tertiary_is_dimmer_than_text_muted_in_light() {
        // On a light background, "dimmer" means *higher* numeric value
        // (closer to white = lower contrast = visually dimmer).
        let t = Theme::light();
        let avg = |(r, g, b): Rgb| (r as u32 + g as u32 + b as u32) / 3;
        assert!(
            avg(t.text_tertiary) > avg(t.text_muted),
            "text_tertiary must be dimmer than text_muted on light bg (higher = lighter = dimmer)"
        );
    }
}
