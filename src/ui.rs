use cursive::align::HAlign;
use cursive::views::{Dialog, DummyView, LinearLayout, TextView};
use cursive::traits::*;

use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;


pub fn start(source: &str, logs: &str) {
    let mut siv = cursive::default();
    let themes = ThemeSet::load_defaults();
    let theme = &themes.themes["Solarized (light)"];
    set_theme(&mut siv, theme);
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let syntax = syntax_set.find_syntax_by_token("rs").unwrap();
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
    // Parse the content and highlight it
    let styled = cursive_syntect::parse(source, &mut highlighter, &syntax_set).unwrap();

    let source_view = Dialog::around(
        LinearLayout::vertical()
                .child(DummyView.fixed_height(1))
                .child(TextView::new(styled))
                .fixed_width(120)
                .scrollable()
        ).title("Source Code");
    let log_view = Dialog::around(
        LinearLayout::vertical()
            .child(DummyView.fixed_height(1))
            .child(TextView::new(logs))
            .fixed_width(120)
            .scrollable()
        ).title("Logs");
    let top_pane = LinearLayout::horizontal()
                .child(source_view)
                .child(log_view);
    siv.add_layer(top_pane);

    siv.run();
}

fn set_theme(siv: &mut cursive::CursiveRunnable, theme: &Theme) {
    siv.load_theme_file("src/assets/style.toml").unwrap();

    // Apply some settings from the theme to cursive's own theme. This probably could be done in
    // the style.toml, but copy-pasta'd from the cursive-syntect lib
    siv.with_theme(|t| {
        if let Some(background) = theme
            .settings
            .background
            .map(cursive_syntect::translate_color)
        {
            // t.palette[cursive::theme::PaletteColor::Background] = background;
            t.palette[cursive::theme::PaletteColor::View] = background;
        }
        if let Some(foreground) = theme
            .settings
            .foreground
            .map(cursive_syntect::translate_color)
        {
            t.palette[cursive::theme::PaletteColor::Primary] = foreground;
            t.palette[cursive::theme::PaletteColor::TitlePrimary] = foreground;
        }

        if let Some(highlight) = theme
            .settings
            .highlight
            .map(cursive_syntect::translate_color)
        {
            t.palette[cursive::theme::PaletteColor::Highlight] = highlight;
        }
    });
}
