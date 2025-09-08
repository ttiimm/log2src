pub fn enable_filters() -> insta::internals::SettingsBindDropGuard {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r#""examples(?:/|\\\\?)"#, "\"{example_dir}/");
    settings.add_filter(r#""tests(?:/|\\\\?)java(?:/|\\\\?)"#, "\"{java_dir}/");
    settings.add_filter(r#"(?:[ \w\.]+) (\(os error \d+\))"#, " {errmsg} $1");
    settings.bind_to_scope()
}
