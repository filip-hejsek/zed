use std::sync::Arc;

use editor::hover_markdown_style;
use gpui::{Context, Entity, ScrollHandle, SharedString, Subscription, Window, div, prelude::*};
use language::LanguageRegistry;
use markdown::{Markdown, MarkdownElement};
use project::project_settings::ProjectSettings;
use settings::{CommitMessageFormat, Settings, SettingsStore};

/// Displays a commit message according to the `git.commit_message_format`
/// setting.
///
/// In Markdown mode, the trailer block (`Signed-off-by:`, `Co-authored-by:`,
/// ...) is split off and shown as plain text, because Markdown would join its
/// lines into a single paragraph.
pub struct CommitMessage {
    source: SharedString,
    format: CommitMessageFormat,
    language_registry: Option<Arc<LanguageRegistry>>,
    body: Entity<Markdown>,
    trailers: Option<Entity<Markdown>>,
    scroll_handle: Option<ScrollHandle>,
    _settings_subscription: Subscription,
}

impl CommitMessage {
    pub fn new(
        source: SharedString,
        language_registry: Option<Arc<LanguageRegistry>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let format = ProjectSettings::get_global(cx).git.commit_message_format;
        let (body, trailers) = build_markdown(&source, format, language_registry.clone(), cx);
        let settings_subscription = cx.observe_global::<SettingsStore>(|this, cx| {
            let format = ProjectSettings::get_global(cx).git.commit_message_format;
            if format != this.format {
                this.format = format;
                (this.body, this.trailers) =
                    build_markdown(&this.source, format, this.language_registry.clone(), cx);
                cx.notify();
            }
        });
        Self {
            source,
            format,
            language_registry,
            body,
            trailers,
            scroll_handle: None,
            _settings_subscription: settings_subscription,
        }
    }

    /// The scroll container the message is rendered in, used to autoscroll
    /// while selecting text.
    pub fn with_scroll_handle(mut self, scroll_handle: ScrollHandle) -> Self {
        self.scroll_handle = Some(scroll_handle);
        self
    }

    pub fn source(&self) -> &SharedString {
        &self.source
    }

    #[cfg(test)]
    pub(crate) fn rendered_parts(&self, cx: &gpui::App) -> (SharedString, Option<SharedString>) {
        (
            self.body.read(cx).source().clone(),
            self.trailers
                .as_ref()
                .map(|trailers| trailers.read(cx).source().clone()),
        )
    }
}

fn build_markdown(
    source: &SharedString,
    format: CommitMessageFormat,
    language_registry: Option<Arc<LanguageRegistry>>,
    cx: &mut Context<CommitMessage>,
) -> (Entity<Markdown>, Option<Entity<Markdown>>) {
    match format {
        CommitMessageFormat::Plain => (cx.new(|cx| Markdown::new_text(source.clone(), cx)), None),
        CommitMessageFormat::Markdown => match split_trailers(source) {
            Some((body, trailers)) => {
                let body = SharedString::from(body.to_string());
                let trailers = SharedString::from(trailers.to_string());
                (
                    cx.new(|cx| Markdown::new(body, language_registry, None, cx)),
                    Some(cx.new(|cx| Markdown::new_text(trailers, cx))),
                )
            }
            None => (
                cx.new(|cx| Markdown::new(source.clone(), language_registry, None, cx)),
                None,
            ),
        },
    }
}

impl Render for CommitMessage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = hover_markdown_style(window, cx);
        let element = |markdown: &Entity<Markdown>| {
            let element = MarkdownElement::new(markdown.clone(), style.clone());
            match &self.scroll_handle {
                Some(scroll_handle) => element.scroll_handle(scroll_handle.clone()),
                None => element,
            }
        };
        // A block container rather than flex, because flex prevents the text
        // from wrapping in some of the containers this is rendered in.
        div()
            .w_full()
            .child(element(&self.body))
            .children(self.trailers.as_ref().map(element))
    }
}

