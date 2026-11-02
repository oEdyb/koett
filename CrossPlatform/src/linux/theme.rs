use gtk4::{self as gtk, prelude::*};

const LIGHT: &str = "
@define-color koett_stone_50 #FAFAF9;
@define-color koett_stone_0 #FFFFFF;
@define-color koett_stone_900 #1C1917;
@define-color koett_stone_500 #75706B;
@define-color koett_stone_200 #E7E5E4;
@define-color koett_selection #F2E8E5;
@define-color koett_focus_ring #B84236;
";
const DARK: &str = "
@define-color koett_stone_50 #181716;
@define-color koett_stone_0 #242220;
@define-color koett_stone_900 #FAFAF9;
@define-color koett_stone_500 #A8A29E;
@define-color koett_stone_200 #403C38;
@define-color koett_selection #3B302C;
@define-color koett_focus_ring #E1887D;
";
const NATIVE: &str = "
@define-color koett_stone_50 @theme_bg_color;
@define-color koett_stone_0 @theme_base_color;
@define-color koett_stone_900 @theme_fg_color;
@define-color koett_stone_500 @theme_fg_color;
@define-color koett_stone_200 @theme_fg_color;
@define-color koett_selection @theme_selected_bg_color;
@define-color koett_focus_ring @theme_selected_bg_color;
";

pub(super) fn install(window: &gtk::ApplicationWindow) {
    let provider = gtk::CssProvider::new();
    let settings = gtk::Settings::for_display(&WidgetExt::display(window));
    let refresh = {
        let window = window.downgrade();
        let provider = provider.clone();
        move |settings: &gtk::Settings| {
            let Some(window) = window.upgrade() else {
                return;
            };
            let theme = std::env::var("GTK_THEME")
                .ok()
                .unwrap_or_else(|| settings.gtk_theme_name().unwrap_or_default().to_string())
                .to_lowercase();
            let high_contrast = theme.contains("highcontrast")
                || theme.contains("high-contrast")
                || setting_is(settings, "gtk-interface-contrast", "more");
            let dark = theme.ends_with(":dark")
                || setting_is(settings, "gtk-interface-color-scheme", "dark")
                || settings.is_gtk_application_prefer_dark_theme()
                || window
                    .style_context()
                    .lookup_color("theme_bg_color")
                    .is_some_and(|color| {
                        color.red() * 0.2126 + color.green() * 0.7152 + color.blue() * 0.0722 < 0.5
                    });
            let palette = if high_contrast {
                NATIVE
            } else if dark {
                DARK
            } else {
                LIGHT
            };
            let primary = if high_contrast {
                "@define-color koett_primary @theme_selected_bg_color;\n@define-color koett_on_primary @theme_selected_fg_color;\n@define-color koett_selected_text @theme_selected_fg_color;"
            } else {
                "@define-color koett_primary #B84236;\n@define-color koett_on_primary #FFFFFF;\n@define-color koett_selected_text @koett_stone_900;"
            };
            provider.load_from_data(&format!(
                "{palette}\n{primary}\n{}",
                include_str!("theme.css")
            ));
        }
    };
    refresh(&settings);
    settings.connect_notify_local(None, move |settings, property| {
        if matches!(
            property.name(),
            "gtk-theme-name"
                | "gtk-application-prefer-dark-theme"
                | "gtk-interface-color-scheme"
                | "gtk-interface-contrast"
        ) {
            let settings = settings.clone();
            let refresh = refresh.clone();
            gtk::glib::idle_add_local_once(move || refresh(&settings));
        }
    });
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(window),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn setting_is(settings: &gtk::Settings, property: &str, expected: &str) -> bool {
    settings.find_property(property).is_some()
        && gtk::glib::EnumValue::from_value(&settings.property_value(property))
            .is_some_and(|(_, value)| value.nick() == expected)
}
