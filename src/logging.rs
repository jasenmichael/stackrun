use crate::config::types::{PrefixColors, ProcessOptions};
use owo_colors::OwoColorize;

/// Host log name. Not truncated by `prefixLength`.
pub const HOST_NAME: &str = "stackrun";

/// Existing prefix palette, cycled across letters of [`HOST_NAME`].
const RAINBOW: [&str; 6] = ["red", "green", "yellow", "blue", "magenta", "cyan"];

/// No-op. Host logs go through [`emit`] / [`emit_opt`], not a tracing subscriber.
pub fn init() {}

/// `colors: false` turns off host rainbow. Missing / auto / true keep it.
pub fn host_color_enabled(process: Option<&ProcessOptions>) -> bool {
    !matches!(
        process.and_then(|p| p.colors.as_ref()),
        Some(PrefixColors::Flag(false))
    )
}

/// Print a host line to stderr with rainbow `[stackrun]`.
pub fn emit(message: impl AsRef<str>) {
    emit_opt(message, true);
}

/// Print a host line to stderr. `color` rainbows the name token only.
pub fn emit_opt(message: impl AsRef<str>, color: bool) {
    eprintln!("{}", format_host_line(message.as_ref(), color));
}

/// `[stackrun] {message}`. Rainbow paints letters of `stackrun` only.
pub fn format_host_line(message: &str, color: bool) -> String {
    format!("{} {message}", host_prefix(color))
}

fn host_prefix(color: bool) -> String {
    if !color {
        return format!("[{HOST_NAME}]");
    }
    let mut out = String::from("[");
    for (i, ch) in HOST_NAME.chars().enumerate() {
        out.push_str(&colorize(&ch.to_string(), Some(RAINBOW[i % RAINBOW.len()])));
    }
    out.push(']');
    out
}

pub(crate) fn colorize(text: &str, color: Option<&str>) -> String {
    match color.map(|c| c.to_ascii_lowercase()) {
        Some(c) if c == "red" => text.red().to_string(),
        Some(c) if c == "green" => text.green().to_string(),
        Some(c) if c == "yellow" => text.yellow().to_string(),
        Some(c) if c == "blue" => text.blue().to_string(),
        Some(c) if c == "magenta" => text.magenta().to_string(),
        Some(c) if c == "cyan" => text.cyan().to_string(),
        Some(c) if c == "white" => text.white().to_string(),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' && chars.peek() == Some(&'[') {
                chars.next();
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn host_line_plain_matches_child_shape() {
        assert_eq!(
            format_host_line("Tunneling is enabled", false),
            "[stackrun] Tunneling is enabled"
        );
        assert_eq!(
            format_host_line("Starting [nuxt] npm run dev", false),
            "[stackrun] Starting [nuxt] npm run dev"
        );
        assert_eq!(
            format_host_line("Stackrun completed", false),
            "[stackrun] Stackrun completed"
        );
    }

    #[test]
    fn host_rainbow_paints_name_letters_only() {
        let out = format_host_line("Tunneling is enabled", true);
        assert_eq!(strip_ansi(&out), "[stackrun] Tunneling is enabled");
        assert!(out.contains('\u{1b}'), "ANSI on name: {out:?}");
        assert!(
            out.ends_with(" Tunneling is enabled"),
            "body uncolored: {out:?}"
        );
        assert!(
            !out[out.len() - " Tunneling is enabled".len()..].contains('\u{1b}'),
            "body has no ANSI: {out:?}"
        );
        assert!(out.starts_with('['), "uncolored opening bracket: {out:?}");
        assert!(out.contains(']'), "closing bracket present: {out:?}");
    }

    #[test]
    fn host_color_follows_process_colors_false() {
        let off = ProcessOptions {
            colors: Some(PrefixColors::Flag(false)),
            ..ProcessOptions::default()
        };
        assert!(!host_color_enabled(Some(&off)));
        assert!(host_color_enabled(None));
        assert!(host_color_enabled(Some(&ProcessOptions::default())));
    }
}
