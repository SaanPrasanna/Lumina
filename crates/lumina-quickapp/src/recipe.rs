//! The recipe model (`resources/quickapp/recipes.toml`).
//!
//! A recipe is data: what it needs (PHP, Composer, Node.js, a database),
//! options the user picks (a starter kit, a template), and steps run in
//! order. Strings in steps may use placeholders, expanded by [`Vars`]:
//!
//! - `{name}`, `{dir}`, `{www}`, `{domain}`, `{url}`, `{db}`, `{db_host}`,
//!   `{db_port}`, `{db_user}`, `{db_password}`, `{composer_home}`, ... — the
//!   values the app provides for this project;
//! - `{kit}` — the value chosen for option `kit`; `{kit.flag}` — a variable
//!   of that choice;
//! - `{random:64}` — fresh random letters and digits (e.g. WordPress salts).
//!
//! A command argument that expands to nothing is dropped, so an option can add
//! a flag or not (`"{kit.flag}"`).

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Programs a `run` step may start. The app maps each to Lumina's own copy.
pub const PROGRAMS: [&str; 6] = ["php", "composer", "node", "npm", "npx", "mysql"];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeFile {
    #[serde(rename = "recipe")]
    pub recipes: Vec<Recipe>,
}

/// What a recipe needs before it can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Need {
    Php,
    Composer,
    Node,
    /// The configured database server, running.
    Database,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    /// `framework`, `cms`, `starter`.
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub needs: Vec<Need>,
    /// Document root inside the project when automatic detection would not
    /// find it (e.g. `dist`). Saved as the site's override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docroot: Option<String>,
    /// Environment for every `run` step (placeholders allowed).
    #[serde(default, skip_serializing)]
    pub env: BTreeMap<String, String>,
    #[serde(default, rename = "option")]
    pub options: Vec<RecipeOption>,
    #[serde(default, rename = "step", skip_serializing)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeOption {
    pub id: String,
    pub label: String,
    /// A choice's `value`; the first choice when empty.
    #[serde(default)]
    pub default: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub label: String,
    /// Extra needs when this choice is picked (e.g. Node.js for a starter kit).
    #[serde(default)]
    pub needs: Vec<Need>,
    /// `{option.var}` values.
    #[serde(default, skip_serializing)]
    pub vars: BTreeMap<String, String>,
}

/// One step: a label, an optional condition, an action, and steps to try
/// instead if it fails.
#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    /// Shown in the log ("Creating the database").
    pub label: String,
    /// `option == value` or `option != value`.
    #[serde(default)]
    pub when: Option<String>,
    /// Run instead when this step fails (not when it is cancelled). The
    /// project folder is removed first, so the fallback starts clean.
    #[serde(default)]
    pub fallback: Vec<Step>,
    /// A failure is noted in the log and the run goes on (e.g. updating the
    /// Laravel installer while offline, when a copy is already there).
    #[serde(default)]
    pub optional: bool,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Runs one of [`PROGRAMS`] with `args`.
    Run {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        /// Default: the project folder once it exists, else the projects folder.
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    /// Fetches `url` (https only), or the newest release asset of a GitHub
    /// repository matching `asset` (`*` wildcards), into the job's work
    /// folder as `save_as`.
    Download {
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        github: Option<String>,
        #[serde(default)]
        asset: Option<String>,
        save_as: String,
    },
    /// Unpacks a downloaded zip / tar.gz into `to`, which must not exist.
    Extract {
        from: String,
        to: String,
        #[serde(default)]
        strip: u32,
    },
    /// Creates database `name` (utf8mb4) on Lumina's server. An existing,
    /// empty database is reused; one with tables is refused.
    CreateDatabase { name: String },
    /// Sets `KEY=value` lines in a dotenv-style file (also `key = value` and
    /// commented-out keys), appending keys that are missing. `copy_from`
    /// seeds the file when it does not exist yet.
    PatchEnv {
        file: String,
        #[serde(default)]
        copy_from: Option<String>,
        set: BTreeMap<String, String>,
    },
    /// Renders a file from `resources/quickapp/templates` with placeholders.
    TemplateFile { template: String, to: String },
}

