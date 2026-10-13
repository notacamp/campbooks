//! Text edits that register a new module or feature in existing files.
//!
//! Each edit is a pure function from the file's text to its new text, so the
//! generators can compute every change before writing anything, and each one
//! refuses (instead of duplicating) when its line is already there.

use anyhow::{Context as _, Result, bail};

/// The comment that marks where `cargo xtask new-module` registers a module
/// in the server and the client shell.
pub const MARKER: &str = "`cargo xtask new-module` inserts above this line.";

/// Insert `line` before the line holding [`MARKER`], with that line's
/// indentation. `file` names the file in errors.
pub fn insert_before_marker(text: &str, file: &str, line: &str) -> Result<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if lines.iter().any(|existing| existing.trim() == line.trim()) {
        bail!("{file} already contains `{}`", line.trim());
    }
    let markers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, existing)| existing.contains(MARKER))
        .map(|(index, _)| index)
        .collect();
    let [at] = markers.as_slice() else {
        bail!(
            "{file} must hold exactly one `{MARKER}` comment (found {}); restore it where the \
             module list ends",
            markers.len()
        );
    };
    let indent = indentation(&lines[*at]).to_owned();
    lines.insert(*at, format!("{indent}{}", line.trim()));
    Ok(join(&lines, text))
}

/// Insert `line` after the last line starting with `prefix` and containing
/// `containing` (an import block, a group of manifest entries).
pub fn insert_after_last(
    text: &str,
    file: &str,
    prefix: &str,
    containing: &str,
    line: &str,
) -> Result<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if lines.iter().any(|existing| existing.trim() == line.trim()) {
        bail!("{file} already contains `{}`", line.trim());
    }
    let at = lines
        .iter()
        .rposition(|existing| existing.starts_with(prefix) && existing.contains(containing))
        .with_context(|| {
            format!("{file} has no line starting with `{prefix}` that mentions `{containing}`")
        })?;
    lines.insert(at + 1, line.to_owned());
    Ok(join(&lines, text))
}

/// Add `<dependency>.workspace = true` to a manifest's `[dependencies]`,
/// keeping the section's alphabetical order.
pub fn add_dependency(manifest: &str, file: &str, dependency: &str) -> Result<String> {
    let line = format!("{dependency}.workspace = true");
    let mut lines: Vec<String> = manifest.lines().map(str::to_owned).collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == "[dependencies]")
        .with_context(|| format!("{file} has no [dependencies] section"))?
        + 1;
    let end = lines[start..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |offset| start + offset);
    let names: Vec<(usize, &str)> = (start..end)
        .filter_map(|index| dependency_name(&lines[index]).map(|name| (index, name)))
        .collect();
    if names.iter().any(|(_, name)| *name == dependency) {
        bail!("{file} already depends on `{dependency}`");
    }
    let at = names
        .iter()
        .find(|(_, name)| *name > dependency)
        .map(|(index, _)| *index)
        .or_else(|| names.last().map(|(index, _)| index + 1))
        .unwrap_or(start);
    lines.insert(at, line);
    Ok(join(&lines, manifest))
}

/// The dependency a manifest line declares (`foo.workspace = true`,
/// `foo = "1"`, `foo = { … }`), if it declares one.
fn dependency_name(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let end = line.find(['.', '=', ' '])?;
    Some(&line[..end])
}

/// Mount `features::<feature>::handler` on a module router: a
/// `.routes(routes!(features::<feature>::handler))` call is chained after the
/// last existing `.routes(…)` call of `lib.rs`. Layout is left to `rustfmt`.
pub fn add_route(lib_rs: &str, file: &str, feature: &str) -> Result<String> {
    let call = format!(".routes(routes!(features::{feature}::handler))");
    if lib_rs.contains(&format!("features::{feature}::handler")) {
        bail!("{file} already mounts `features::{feature}::handler`");
    }
    let start = lib_rs.rfind(".routes(").with_context(|| {
        format!(
            "{file} has no `.routes(routes!(…))` call to chain `{call}` onto; add it to the \
             module's router by hand"
        )
    })?;
    let open = start + ".routes".len();
    let close = matching_paren(lib_rs, open)
        .with_context(|| format!("{file}: unbalanced parentheses after `.routes(`"))?;
    let mut out = String::with_capacity(lib_rs.len() + call.len());
    out.push_str(&lib_rs[..=close]);
    out.push_str(&call);
    out.push_str(&lib_rs[close + 1..]);
    Ok(out)
}

