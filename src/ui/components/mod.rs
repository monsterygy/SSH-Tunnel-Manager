// UI Components module

pub mod connection_header;
pub mod connection_sidebar;
pub mod section_card;
pub mod theme;
pub mod top_bar;
pub mod tunnel_fields;

pub use connection_header::{render_connection_header, render_password_section};
pub use connection_sidebar::render_connection_sidebar;
pub use section_card::section_card;
pub use theme::AppColors;
pub use top_bar::render_top_bar;
pub use tunnel_fields::render_tunnel_card;