impl RecipeFile {
    pub fn parse(text: &str) -> Result<Self> {
        let file: Self = toml::from_str(text).map_err(|e| Error::Recipe(e.to_string()))?;
        let mut ids = HashSet::new();
        for recipe in &file.recipes {
            if !ids.insert(recipe.id.as_str()) {
                return Err(Error::Recipe(format!(
                    "recipe `{}` is defined twice",
                    recipe.id
                )));
            }
            recipe.validate()?;
        }
        Ok(file)
    }

    pub fn recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|r| r.id == id)
    }
}

impl Recipe {
    fn validate(&self) -> Result<()> {
        let bad = |msg: String| Err(Error::Recipe(format!("recipe `{}`: {msg}", self.id)));
        if self.steps.is_empty() {
            return bad("has no steps".into());
        }
        for option in &self.options {
            if option.choices.is_empty() {
                return bad(format!("option `{}` has no choices", option.id));
            }
            if !option.default.is_empty() && option.choice(&option.default).is_none() {
                return bad(format!(
                    "option `{}` defaults to an unknown choice",
                    option.id
                ));
            }
        }
        fn walk(recipe: &Recipe, steps: &[Step]) -> std::result::Result<(), String> {
            for step in steps {
                if let Some(when) = &step.when {
                    let (option, _, value) = parse_when(when)
                        .ok_or_else(|| format!("bad condition `{when}` (use `option == value`)"))?;
                    let opt = recipe
                        .option(option)
                        .ok_or_else(|| format!("condition `{when}` names an unknown option"))?;
                    if opt.choice(value).is_none() {
                        return Err(format!("condition `{when}` names an unknown choice"));
                    }
                }
                match &step.action {
                    Action::Run { program, .. } if !PROGRAMS.contains(&program.as_str()) => {
                        return Err(format!("unknown program `{program}`"));
                    }
                    Action::Download {
                        url, github, asset, ..
                    } => match (url, github, asset) {
                        (Some(url), None, None) if url.starts_with("https://") => {}
                        (None, Some(repo), Some(_)) if repo.split('/').count() == 2 => {}
                        _ => {
                            return Err(format!(
                                "step `{}` needs an https `url`, or `github` = \"owner/repo\" with `asset`",
                                step.label
                            ));
                        }
                    },
                    Action::TemplateFile { template, .. } if !is_relative_plain(template) => {
                        return Err(format!("bad template path `{template}`"));
                    }
                    _ => {}
                }
                walk(recipe, &step.fallback)?;
            }
            Ok(())
        }
        walk(self, &self.steps).or_else(bad)
    }

    pub fn option(&self, id: &str) -> Option<&RecipeOption> {
        self.options.iter().find(|o| o.id == id)
    }

    /// The chosen value of every option: `chosen` where valid, else the default.
    pub fn resolve_options(
        &self,
        chosen: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        if let Some(id) = chosen.keys().find(|id| self.option(id).is_none()) {
            return Err(Error::Recipe(format!("{} has no option `{id}`", self.name)));
        }
        for option in &self.options {
            let value = match chosen.get(&option.id) {
                Some(v) if option.choice(v).is_some() => v.clone(),
                Some(v) => {
                    return Err(Error::Recipe(format!(
                        "`{v}` is not a choice for {}",
                        option.label
                    )));
                }
                None => option.default_value().to_owned(),
            };
            out.insert(option.id.clone(), value);
        }
        Ok(out)
    }

    /// Needs of the recipe plus those of the chosen options.
    pub fn needs_for(&self, options: &BTreeMap<String, String>) -> Vec<Need> {
        let mut needs = self.needs.clone();
        for option in &self.options {
            if let Some(choice) = options.get(&option.id).and_then(|v| option.choice(v)) {
                needs.extend(choice.needs.iter().copied());
            }
        }
        let mut seen = HashSet::new();
        needs.retain(|n| seen.insert(*n));
        needs
    }
}

