use gpui::*;
use gpui_component::label;
use gpui_component::v_flex;

use super::theme::AppColors;

/// Reusable section card wrapper for form sections.
/// Provides consistent styling: white card bg, border, rounded_lg, p_4, bold title.
pub fn section_card(title: &str, is_dark: bool) -> Div {
    let card_bg = AppColors::card_bg(is_dark);
    let border = AppColors::border_color(is_dark);
    let text = AppColors::primary_text(is_dark);

    v_flex()
        .gap_3()
        .p_4()
        .bg(card_bg)
        .border_1()
        .border_color(border)
        .rounded_lg()
        .child(
            label::Label::new(title.to_string())
                .text_size(rems(0.95))
                .font_weight(FontWeight::BOLD)
                .text_color(text),
        )
}