/// Splits a commit message into its body and its trailer block, if it has one.
///
/// This follows Git's rules for finding trailers (see `git help
/// interpret-trailers`), with one simplification: every line of the last
/// paragraph must be a trailer or a continuation line. Git also accepts a
/// paragraph where only 25% of the lines are trailers, as long as one of them
/// is Git-generated or configured in `trailer.*`. Some projects rely on that,
/// e.g. QEMU puts notes like `[Filip: code style fixes]` between trailers,
/// but detecting it requires reading the user's Git configuration.
pub fn split_trailers(message: &str) -> Option<(&str, &str)> {
    let message = message.trim();

    // The trailers are the last paragraph, which must not be the first one
    // (the subject).
    let mut last_paragraph_start = None;
    let mut offset = 0;
    for line in message.split_inclusive('\n') {
        offset += line.len();
        if line.trim().is_empty() {
            last_paragraph_start = Some(offset);
        }
    }
    let last_paragraph_start = last_paragraph_start?;
    let body = &message[..last_paragraph_start];
    let trailers = &message[last_paragraph_start..];

    let mut has_trailer = false;
    for line in trailers.lines() {
        if line.starts_with([' ', '\t']) {
            // A continuation line, which is only allowed after a trailer.
            if !has_trailer {
                return None;
            }
        } else if is_trailer_line(line) {
            has_trailer = true;
        } else {
            return None;
        }
    }

    has_trailer.then(|| (body.trim_end(), trailers))
}

/// Whether the line has the form `Token: value`, where the token consists of
/// ASCII alphanumerics and hyphens, optionally followed by whitespace.
fn is_trailer_line(line: &str) -> bool {
    let Some((token, _value)) = line.split_once(':') else {
        return false;
    };
    let token = token.trim_end_matches([' ', '\t']);
    !token.is_empty()
        && token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use settings::SettingsStore;

    #[test]
    fn test_split_trailers() {
        assert_eq!(
            split_trailers(
                "Fix crash\n\nThe body wraps\nacross lines.\n\nSigned-off-by: A <a@example.com>\nCo-authored-by: B <b@example.com>\n"
            ),
            Some((
                "Fix crash\n\nThe body wraps\nacross lines.",
                "Signed-off-by: A <a@example.com>\nCo-authored-by: B <b@example.com>"
            ))
        );

        // Only a subject and trailers.
        assert_eq!(
            split_trailers("Fix crash\n\nFixes: #123"),
            Some(("Fix crash", "Fixes: #123"))
        );

        // Continuation lines, whitespace before the separator and empty values.
        assert_eq!(
            split_trailers("Subject\n\nBody\n\nKey : value\n  continued\nAcked-by:"),
            Some(("Subject\n\nBody", "Key : value\n  continued\nAcked-by:"))
        );

        // Blank lines that contain whitespace and CRLF line endings.
        assert_eq!(
            split_trailers("Subject\r\n \r\nKey: value\r\n"),
            Some(("Subject", "Key: value"))
        );
    }

    #[test]
    fn test_split_trailers_without_trailers() {
        // The subject is never a trailer.
        assert_eq!(split_trailers("Release: 1.0"), None);
        assert_eq!(split_trailers("Release: 1.0\nKey: value"), None);
        assert_eq!(split_trailers("\n\nRelease: 1.0\n\n"), None);

        // The last paragraph contains a line that isn't a trailer.
        assert_eq!(
            split_trailers("Subject\n\nSigned-off-by: A\nNot a trailer"),
            None
        );
        assert_eq!(
            split_trailers("Subject\n\nSigned-off-by: A\n[Filip: code style fixes]"),
            None
        );

        // Tokens can't contain spaces or punctuation other than hyphens.
        assert_eq!(split_trailers("Subject\n\nSee the docs: here"), None);
        assert_eq!(split_trailers("Subject\n\nfoo_bar: baz"), None);
        assert_eq!(split_trailers("Subject\n\n: value"), None);

        // A continuation line can't start the block.
        assert_eq!(split_trailers("Subject\n\n  Key: value"), None);

        // Trailers that aren't in the last paragraph.
        assert_eq!(
            split_trailers("Subject\n\nSigned-off-by: A\n\nMore text."),
            None
        );
    }

    fn init_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
            theme_settings::init(theme::LoadThemes::JustBase, cx);
        });
    }

    #[gpui::test]
    fn test_commit_message_format_setting(cx: &mut TestAppContext) {
        init_test(cx);

        let source = SharedString::from("Subject\n\nBody\ntext\n\nSigned-off-by: A\n");
        let commit_message = cx.new(|cx| CommitMessage::new(source.clone(), None, cx));
        commit_message.read_with(cx, |commit_message, cx| {
            assert_eq!(
                commit_message.rendered_parts(cx),
                (
                    SharedString::from("Subject\n\nBody\ntext"),
                    Some(SharedString::from("Signed-off-by: A"))
                )
            );
        });

        cx.update(|cx| {
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |settings| {
                    settings.git.get_or_insert_default().commit_message_format =
                        Some(CommitMessageFormat::Plain);
                });
            });
        });
        commit_message.read_with(cx, |commit_message, cx| {
            assert_eq!(commit_message.rendered_parts(cx), (source.clone(), None));
        });
    }
}