/// Byte index of the `)` closing the `(` at `open`.
fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (offset, c) in text[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Declare `pub mod <feature>;` in `features/mod.rs`, keeping the
/// declarations in alphabetical order.
pub fn add_feature_mod(mod_rs: &str, file: &str, feature: &str) -> Result<String> {
    let line = format!("pub mod {feature};");
    let mut lines: Vec<String> = mod_rs.lines().map(str::to_owned).collect();
    let mods: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, l)| {
            l.trim()
                .strip_prefix("pub mod ")
                .and_then(|rest| rest.strip_suffix(';'))
                .map(|name| (index, name.to_owned()))
        })
        .collect();
    if mods.iter().any(|(_, name)| name == feature) {
        bail!("{file} already declares `{line}`");
    }
    let at = mods
        .iter()
        .find(|(_, name)| name.as_str() > feature)
        .map(|(index, _)| *index)
        .or_else(|| mods.last().map(|(index, _)| index + 1))
        .unwrap_or(lines.len());
    lines.insert(at, line);
    Ok(join(&lines, mod_rs))
}

fn indentation(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// Join `lines`, ending with a newline when `original` did.
fn join(lines: &[String], original: &str) -> String {
    let mut text = lines.join("\n");
    if original.ends_with('\n') {
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_before_the_marker_with_its_indentation() {
        let text = format!("vec![\n    a(),\n    // {MARKER}\n]\n");
        let out = insert_before_marker(&text, "f", "b(),").unwrap();
        assert_eq!(
            out,
            format!("vec![\n    a(),\n    b(),\n    // {MARKER}\n]\n")
        );
        // Twice is refused rather than duplicated.
        assert!(
            insert_before_marker(&out, "f", "b(),")
                .unwrap_err()
                .to_string()
                .contains("already contains")
        );
    }

    #[test]
    fn needs_exactly_one_marker() {
        assert!(insert_before_marker("vec![]\n", "f", "b(),").is_err());
        let twice = format!("// {MARKER}\n// {MARKER}\n");
        assert!(insert_before_marker(&twice, "f", "b(),").is_err());
    }

    #[test]
    fn inserts_after_the_last_matching_line() {
        let text = "import a from \"~/modules/a\";\nimport b from \"~/modules/b\";\n\nrest\n";
        let out = insert_after_last(text, "f", "import ", "~/modules/", "import c;").unwrap();
        assert_eq!(
            out,
            "import a from \"~/modules/a\";\nimport b from \"~/modules/b\";\nimport c;\n\nrest\n"
        );
        assert!(insert_after_last(text, "f", "export ", "~/modules/", "x").is_err());
    }

    #[test]
    fn adds_a_dependency_in_alphabetical_order() {
        let manifest = "[package]\nname = \"x\"\n\n[dependencies]\naxum.workspace = true\n\
                        campbooks-kernel.workspace = true\ncampbooks-system.workspace = true\n\
                        chrono.workspace = true\n\n[dev-dependencies]\nzzz.workspace = true\n";
        let out = add_dependency(manifest, "f", "campbooks-mail").unwrap();
        assert!(out.contains(
            "campbooks-kernel.workspace = true\ncampbooks-mail.workspace = true\n\
             campbooks-system.workspace = true\n"
        ));
        let last = add_dependency(manifest, "f", "zeta").unwrap();
        assert!(last.contains("chrono.workspace = true\nzeta.workspace = true\n\n[dev"));
        assert!(add_dependency(manifest, "f", "axum").is_err());
        assert!(add_dependency("[package]\n", "f", "axum").is_err());
    }

    #[test]
    fn chains_a_route_after_the_last_one() {
        let inline = "router: OpenApiRouter::new()\n    .routes(routes!(features::a::handler))\n    \
                      .routes(routes!(features::b::handler)),\n";
        let out = add_route(inline, "f", "c").unwrap();
        assert_eq!(
            out,
            "router: OpenApiRouter::new()\n    .routes(routes!(features::a::handler))\n    \
             .routes(routes!(features::b::handler)).routes(routes!(features::c::handler)),\n"
        );
        assert!(add_route(&out, "f", "c").is_err());
        assert!(add_route("OpenApiRouter::new()", "f", "c").is_err());
    }

    #[test]
    fn declares_a_feature_in_alphabetical_order() {
        let mod_rs = "//! One folder per use case.\n\npub mod health;\npub mod version;\n";
        assert_eq!(
            add_feature_mod(mod_rs, "f", "status").unwrap(),
            "//! One folder per use case.\n\npub mod health;\npub mod status;\npub mod version;\n"
        );
        assert!(
            add_feature_mod(mod_rs, "f", "zone")
                .unwrap()
                .ends_with("pub mod zone;\n")
        );
        assert!(add_feature_mod(mod_rs, "f", "health").is_err());
        assert_eq!(
            add_feature_mod("//! Doc.\n", "f", "a").unwrap(),
            "//! Doc.\npub mod a;\n"
        );
    }
}
