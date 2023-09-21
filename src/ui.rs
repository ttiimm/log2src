use cursive::align::HAlign;
use cursive::views::{Dialog, DummyView, LinearLayout, TextView};
use cursive::traits::*;

pub fn start(source: &str) {
    let mut siv = cursive::default();

    siv.load_theme_file("src/assets/style.toml").unwrap();

    siv.add_layer(
        Dialog::around(
            LinearLayout::vertical()
                    .child(TextView::new("Source Code").h_align(HAlign::Center))
                    .child(DummyView.fixed_height(1))
                    .child(TextView::new(source))
                    .fixed_width(120),
        ).button("Press 'enter' to quit", |s| s.quit())
    );

    siv.run();
}
