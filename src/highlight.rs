use anyhow::{Context, Result};
use std::sync::OnceLock;
use syntect::{easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet};

pub fn lines(lines: Vec<String>, language: Option<&str>) -> Result<Vec<String>> {
    let Some(language) = language else {
        return Ok(lines);
    };
    static DEFINITIONS: OnceLock<(SyntaxSet, ThemeSet)> = OnceLock::new();
    let (syntaxes, themes) = DEFINITIONS.get_or_init(|| {
        (
            SyntaxSet::load_defaults_newlines(),
            ThemeSet::load_defaults(),
        )
    });
    let token = language.to_ascii_lowercase();
    let token = match token.as_str() {
        "csharp" | "c#" => "cs",
        "python" => "py",
        "bash" | "shell" => "sh",
        token => token,
    };
    let syntax = syntaxes
        .find_syntax_by_token(token)
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let theme = themes
        .themes
        .get("base16-ocean.dark")
        .context("Missing built-in walkthrough syntax theme")?;
    let mut highlighter = HighlightLines::new(syntax, theme);
    lines
        .into_iter()
        .map(|line| {
            let input = format!("{line}\n");
            let regions = highlighter
                .highlight_line(&input, syntaxes)
                .with_context(|| format!("Failed highlighting {language} source"))?;
            let mut styled = String::new();
            for (style, text) in regions {
                let text = text.strip_suffix('\n').unwrap_or(text);
                if Some(style.foreground) == theme.settings.foreground {
                    styled.push_str(text);
                } else if !text.is_empty() {
                    let color = style.foreground;
                    styled.push_str(&format!(
                        "\x1b[0;38;2;{};{};{}m{text}\x1b[0m",
                        color.r, color.g, color.b
                    ));
                }
            }
            Ok(styled)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::lines;
    use crate::matching::strip_ansi;

    #[test]
    fn languages_colorize_without_changing_source() {
        for (language, source) in [
            ("bash", "printf '%s\\n' \"$VALUE\" # comment"),
            ("python", "print('hello') # comment"),
            ("csharp", "Console.WriteLine(\"hello\"); // comment"),
            ("c#", "Console.WriteLine(\"hello\"); // comment"),
        ] {
            let styled = lines(vec![source.to_owned()], Some(language)).unwrap();
            assert_eq!(strip_ansi(&styled[0]), source);
            assert!(styled[0].contains("\x1b[0;38;2;"), "{language}: {styled:?}");
        }
    }

    #[test]
    fn multiline_syntax_retains_state_and_unknown_languages_remain_plain() {
        let source = vec!["value = \"\"\"first".to_owned(), "second\"\"\"".to_owned()];
        let styled = lines(source.clone(), Some("python")).unwrap();
        assert!(styled[1].contains("\x1b[0;38;2;"));
        assert_eq!(
            styled
                .iter()
                .map(|line| strip_ansi(line))
                .collect::<Vec<_>>(),
            source
        );
        assert_eq!(lines(source.clone(), None).unwrap(), source);
        assert_eq!(
            lines(source.clone(), Some("custom-unknown-language")).unwrap(),
            source
        );
    }
}
