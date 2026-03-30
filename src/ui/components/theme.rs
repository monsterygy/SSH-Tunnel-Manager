use gpui::*;

/// Application color system — centralized color definitions.
pub struct AppColors;

impl AppColors {
    pub fn page_bg(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(220.0 / 360.0, 0.13, 0.10, 1.0)
        } else {
            hsla(80.0 / 360.0, 0.08, 0.96, 1.0)
        }
    }

    pub fn card_bg(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(220.0 / 360.0, 0.13, 0.14, 1.0)
        } else {
            hsla(0.0, 0.0, 1.0, 1.0)
        }
    }

    pub fn primary_text(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(210.0 / 360.0, 0.07, 0.90, 1.0)
        } else {
            hsla(210.0 / 360.0, 0.07, 0.14, 1.0)
        }
    }

    pub fn secondary_text(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(207.0 / 360.0, 0.05, 0.60, 1.0)
        } else {
            hsla(207.0 / 360.0, 0.08, 0.50, 1.0)
        }
    }

    pub fn border_color(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(210.0 / 360.0, 0.08, 0.22, 1.0)
        } else {
            hsla(210.0 / 360.0, 0.08, 0.90, 1.0)
        }
    }

    pub fn accent_color(_is_dark: bool) -> Hsla {
        hsla(177.0 / 360.0, 0.50, 0.24, 1.0)
    }

    pub fn success_color() -> Hsla {
        hsla(142.0 / 360.0, 0.71, 0.45, 1.0)
    }

    pub fn danger_color() -> Hsla {
        hsla(0.0, 0.84, 0.60, 1.0)
    }

    pub fn warning_color() -> Hsla {
        hsla(45.0 / 360.0, 0.93, 0.58, 1.0)
    }

    pub fn selected_bg(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(177.0 / 360.0, 0.25, 0.18, 1.0)
        } else {
            hsla(177.0 / 360.0, 0.15, 0.92, 1.0)
        }
    }

    // ── Status colors: unified 4-state system ────────────────────────

    /// Returns (background, text_color, label)
    pub fn status_disconnected(is_dark: bool) -> (Hsla, Hsla, &'static str) {
        let bg = if is_dark {
            hsla(0.0, 0.0, 0.25, 1.0)
        } else {
            hsla(210.0 / 360.0, 0.10, 0.86, 1.0)
        };
        let text = if is_dark {
            hsla(0.0, 0.0, 0.65, 1.0)
        } else {
            hsla(210.0 / 360.0, 0.08, 0.35, 1.0)
        };
        (bg, text, "Disconnected")
    }

    /// Returns (background, text_color, label)
    pub fn status_connecting(is_dark: bool) -> (Hsla, Hsla, &'static str) {
        let accent = Self::accent_color(is_dark);
        (accent.opacity(0.12), accent, "Connecting")
    }

    /// Returns (background, text_color, label)
    pub fn status_connected(is_dark: bool) -> (Hsla, Hsla, &'static str) {
        let bg = if is_dark {
            hsla(142.0 / 360.0, 0.40, 0.18, 1.0)
        } else {
            hsla(142.0 / 360.0, 0.45, 0.90, 1.0)
        };
        let text = if is_dark {
            hsla(142.0 / 360.0, 0.70, 0.65, 1.0)
        } else {
            hsla(142.0 / 360.0, 0.60, 0.28, 1.0)
        };
        (bg, text, "Connected")
    }

    /// Returns (background, text_color, label)
    #[allow(dead_code)]
    pub fn status_error(is_dark: bool) -> (Hsla, Hsla, &'static str) {
        let danger = Self::danger_color();
        let bg = if is_dark {
            hsla(0.0, 0.40, 0.20, 1.0)
        } else {
            hsla(0.0, 0.86, 0.94, 1.0)
        };
        let _ = danger;
        (bg, danger, "Error")
    }

    /// Disconnected dot color for sidebar — visible gray, not invisible
    pub fn disconnected_dot(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(0.0, 0.0, 0.40, 1.0)
        } else {
            hsla(210.0 / 360.0, 0.08, 0.72, 1.0)
        }
    }

    /// Inactive dot color for sidebar items
    pub fn inactive_dot(is_dark: bool) -> Hsla {
        if is_dark {
            hsla(0.0, 0.0, 0.40, 1.0)
        } else {
            hsla(0.0, 0.0, 0.83, 1.0)
        }
    }
}
