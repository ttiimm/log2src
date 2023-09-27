use cursive::{Cursive, CursiveRunnable};
use cursive::event::EventResult;
use cursive::views::*;
use cursive::theme::{BaseColor, Color};
use cursive::traits::*;
use cursive::utils::markup::StyledString;

use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

use logdbg::{LogRef, SourceRef};


pub fn start(source: &str, log_mappings: &Vec<(&LogRef<'_>, Option<&SourceRef<'_>>)>) {
    let mut siv = cursive::default();
    siv.add_global_callback('q', |s| s.quit());

    let num_lines = source.split("\n").collect::<Vec<_>>().len();
    let source_view = make_source_view(&mut siv, source, num_lines);
    let log_view = make_log_view(num_lines, log_mappings);
        
    let top_pane = LinearLayout::horizontal()
                .child(source_view)
                .child(log_view);

    siv.add_layer(LinearLayout::vertical()
    .child(top_pane));

    siv.run();
}

fn make_log_view(num_lines: usize, log_mappings: &Vec<(&LogRef<'_>, Option<&SourceRef<'_>>)>) -> LinearLayout {
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
        if lm.1.is_some() {
            select_view.add_item(format!("{}", i), format!("{}", lm.1.unwrap().line_no));
        }
    }

    // set up 'j' and 'k' keys for navigation
    let select_view = OnEventView::new(select_view)
        .on_pre_event_inner('k', |s, _| {
            let cb = s.select_up(1);
            Some(EventResult::Consumed(Some(cb)))
        })
        .on_pre_event_inner('j', |s, _| {
            let cb = s.select_down(1);
            Some(EventResult::Consumed(Some(cb)))
        });

    let selector = LinearLayout::vertical()
            .child(DummyView.fixed_height(1))
            .child(select_view);

    let logs = log_mappings.iter()
        .map(|e| e.0.text)
        .collect::<Vec<&str>>()
        .join("\n");
    LinearLayout::horizontal()
        .child(selector)
        .child(Dialog::around(
            TextView::new(logs)
                    .fixed_width(120)
                    .full_height()
                    .scrollable())
            .title("Logs")
            .button("Press 'q' to quit", |s| s.quit()))
}


fn make_source_view(siv: &mut CursiveRunnable, source: &str, num_lines: usize) -> LinearLayout {
    let themes = ThemeSet::load_defaults();
    let theme = &themes.themes["Solarized (light)"];
    set_theme(siv, theme);
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let syntax = syntax_set.find_syntax_by_token("rs").unwrap();
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
    // Parse the content and highlight it
    let styled = cursive_syntect::parse(source, &mut highlighter, &syntax_set)
        .unwrap();

    let mut gutter_view = LinearLayout::vertical();
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

    LinearLayout::horizontal()
        .child(gutter_view)
        .child(Dialog::around(
            TextView::new(styled)
                .fixed_width(120)
                .full_height()
                .scrollable())
            .title("Source Code"))
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