impl RecipeOption {
    pub fn choice(&self, value: &str) -> Option<&Choice> {
        self.choices.iter().find(|c| c.value == value)
    }

    pub fn default_value(&self) -> &str {
        if self.default.is_empty() {
            &self.choices[0].value
        } else {
            &self.default
        }
    }
}

/// `kit == none` → (`kit`, `==`, `none`).
fn parse_when(when: &str) -> Option<(&str, &str, &str)> {
    for op in ["==", "!="] {
        if let Some((a, b)) = when.split_once(op) {
            let (a, b) = (a.trim(), b.trim());
            if !a.is_empty() && !b.is_empty() {
                return Some((a, op, b));
            }
        }
    }
    None
}

/// Whether `step` runs with `options`.
pub fn applies(step: &Step, options: &BTreeMap<String, String>) -> bool {
    let Some(when) = &step.when else {
        return true;
    };
    let Some((option, op, value)) = parse_when(when) else {
        return false;
    };
    let chosen = options.get(option).map(String::as_str);
    (chosen == Some(value)) == (op == "==")
}

/// A relative path without `..`, `.` or a drive: safe to join to a folder.
pub fn is_relative_plain(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['/', '\\'])
        && !path.contains(':')
        && path
            .split(['/', '\\'])
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Placeholder values for one project.
pub struct Vars<'a> {
    pub values: &'a BTreeMap<String, String>,
    pub recipe: &'a Recipe,
    pub options: &'a BTreeMap<String, String>,
}

impl Vars<'_> {
    /// Expands `{...}` placeholders in `text`. Braces that do not hold a
    /// placeholder name (PHP's `{$x}`, JSON) are left alone; an unknown name
    /// is an error, so a typo in a recipe never reaches a file.
    pub fn expand(&self, text: &str) -> Result<String> {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            let key = after
                .find('}')
                .map(|end| &after[..end])
                .filter(|k| is_placeholder(k));
            match key {
                Some(key) => {
                    out.push_str(&self.lookup(key)?);
                    rest = &after[key.len() + 1..];
                }
                None => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        Ok(out)
    }

    fn lookup(&self, key: &str) -> Result<String> {
        if let Some(len) = key.strip_prefix("random:") {
            let len: usize = len
                .parse()
                .ok()
                .filter(|n| (1..=256).contains(n))
                .ok_or_else(|| Error::Recipe(format!("bad placeholder {{{key}}}")))?;
            return Ok(lumina_ssl::random_token(len));
        }
        if let Some(value) = self.values.get(key) {
            return Ok(value.clone());
        }
        let (option, var) = key.split_once('.').unwrap_or((key, ""));
        if let (Some(opt), Some(value)) = (self.recipe.option(option), self.options.get(option)) {
            if var.is_empty() {
                return Ok(value.clone());
            }
            if let Some(v) = opt.choice(value).and_then(|c| c.vars.get(var)) {
                return Ok(v.clone());
            }
            // A variable only some choices define is empty for the others.
            if opt.choices.iter().any(|c| c.vars.contains_key(var)) {
                return Ok(String::new());
            }
        }
        Err(Error::Recipe(format!("unknown placeholder {{{key}}}")))
    }
}

