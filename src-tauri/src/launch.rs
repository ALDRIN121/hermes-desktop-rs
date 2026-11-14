//! Launch presentation, shared by initial and forwarded invocations.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Main,
    Task,
    Workspace,
}

#[derive(Debug, PartialEq)]
pub struct Launch {
    pub view: View,
    pub task_url: String,
    pub width: f64,
    pub height: f64,
    /// In-app route for the main window, so a review can open one screen directly.
    pub route: Option<String>,
}

/// Accepts `--route /settings?tab=config%3Avoice` or `--route '#/settings…'` and
/// returns a hash route. Anything outside a plain path/query is refused, so the
/// flag can never smuggle a script URL into the window.
pub fn route(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let path = trimmed.strip_prefix('#').unwrap_or(trimmed);
    let allowed = |c: char| c.is_ascii_alphanumeric() || "-._~/?=&%:".contains(c);
    if !path.starts_with('/') || path.starts_with("//") || path.len() > 512 || !path.chars().all(allowed) {
        return None;
    }
    Some(format!("#{path}"))
}

pub fn parse(args: &[String]) -> Launch {
    let has = |flag: &str| {
        args.iter()
            .any(|arg| arg == flag || arg == &format!("--{flag}"))
    };
    let value = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag || arg == &format!("--{flag}"))
            .and_then(|index| args.get(index + 1))
    };
    let view = if has("task") {
        View::Task
    } else if has("workspace") {
        View::Workspace
    } else {
        View::Main
    };
    let mut query = Vec::new();
    for flag in ["selftest", "audit"] {
        if has(flag) {
            query.push(format!("{flag}=1"));
        }
    }
    for flag in ["session", "repo"] {
        if let Some(value) = value(flag) {
            query.push(format!("{flag}={}", urlencoding::encode(value)));
        }
    }
    let size = |flag: &str, default: f64, minimum: f64, maximum: f64| {
        value(flag)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(minimum, maximum))
            .unwrap_or(default)
    };
    Launch {
        view,
        task_url: if query.is_empty() {
            "task.html".into()
        } else {
            format!("task.html?{}", query.join("&"))
        },
        width: size("width", 1440.0, 900.0, 3840.0),
        height: size("height", 900.0, 600.0, 2160.0),
        route: value("route").map(String::as_str).and_then(route),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn normal_and_classic_alias_launch_the_unified_app() {
        assert_eq!(parse(&args(&["hermes"])).view, View::Main);
        assert_eq!(parse(&args(&["hermes", "--classic"])).view, View::Main);
        assert_eq!(parse(&args(&["hermes", "workspace"])).view, View::Workspace);
    }

    #[test]
    fn forwarded_task_arguments_are_read_from_that_invocation() {
        let launch = parse(&args(&[
            "hermes",
            "--task",
            "--session",
            "new session",
            "--repo",
            "/tmp/my repo",
            "--width",
            "1024",
            "--height",
            "700",
        ]));
        assert_eq!(launch.view, View::Task);
        assert_eq!(
            launch.task_url,
            "task.html?session=new%20session&repo=%2Ftmp%2Fmy%20repo"
        );
        assert_eq!((launch.width, launch.height), (1024.0, 700.0));
        assert_eq!(parse(&args(&["hermes", "--task"])).task_url, "task.html");
    }

    #[test]
    fn window_sizes_are_finite_and_bounded() {
        let launch = parse(&args(&[
            "hermes", "--task", "--width", "NaN", "--height", "0",
        ]));
        assert_eq!((launch.width, launch.height), (1440.0, 600.0));
    }

    #[test]
    fn a_review_route_opens_one_main_window_screen() {
        let launch = parse(&args(&[
            "hermes",
            "--route",
            "/settings?tab=config%3Avoice&page=conversation",
        ]));
        assert_eq!(launch.view, View::Main);
        assert_eq!(
            launch.route.as_deref(),
            Some("#/settings?tab=config%3Avoice&page=conversation")
        );
        // An already-prefixed hash behaves the same, so both spellings work.
        assert_eq!(
            parse(&args(&["hermes", "--route", "#/settings"])).route.as_deref(),
            Some("#/settings")
        );
        assert_eq!(parse(&args(&["hermes"])).route, None);
    }

    #[test]
    fn a_route_cannot_carry_a_script_url() {
        for refused in [
            "javascript:alert(1)",
            "//evil.example/",
            "/settings with space",
            "/settings\"><script>",
            "",
        ] {
            assert_eq!(route(refused), None, "accepted {refused:?}");
        }
    }
}
