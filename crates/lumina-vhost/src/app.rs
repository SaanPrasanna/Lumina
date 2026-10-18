//! Projects that run their own dev server (Vite, Next.js, Nuxt, Astro,
//! SvelteKit, Django, FastAPI, Flask): which command starts one, on which port.
//! The web server then forwards `<name>.test` to it.

use std::path::Path;

use serde::Serialize;

/// Files that make a folder a PHP project, even when it also has a
/// `package.json` (Laravel and Symfony with Vite or Encore).
const PHP_MARKERS: [&str; 5] = [
    "composer.json",
    "index.php",
    "public/index.php",
    "web/index.php",
    "wp-config.php",
];

/// Dev servers listen on ports from this range, one fixed port per site.
pub const PORT_BASE: u16 = 41000;
pub const PORT_SPAN: u16 = 8000;

/// A dev server Lumina found in a project folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    /// Shown in the UI: "Next.js", "Vite", "Django", ...
    pub framework: String,
    /// Run in the project folder; `{port}` stands for the port.
    pub command: String,
}

/// The dev server of the project in `dir`, if it is a Node.js or Python web
/// app and not a PHP project.
pub fn detect(dir: &Path) -> Option<Detected> {
    if PHP_MARKERS.iter().any(|m| dir.join(m).is_file()) {
        return None;
    }
    node(dir).or_else(|| python(dir))
}

fn node(dir: &Path) -> Option<Detected> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("scripts")?.get("dev")?.as_str()?;
    let has = |dep: &str| {
        ["dependencies", "devDependencies"]
            .iter()
            .any(|section| json.get(section).and_then(|s| s.get(dep)).is_some())
    };
    // Most specific first: Nuxt, SvelteKit and Astro also depend on Vite.
    let vite_flags = "--port {port} --strictPort --host 127.0.0.1";
    let (framework, flags) = if has("next") {
        ("Next.js", "--port {port} --hostname 127.0.0.1")
    } else if has("nuxt") {
        ("Nuxt", "--port {port} --host 127.0.0.1")
    } else if has("astro") {
        ("Astro", "--port {port} --host 127.0.0.1")
    } else if has("@sveltejs/kit") {
        ("SvelteKit", vite_flags)
    } else if has("@react-router/dev") || has("@remix-run/dev") {
        ("React Router", vite_flags)
    } else if has("vite") {
        ("Vite", vite_flags)
    } else {
        return None;
    };
    // npm needs `--` before arguments for the script; pnpm, Yarn and Bun
    // pass them on as they are (pnpm would pass a `--` through literally).
    let run = if dir.join("pnpm-lock.yaml").is_file() {
        "pnpm run dev"
    } else if dir.join("yarn.lock").is_file() {
        "yarn run dev"
    } else if dir.join("bun.lock").is_file() || dir.join("bun.lockb").is_file() {
        "bun run dev"
    } else {
        "npm run dev --"
    };
    Some(Detected {
        framework: framework.into(),
        command: format!("{run} {flags}"),
    })
}

fn python(dir: &Path) -> Option<Detected> {
    let python = venv_python(dir).unwrap_or_else(|| "python".into());
    if dir.join("manage.py").is_file() {
        return Some(Detected {
            framework: "Django".into(),
            command: format!("{python} manage.py runserver 127.0.0.1:{{port}}"),
        });
    }
    let requirements: String = ["pyproject.toml", "requirements.txt"]
        .iter()
        .filter_map(|f| std::fs::read_to_string(dir.join(f)).ok())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    if requirements.contains("fastapi") {
        let module = [("main.py", "main"), ("app/main.py", "app.main")]
            .iter()
            .find(|(file, _)| dir.join(file).is_file())
            .map(|(_, module)| *module)?;
        return Some(Detected {
            framework: "FastAPI".into(),
            command: format!(
                "{python} -m uvicorn {module}:app --reload --host 127.0.0.1 --port {{port}}"
            ),
        });
    }
    if requirements.contains("flask") {
        let app = ["app.py", "wsgi.py"]
            .iter()
            .find(|f| dir.join(f).is_file())?
            .trim_end_matches(".py");
        return Some(Detected {
            framework: "Flask".into(),
            command: format!(
                "{python} -m flask --app {app} run --debug --host 127.0.0.1 --port {{port}}"
            ),
        });
    }
    None
}