fn is_placeholder(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | ':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = include_str!("../../../resources/quickapp/recipes.toml");

    #[test]
    fn bundled_recipes_are_valid() {
        let file = RecipeFile::parse(BUNDLED).expect("recipes.toml");
        for id in [
            "laravel",
            "wordpress",
            "symfony",
            "codeigniter",
            "drupal",
            "joomla",
            "statamic",
            "php",
            "vite",
        ] {
            assert!(file.recipe(id).is_some(), "missing recipe {id}");
        }
    }

    fn sample() -> Recipe {
        RecipeFile::parse(
            r#"
[[recipe]]
id = "x"
name = "X"
description = "d"
needs = ["php"]

[[recipe.option]]
id = "kit"
label = "Kit"
default = "none"
choices = [
  { value = "none", label = "None" },
  { value = "react", label = "React", needs = ["node"], vars = { flag = "--react" } },
]

[[recipe.step]]
label = "one"
type = "run"
program = "composer"
args = ["new", "{name}", "{kit.flag}"]

[[recipe.step]]
label = "two"
when = "kit != none"
type = "create_database"
name = "{db}"
"#,
        )
        .expect("parse")
        .recipes
        .remove(0)
    }

    #[test]
    fn options_needs_and_conditions() {
        let recipe = sample();
        let defaults = recipe.resolve_options(&BTreeMap::new()).expect("defaults");
        assert_eq!(defaults["kit"], "none");
        assert_eq!(recipe.needs_for(&defaults), vec![Need::Php]);
        assert!(!applies(&recipe.steps[1], &defaults));

        let react = recipe
            .resolve_options(&BTreeMap::from([("kit".into(), "react".into())]))
            .expect("react");
        assert_eq!(recipe.needs_for(&react), vec![Need::Php, Need::Node]);
        assert!(applies(&recipe.steps[1], &react));

        assert!(
            recipe
                .resolve_options(&BTreeMap::from([("kit".into(), "svelte".into())]))
                .is_err()
        );
        assert!(
            recipe
                .resolve_options(&BTreeMap::from([("nope".into(), "x".into())]))
                .is_err()
        );
    }

    #[test]
    fn expansion() {
        let recipe = sample();
        let values = BTreeMap::from([("name".to_owned(), "blog".to_owned())]);
        let react = BTreeMap::from([("kit".to_owned(), "react".to_owned())]);
        let none = BTreeMap::from([("kit".to_owned(), "none".to_owned())]);
        let vars = Vars {
            values: &values,
            recipe: &recipe,
            options: &react,
        };
        assert_eq!(
            vars.expand("{name} {kit} {kit.flag}").expect("x"),
            "blog react --react"
        );
        assert_eq!(
            vars.expand("<?php if ($a) {$b} {}").expect("php"),
            "<?php if ($a) {$b} {}"
        );
        assert!(vars.expand("{nmae}").is_err());
        assert_eq!(vars.expand("{random:12}").expect("random").len(), 12);
        assert_ne!(
            vars.expand("{random:32}").ok(),
            vars.expand("{random:32}").ok()
        );
        let vars = Vars {
            values: &values,
            recipe: &recipe,
            options: &none,
        };
        assert_eq!(vars.expand("{kit.flag}").expect("empty"), "");
    }

    #[test]
    fn invalid_recipes_are_refused() {
        let base = "[[recipe]]\nid = \"x\"\nname = \"X\"\ndescription = \"d\"\n";
        for step in [
            "[[recipe.step]]\nlabel = \"a\"\ntype = \"run\"\nprogram = \"bash\"\n",
            "[[recipe.step]]\nlabel = \"a\"\ntype = \"download\"\nurl = \"http://x/a.zip\"\nsave_as = \"a.zip\"\n",
            "[[recipe.step]]\nlabel = \"a\"\ntype = \"template_file\"\ntemplate = \"../x\"\nto = \"y\"\n",
            "[[recipe.step]]\nlabel = \"a\"\nwhen = \"kit == x\"\ntype = \"create_database\"\nname = \"d\"\n",
        ] {
            assert!(
                RecipeFile::parse(&format!("{base}{step}")).is_err(),
                "{step}"
            );
        }
        assert!(RecipeFile::parse(base).is_err(), "no steps");
    }

    #[test]
    fn plain_relative_paths() {
        assert!(is_relative_plain("wordpress/wp-config.php"));
        for bad in ["", "/etc", "..\\x", "a/../b", "C:\\x", "a//b"] {
            assert!(!is_relative_plain(bad), "{bad}");
        }
    }
}
