use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{
    chrome::sanitize,
    config::{SemanticStyle, StylesConfig},
    dialog::{
        dialog_area, fill_row, frame_inner, render_footer, render_frame, render_list_scrollbar,
        render_title,
    },
    toast::{Message, MessageKind},
};

const MAX_WIDTH: u16 = 80;
const MAX_HEIGHT: u16 = 30;

pub(super) struct MessagesDialog {
    scroll: usize,
    follow_end: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MessagesAction {
    Stay,
    Close,
    Clear,
}

impl MessagesDialog {
    pub(super) fn open() -> Self {
        Self {
            scroll: 0,
            follow_end: true,
        }
    }

    pub(super) fn reset(&mut self) {
        self.scroll = 0;
        self.follow_end = true;
    }

    pub(super) fn key<'a>(
        &mut self,
        key: KeyEvent,
        host: Rect,
        messages: impl Iterator<Item = &'a Message>,
    ) -> MessagesAction {
        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return MessagesAction::Stay;
        }
        let (width, visible) = body_size(host);
        let total = message_lines(messages, width).len();
        let max_scroll = total.saturating_sub(visible);
        if self.follow_end {
            self.scroll = max_scroll;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return MessagesAction::Close,
            KeyCode::Char('c') => return MessagesAction::Clear,
            KeyCode::Up | KeyCode::Char('k') => {
                self.follow_end = false;
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll = (self.scroll + 1).min(max_scroll);
                self.follow_end = self.scroll == max_scroll;
            }
            KeyCode::PageUp => {
                self.follow_end = false;
                self.scroll = self.scroll.saturating_sub(visible.max(1));
            }
            KeyCode::PageDown => {
                self.scroll = (self.scroll + visible.max(1)).min(max_scroll);
                self.follow_end = self.scroll == max_scroll;
            }
            KeyCode::Home => {
                self.follow_end = false;
                self.scroll = 0;
            }
            KeyCode::End => {
                self.follow_end = true;
                self.scroll = max_scroll;
            }
            _ => {}
        }
        MessagesAction::Stay
    }

    pub(super) fn render<'a>(
        &mut self,
        host: Rect,
        styles: &StylesConfig,
        messages: impl Iterator<Item = &'a Message>,
        buffer: &mut Buffer,
    ) {
        let area = render_frame(dialog_area(host, MAX_WIDTH, MAX_HEIGHT), buffer);
        if area.width == 0 || area.height == 0 {
            return;
        }
        let (header, footer) = chrome_rows(area.height);
        let body_height = usize::from(area.height.saturating_sub(header + footer));
        if header == 1 {
            render_title(area, " messages · this session", buffer);
        }
        let lines = message_lines(messages, usize::from(area.width));
        let max_scroll = lines.len().saturating_sub(body_height);
        if self.follow_end {
            self.scroll = max_scroll;
        } else {
            self.scroll = self.scroll.min(max_scroll);
        }
        if lines.is_empty() && body_height > 0 {
            buffer.set_string(
                area.x,
                area.y + header,
                " No messages yet",
                Style::default(),
            );
        } else {
            for (offset, line) in lines.iter().skip(self.scroll).take(body_height).enumerate() {
                let y = area.y + header + u16::try_from(offset).unwrap_or(u16::MAX);
                let row = Rect::new(area.x, y, area.width, 1);
                let style = styles.apply(
                    match line.kind {
                        MessageKind::Info => SemanticStyle::Normal,
                        MessageKind::Error => SemanticStyle::Error,
                    },
                    Style::default(),
                );
                fill_row(row, style, buffer);
                buffer.set_stringn(area.x, y, &line.text, usize::from(area.width), style);
            }
            render_list_scrollbar(
                self.scroll,
                lines.len(),
                Rect::new(area.x, area.y + header, area.width, body_height as u16),
                buffer,
            );
        }
        if footer == 1 {
            render_footer(area, " ↑↓/jk scroll  home/end  c clear  esc close", buffer);
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct MessageLine {
    kind: MessageKind,
    text: String,
}

fn message_lines<'a>(
    messages: impl IntoIterator<Item = &'a Message>,
    width: usize,
) -> Vec<MessageLine> {
    messages
        .into_iter()
        .flat_map(|message| {
            let marker = match message.kind {
                MessageKind::Info => "•",
                MessageKind::Error => "!",
            };
            wrap(&sanitize(&message.text), width.saturating_sub(3))
                .into_iter()
                .enumerate()
                .map(move |(index, text)| MessageLine {
                    kind: message.kind,
                    text: format!(" {} {text}", if index == 0 { marker } else { " " }),
                })
        })
        .collect()
}

fn wrap(message: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut used = 0usize;
    for grapheme in message.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if used > 0 && used.saturating_add(grapheme_width) > width {
            lines.push(std::mem::take(&mut line));
            used = 0;
        }
        line.push_str(grapheme);
        used = used.saturating_add(grapheme_width);
    }
    lines.push(line);
    lines
}

fn chrome_rows(height: u16) -> (u16, u16) {
    (u16::from(height >= 2), u16::from(height >= 3))
}

fn body_size(host: Rect) -> (usize, usize) {
    let area = frame_inner(dialog_area(host, MAX_WIDTH, MAX_HEIGHT));
    let (header, footer) = chrome_rows(area.height);
    (
        usize::from(area.width),
        usize::from(area.height.saturating_sub(header + footer)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn message(kind: MessageKind, text: &str) -> Message {
        Message {
            kind,
            text: text.into(),
        }
    }

    #[test]
    fn long_messages_wrap_instead_of_truncating() {
        let lines = message_lines(
            &[message(
                MessageKind::Error,
                "this notification is much too long",
            )],
            12,
        );
        assert_eq!(
            lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            [" ! this noti", "   fication ", "   is much t", "   oo long"]
        );
    }

    #[test]
    fn navigation_scrolls_and_clear_is_typed() {
        let messages = (0..20)
            .map(|index| message(MessageKind::Info, &format!("message {index}")))
            .collect::<Vec<_>>();
        let mut dialog = MessagesDialog::open();
        let host = Rect::new(0, 0, 30, 8);
        dialog.render(
            host,
            &StylesConfig::default(),
            messages.iter(),
            &mut Buffer::empty(host),
        );
        let end = dialog.scroll;
        assert!(end > 0);
        assert_eq!(
            dialog.key(
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                host,
                messages.iter(),
            ),
            MessagesAction::Stay
        );
        assert_eq!(dialog.scroll, end - 1);
        assert_eq!(
            dialog.key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
                host,
                messages.iter(),
            ),
            MessagesAction::Clear
        );
    }
}
