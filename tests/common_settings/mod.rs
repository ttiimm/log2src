use assert_cmd::prelude::CommandCargoExt;
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub fn enable_filters(home_path: &Path) -> insta::internals::SettingsBindDropGuard {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r#""[^"]*examples(?:/|\\\\?)"#, "\"{example_dir}/");
    settings.add_filter(r#""[^"]*tests(?:/|\\\\?)java(?:/|\\\\?)"#, "\"{java_dir}/");
    settings.add_filter(r#"(?:[ \w\.]+) (\(os error \d+\))"#, " {errmsg} $1");
    settings.add_filter(r#""[^"]*cache\.[0-9a-f]+"#, "\"cache.XXX");
    settings.add_filter(
        regex::escape(home_path.to_string_lossy().as_ref()).as_str(),
        "{home}",
    );
    settings.bind_to_scope()
}

pub struct TempHome {
    location: TempDir,
}

impl TempHome {
    pub fn new() -> Self {
        Self {
            location: tempfile::tempdir().unwrap(),
        }
    }

    pub fn set_vars_in(&self, cmd: &mut Command) {
        cmd.env("HOME", self.location.path());
        cmd.env("XDG_CONFIG_HOME", self.location.path());
        cmd.env("USERPROFILE", self.location.path());
    }
}

pub struct CommandGuard {
    _filter_guard: insta::internals::SettingsBindDropGuard,
    home_guard: TempHome,
    pub cmd: Command,
}

impl CommandGuard {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let home_guard = TempHome::new();
        let _filter_guard = enable_filters(home_guard.location.path());
        let mut cmd = Command::cargo_bin("log2src")?;
        cmd.env("COLS", "1000");
        home_guard.set_vars_in(&mut cmd);
        Ok(Self {
            _filter_guard,
            home_guard,
            cmd,
        })
    }

    #[allow(dead_code)]
    pub fn home_path(&self) -> &Path {
        self.home_guard.location.path()
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.cmd.arg(arg);
        self
    }
}
