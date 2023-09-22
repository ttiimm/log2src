use cursive::Cursive;
use cursive::views::*;
use cursive::theme::BaseColor;
use cursive::theme::Color;
use cursive::traits::*;
use cursive::utils::markup::StyledString;

use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

use logdbg::{LogRef};


pub fn start(source: &str, log_mappings: &Vec<(&LogRef<'_>, usize)>) {
    let mut siv = cursive::default();
    siv.add_global_callback('q', |s| s.quit());

    let themes = ThemeSet::load_defaults();
    let theme = &themes.themes["Solarized (light)"];
    set_theme(&mut siv, theme);
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let syntax = syntax_set.find_syntax_by_token("rs").unwrap();
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
    // Parse the content and highlight it
    let styled = cursive_syntect::parse(source, &mut highlighter, &syntax_set).unwrap();

    // Set up the source view
    let mut gutter_view = LinearLayout::vertical();
    let num_lines = source.split("\n").collect::<Vec<_>>().len();
    for i in 0..num_lines {
        let value = if i != 0 {
            format!("{:-<5}\n", i)
        } else {
            String::from("     ")
        };
        gutter_view.add_child(TextView::new(value)
            .with_name(format!("line{}", i))
        );
    }

    let gutter_view = gutter_view.with_name("gutter");

    let source_view = LinearLayout::horizontal()
        .child(gutter_view)
        .child(Dialog::around(
            TextView::new(styled)
                    .fixed_width(120)
                    .full_height()
                    .scrollable())
                .title("Source Code"));


    // Set up the log view
    let mut select_view = SelectView::<String>::new()
        .autojump()
        .on_select(move |s: &mut Cursive, line_no: &String| {
            for i in 0..num_lines {
                let value = if i != 0 {
                    StyledString::plain(format!("{:-<5}\n", i))
                } else {
                    StyledString::plain(String::from("     "))
                };

                let mut view: ViewRef<TextView> = s.find_name(&format!("line{}", i)).unwrap();
                view.set_content(value);
            }

            let mut view: ViewRef<TextView> = s.find_name(&format!("line{}", line_no)).unwrap();
            let styled = StyledString::styled(format!("{:><5}\n", line_no), 
                Color::Dark(BaseColor::Red));
            view.set_content(styled);
        });
    for (i, lm) in log_mappings.iter().enumerate() {
        select_view.add_item(format!("{}", i), format!("{}", lm.1));
    }
    
    let selector = LinearLayout::vertical()
            .child(DummyView.fixed_height(1))
            .child(select_view);

    let logs = log_mappings.iter()
        .map(|e| e.0.text)
        .collect::<Vec<&str>>()
        .join("\n");
    let log_view = LinearLayout::horizontal()
        .child(selector)
        .child(Dialog::around(
            TextView::new(logs)
                    .fixed_width(120)
                    .full_height()
                    .scrollable())
            .title("Logs")
            .button("Press 'q' to quit", |s| s.quit()));
        
    let top_pane = LinearLayout::horizontal()
                .child(source_view)
                .child(log_view);

    siv.add_layer(LinearLayout::vertical()
        .child(top_pane));

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