/// The project's virtual environment interpreter (`.venv`, `venv`), quoted
/// for the shell, if there is one.
fn venv_python(dir: &Path) -> Option<String> {
    let exe = if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    };
    [".venv", "venv"]
        .iter()
        .map(|v| Path::new(v).join(exe))
        .find(|rel| dir.join(rel).is_file())
        .map(|rel| format!("\"{}\"", rel.to_string_lossy().replace('\\', "/")))
}

/// Runtime versions a project asks for in its own files: `.nvmrc` or
/// `.node-version`, `.python-version`, else `.tool-versions` (asdf, mise).
/// Each comes with the file it was read from. Aliases such as `lts/*` or
/// `system` are not versions and are left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Requested {
    pub node: Option<(String, &'static str)>,
    pub python: Option<(String, &'static str)>,
}

pub fn requested_versions(dir: &Path) -> Requested {
    let first_line = |file: &str| {
        let text = std::fs::read_to_string(dir.join(file)).ok()?;
        version(
            text.lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))?,
        )
    };
    let mut out = Requested {
        node: [".nvmrc", ".node-version"]
            .into_iter()
            .find_map(|f| first_line(f).map(|v| (v, f))),
        python: first_line(".python-version").map(|v| (v, ".python-version")),
    };
    if let Ok(text) = std::fs::read_to_string(dir.join(".tool-versions")) {
        for line in text.lines() {
            let mut words = line.split_whitespace();
            let (Some(tool), Some(v)) = (words.next(), words.next()) else {
                continue;
            };
            let Some(v) = version(v) else { continue };
            match tool {
                "nodejs" | "node" if out.node.is_none() => out.node = Some((v, ".tool-versions")),
                "python" if out.python.is_none() => out.python = Some((v, ".tool-versions")),
                _ => {}
            }
        }
    }
    out
}

/// "v22.11.0" → "22.11.0"; `None` for anything that does not start with a digit.
fn version(text: &str) -> Option<String> {
    let v = text.trim().trim_start_matches('v');
    v.starts_with(|c: char| c.is_ascii_digit()).then(|| {
        v.chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect()
    })
}

/// The port a site's dev server gets when none is configured: always the same
/// for the same folder name, so the web server's config does not depend on
/// whether the dev server runs.
pub fn stable_port(name: &str) -> u16 {
    // FNV-1a: tiny, and stable across Rust versions (unlike DefaultHasher).
    let mut hash: u32 = 0x811c_9dc5;
    for byte in name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    PORT_BASE + (hash % u32::from(PORT_SPAN)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        for (file, text) in files {
            let path = tmp.path().join(file);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, text).expect("write");
        }
        tmp
    }

    fn package(deps: &str) -> String {
        format!(r#"{{"scripts": {{"dev": "x"}}, "devDependencies": {{{deps}}}}}"#)
    }

    #[test]
    fn detects_node_frameworks_and_their_port_flags() {
        let cases = [
            (
                r#""next": "16""#,
                "Next.js",
                "npm run dev -- --port {port} --hostname 127.0.0.1",
            ),
            (
                r#""nuxt": "4", "vite": "7""#,
                "Nuxt",
                "npm run dev -- --port {port} --host 127.0.0.1",
            ),
            (
                r#""astro": "5""#,
                "Astro",
                "npm run dev -- --port {port} --host 127.0.0.1",
            ),
            (
                r#""@sveltejs/kit": "2", "vite": "7""#,
                "SvelteKit",
                "npm run dev -- --port {port} --strictPort --host 127.0.0.1",
            ),
            (
                r#""vite": "7""#,
                "Vite",
                "npm run dev -- --port {port} --strictPort --host 127.0.0.1",
            ),
        ];
        for (deps, framework, command) in cases {
            let dir = project(&[("package.json", &package(deps))]);
            let found = detect(dir.path()).expect(framework);
            assert_eq!(found.framework, framework);
            assert_eq!(found.command, command);
        }
    }

    #[test]
    fn uses_the_projects_package_manager() {
        let dir = project(&[
            ("package.json", &package(r#""vite": "7""#)),
            ("pnpm-lock.yaml", ""),
        ]);
        assert!(
            detect(dir.path())
                .expect("vite")
                .command
                .starts_with("pnpm run dev --port")
        );
    }

    #[test]
    fn php_projects_with_vite_stay_php() {
        let dir = project(&[
            ("package.json", &package(r#""vite": "7""#)),
            ("composer.json", "{}"),
        ]);
        assert_eq!(detect(dir.path()), None);
    }

    #[test]
    fn needs_a_dev_script_and_a_known_framework() {
        let no_script = project(&[("package.json", r#"{"devDependencies": {"vite": "7"}}"#)]);
        assert_eq!(detect(no_script.path()), None);
        let unknown = project(&[("package.json", &package(r#""express": "5""#))]);
        assert_eq!(detect(unknown.path()), None);
        let broken = project(&[("package.json", "{ not json")]);
        assert_eq!(detect(broken.path()), None);
    }

    #[test]
    fn detects_python_apps_and_their_virtualenv() {
        let django = project(&[("manage.py", "")]);
        assert_eq!(
            detect(django.path()).expect("django").command,
            "python manage.py runserver 127.0.0.1:{port}"
        );

        let venv = if cfg!(windows) {
            ".venv/Scripts/python.exe"
        } else {
            ".venv/bin/python"
        };
        let fastapi = project(&[
            ("requirements.txt", "FastAPI==0.120\nuvicorn"),
            ("app/main.py", ""),
            (venv, ""),
        ]);
        let found = detect(fastapi.path()).expect("fastapi");
        assert_eq!(found.framework, "FastAPI");
        assert_eq!(
            found.command,
            format!("\"{venv}\" -m uvicorn app.main:app --reload --host 127.0.0.1 --port {{port}}")
        );

        let flask = project(&[
            ("pyproject.toml", "dependencies = [\"flask\"]"),
            ("app.py", ""),
        ]);
        assert_eq!(detect(flask.path()).expect("flask").framework, "Flask");
    }

    #[test]
    fn reads_version_files_and_ignores_aliases() {
        let dir = project(&[(".nvmrc", "v22.11.0\n"), (".python-version", "3.12\n")]);
        let found = requested_versions(dir.path());
        assert_eq!(found.node, Some(("22.11.0".into(), ".nvmrc")));
        assert_eq!(found.python, Some(("3.12".into(), ".python-version")));

        let alias = project(&[
            (".nvmrc", "lts/*"),
            (".tool-versions", "nodejs 20.18.1\npython 3.13.0\n"),
        ]);
        let found = requested_versions(alias.path());
        assert_eq!(found.node, Some(("20.18.1".into(), ".tool-versions")));
        assert_eq!(found.python, Some(("3.13.0".into(), ".tool-versions")));

        assert_eq!(
            requested_versions(project(&[]).path()),
            Requested::default()
        );
    }

    #[test]
    fn stable_ports_stay_in_range_and_differ_by_name() {
        for name in ["blog", "shop", "a", "my-very-long-project-name"] {
            let port = stable_port(name);
            assert!((PORT_BASE..PORT_BASE + PORT_SPAN).contains(&port));
            assert_eq!(port, stable_port(name));
        }
        assert_ne!(stable_port("blog"), stable_port("shop"));
    }
}
