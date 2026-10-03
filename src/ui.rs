use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::preview::render_ansi;
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < 20 || area.height < 8 {
        return;
    }
    let theme = app.theme.clone();
    frame.render_widget(Block::default().style(theme.base()), area);

    if matches!(app.screen, crate::app::Screen::Wizard) {
        draw_wizard_screen(frame, app, &theme, area);
    } else {
        let [status, steps, gap, question, note, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        draw_status(frame, app, &theme, status);
        draw_editor_label(frame, app, &theme, steps);
        clear_line(frame, &theme, gap);
        draw_question(frame, app, &theme, question);
        draw_note(frame, app, &theme, note);
        draw_editor(frame, app, &theme, body);
        draw_keys(frame, app, &theme, footer);
    }

    if app.confirm_quit {
        draw_overlay(
            frame,
            &theme,
            &[
                "Save changes before quitting?",
                "",
                "s     save and quit",
                "d     discard",
                "esc   cancel",
            ],
            "Quit",
        );
    }
}

const WORDMARK: &[&str] = &[
    "███████╗ ████████╗  █████╗  ██████╗  ███████╗ ██╗  ██╗ ██╗ ██████╗ ",
    "██╔════╝ ╚══██╔══╝ ██╔══██╗ ██╔══██╗ ██╔════╝ ██║  ██║ ██║ ██╔══██╗",
    "███████╗    ██║    ███████║ ██████╔╝ ███████╗ ███████║ ██║ ██████╔╝",
    "╚════██║    ██║    ██╔══██║ ██╔══██╗ ╚════██║ ██╔══██║ ██║ ██╔═══╝ ",
    "███████║    ██║    ██║  ██║ ██║  ██║ ███████║ ██║  ██║ ██║ ██║     ",
    "╚══════╝    ╚═╝    ╚═╝  ╚═╝ ╚═╝  ╚═╝ ╚══════╝ ╚═╝  ╚═╝ ╚═╝ ╚═╝     ",
];

fn draw_wordmark(frame: &mut Frame, theme: &Theme, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let lines: Vec<String> = if area.height >= 6 && area.width >= 76 {
        let width = WORDMARK.iter().map(|line| line.chars().count()).max().unwrap_or(0);
        WORDMARK.iter().map(|line| pad(line, width)).collect()
    } else {
        vec!["S T A R S H I P".into()]
    };
    let block_w = lines.iter().map(|line| line.chars().count()).max().unwrap_or(0);
    let left = (area.width as usize).saturating_sub(block_w) / 2;
    let rows = lines.len().min(area.height as usize);
    for (index, line) in lines.iter().take(rows).enumerate() {
        frame.render_widget(
            Paragraph::new(paint_logo(line, left, area.width as usize, theme)),
            Rect::new(area.x, area.y + index as u16, area.width, 1),
        );
    }
}

fn paint_logo(line: &str, left: usize, width: usize, theme: &Theme) -> Line<'static> {
    let base = theme.base();
    let count = line.chars().count().max(1);
    let mut spans = vec![Span::styled(" ".repeat(left.min(width)), base)];
    for (index, ch) in line.chars().enumerate() {
        let style = if ch == ' ' {
            base
        } else {
            let t = index as f32 / (count - 1).max(1) as f32;
            base.fg(theme.gradient_at(t))
        };
        spans.push(Span::styled(ch.to_string(), style));
    }
    let used = left.min(width) + count;
    if used < width {
        spans.push(Span::styled(" ".repeat(width - used), base));
    }
    Line::from(spans)
}

fn draw_status(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let bar = theme.footer();
    let name = Span::styled(
        " starship-editor ",
        Style::default()
            .fg(theme.accent)
            .bg(theme.bg_deep)
            .add_modifier(Modifier::BOLD),
    );
    let right = if app.notice.is_empty() {
        format!("{} ", theme.name)
    } else {
        format!("{} ", app.notice)
    };
    let used = 16 + right.chars().count();
    let pad = (area.width as usize).saturating_sub(used);
    let right_style = if app.notice.is_empty() {
        Style::default().fg(theme.border).bg(theme.bg_deep)
    } else {
        Style::default().fg(theme.fg).bg(theme.bg_deep)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            name,
            Span::styled(" ".repeat(pad), bar),
            Span::styled(right, right_style),
        ])),
        area,
    );
}

fn draw_steps(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let (current, names) = app.wizard_progress();
    let mut spans = vec![Span::styled("  ", theme.base())];
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("    ", theme.base()));
        }
        let style = if index == current {
            theme
                .base()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else if index < current {
            theme.base()
        } else {
            dim(theme)
        };
        spans.push(Span::styled((*name).to_string(), style));
    }
    let used = line_width(&spans);
    pad_line(&mut spans, area.width as usize, used, theme.base());
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_editor_label(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let path = app.path().display().to_string();
    let left = format!(" {}", app.editor_title());
    let room = (area.width as usize).saturating_sub(left.chars().count() + 2);
    let shown = clip(&path, room);
    let pad = (area.width as usize).saturating_sub(left.chars().count() + 1 + shown.chars().count());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, theme.base().add_modifier(Modifier::BOLD)),
            Span::styled(" ".repeat(pad), theme.base()),
            Span::styled(shown, dim(theme)),
            Span::styled(" ", theme.base()),
        ])),
        area,
    );
}

fn draw_question(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let text = match app.screen {
        crate::app::Screen::Wizard => app.wizard_question(),
        crate::app::Screen::Editor => String::new(),
    };
    let style = theme.base().add_modifier(Modifier::BOLD);
    let shown = clip(&format!("  {text}"), area.width as usize);
    frame.render_widget(
        Paragraph::new(fit_span(&shown, area.width as usize, style)).style(style),
        area,
    );
}

fn draw_note(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let (text, style) = match app.screen {
        crate::app::Screen::Wizard => {
            let note = app.wizard_note();
            let typing = app.wizard_typing_color();
            let placeholder = typing && note == "#cba6f7";
            let style = if typing && !placeholder {
                theme.base().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else {
                dim(theme)
            };
            (format!("  {note}"), style)
        }
        crate::app::Screen::Editor => (String::new(), dim(theme)),
    };
    let shown = clip(&text, area.width as usize);
    frame.render_widget(
        Paragraph::new(fit_span(&shown, area.width as usize, style)).style(style),
        area,
    );
}

fn draw_wizard_screen(frame: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let [status, middle, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_status(frame, app, theme, status);
    draw_keys(frame, app, theme, footer);

    let rows = app.wizard_rows();
    let confirm = rows.is_empty();
    let note = !app.wizard_note().trim().is_empty();
    let plan = plan_wizard(
        middle.height,
        middle.width,
        if confirm { 4 } else { rows.len() as u16 },
        note,
        !confirm,
    );
    let bands = place(middle, &plan);
    let column = content_column(middle);

    draw_wordmark(frame, theme, bands.title);
    draw_steps(frame, app, theme, slice_x(bands.steps, column));
    draw_question(frame, app, theme, slice_x(bands.question, column));
    if bands.note.height > 0 {
        draw_note(frame, app, theme, slice_x(bands.note, column));
    }
    if confirm {
        draw_summary(frame, app, theme, slice_x(bands.list, column));
    } else {
        draw_choice_list(frame, app, theme, slice_x(bands.list, column), &rows);
        draw_detail(frame, theme, slice_x(bands.detail, column), &rows);
    }
    draw_previews(frame, app, theme, slice_x(bands.preview, column));
}

fn draw_summary(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let facts = app.wizard_summary();
    for (index, (label, value)) in facts.iter().enumerate() {
        if index as u16 >= area.height {
            break;
        }
        let label = format!("  {label:<8}");
        let value = clip(value, (area.width as usize).saturating_sub(label.chars().count()));
        let rest = (area.width as usize).saturating_sub(label.chars().count() + value.chars().count());
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(label, dim(theme)),
                Span::styled(value, theme.base()),
                Span::styled(" ".repeat(rest), theme.base()),
            ])),
            Rect::new(area.x, area.y + index as u16, area.width, 1),
        );
    }
}

#[derive(Clone, Copy)]
enum Part {
    Title,
    Steps,
    Question,
    Note,
    List,
    Detail,
    Preview,
}

/// Heights for one screen. Gaps are the top margin, the space between parts, and the bottom margin.
struct Plan {
    parts: Vec<(Part, u16)>,
    gaps: Vec<u16>,
}

struct Bands {
    title: Rect,
    steps: Rect,
    question: Rect,
    note: Rect,
    list: Rect,
    detail: Rect,
    preview: Rect,
}

fn plan_wizard(height: u16, width: u16, list_len: u16, note: bool, detail: bool) -> Plan {
    let mut title = if height >= 28 && width >= 76 {
        6
    } else if height >= 16 {
        1
    } else {
        0
    };
    let mut list = list_len;
    let mut preview = 7u16;
    let note_h = u16::from(note);
    let detail_h = u16::from(detail);
    loop {
        let content = title + 2 + note_h + list + detail_h + preview;
        let slots = gap_count(title, note_h, list, detail_h);
        if content.saturating_add(slots) <= height || (list <= 3 && preview <= 6 && title == 0) {
            break;
        }
        if list > 3 {
            list -= 1;
        } else if preview > 6 {
            preview -= 1;
        } else if title > 1 {
            title = 1;
        } else if title == 1 {
            title = 0;
        } else {
            break;
        }
    }
    let mut parts = Vec::new();
    if title > 0 {
        parts.push((Part::Title, title));
    }
    parts.push((Part::Steps, 1));
    parts.push((Part::Question, 1));
    if note_h > 0 {
        parts.push((Part::Note, note_h));
    }
    if list > 0 {
        parts.push((Part::List, list));
    }
    if detail_h > 0 {
        parts.push((Part::Detail, detail_h));
    }
    parts.push((Part::Preview, preview));
    let slots = parts.len() + 1;
    let content: u16 = parts.iter().map(|(_, height)| height).sum();
    let gaps = spread(height.saturating_sub(content), slots);
    Plan { parts, gaps }
}

fn gap_count(title: u16, note: u16, list: u16, detail: u16) -> u16 {
    let visible = 3
        + usize::from(title > 0)
        + usize::from(note > 0)
        + usize::from(list > 0)
        + usize::from(detail > 0);
    visible as u16 + 1
}

fn spread(slack: u16, slots: usize) -> Vec<u16> {
    if slots == 0 {
        return Vec::new();
    }
    let base = slack / slots as u16;
    let extra = (slack % slots as u16) as usize;
    let mut gaps = vec![base; slots];
    for step in 0..extra {
        let index = if slots == 1 { 0 } else { 1 + step % (slots - 1) };
        gaps[index] += 1;
    }
    gaps
}

fn place(area: Rect, plan: &Plan) -> Bands {
    let mut y = area.y;
    let mut gaps = plan.gaps.iter().copied();
    if let Some(gap) = gaps.next() {
        y = y.saturating_add(gap);
    }
    let mut bands = Bands {
        title: Rect::default(),
        steps: Rect::default(),
        question: Rect::default(),
        note: Rect::default(),
        list: Rect::default(),
        detail: Rect::default(),
        preview: Rect::default(),
    };
    for (part, height) in &plan.parts {
        let rect = Rect::new(area.x, y, area.width, *height);
        match part {
            Part::Title => bands.title = rect,
            Part::Steps => bands.steps = rect,
            Part::Question => bands.question = rect,
            Part::Note => bands.note = rect,
            Part::List => bands.list = rect,
            Part::Detail => bands.detail = rect,
            Part::Preview => bands.preview = rect,
        }
        y = y.saturating_add(*height);
        if let Some(gap) = gaps.next() {
            y = y.saturating_add(gap);
        }
    }
    bands
}

fn content_column(area: Rect) -> Rect {
    let width = if area.width > 108 {
        100
    } else {
        area.width.saturating_sub(4).max(20).min(area.width)
    };
    Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y,
        width,
        area.height,
    )
}

fn slice_x(band: Rect, column: Rect) -> Rect {
    Rect::new(column.x, band.y, column.width.min(band.width), band.height)
}

fn draw_choice_list(
    frame: &mut Frame,
    app: &App,
    theme: &Theme,
    area: Rect,
    rows: &[(String, String, String, bool)],
) {
    if area.height == 0 {
        return;
    }
    let selected = rows.iter().position(|(_, _, _, on)| *on).unwrap_or(0);
    let start = window_start(selected, rows.len(), area.height as usize);
    let name_w = 24.min((area.width as usize).saturating_sub(10) / 2).max(12);
    for (offset, (name, meta, _, on)) in rows.iter().skip(start).take(area.height as usize).enumerate()
    {
        let index = start + offset;
        let row = if *on { theme.selected() } else { theme.base() };
        let mut spans = Vec::new();
        spans.push(Span::styled("  ", row));
        if let Some(((red, green, blue), _)) = app.wizard_color(index) {
            let chip = Color::Rgb(red, green, blue);
            spans.push(Span::styled("██", Style::default().fg(chip).bg(chip)));
            spans.push(Span::styled(" ", row));
        }
        spans.push(Span::styled(pad(name, name_w), row));
        let meta_style = if *on {
            row
        } else if meta == "On" {
            theme.base().fg(theme.accent)
        } else {
            dim(theme)
        };
        let used = line_width(&spans);
        let room = (area.width as usize).saturating_sub(used + 2);
        spans.push(Span::styled(clip(meta, room), meta_style));
        let used = line_width(&spans);
        pad_line(&mut spans, area.width as usize, used, row);
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(area.x, area.y + offset as u16, area.width, 1),
        );
    }
}

fn draw_detail(frame: &mut Frame, theme: &Theme, area: Rect, rows: &[(String, String, String, bool)]) {
    clear_line(frame, theme, area);
    let Some((_, _, detail, _)) = rows.iter().find(|(_, _, _, on)| *on) else {
        return;
    };
    if area.height == 0 {
        return;
    }
    let text = clip(&format!("  {detail}"), area.width as usize);
    frame.render_widget(
        Paragraph::new(fit_span(&text, area.width as usize, dim(theme))).style(dim(theme)),
        Rect::new(area.x, area.y, area.width, 1),
    );
}

fn draw_editor(frame: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let preview_h = 6.min(area.height.saturating_sub(5));
    let [lists, preview] =
        Layout::vertical([Constraint::Min(4), Constraint::Length(preview_h)]).areas(area);
    let [modules, fields] =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).areas(lists);
    let rows = app.editor_rows();
    let selected = rows.iter().position(|(_, on)| *on).unwrap_or(0);
    draw_lines(frame, theme, modules, "Modules", &rows, selected, false);
    let field_rows: Vec<(String, bool)> = app
        .editor_fields()
        .into_iter()
        .map(|(name, value, on)| (format!("{name:<14}  {value}"), on))
        .collect();
    let field_at = field_rows.iter().position(|(_, on)| *on).unwrap_or(0);
    draw_lines(frame, theme, fields, "Fields", &field_rows, field_at, false);
    draw_previews(frame, app, theme, preview);
}

fn draw_previews(frame: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let panes = app.panes();
    if panes.is_empty() || area.height < 3 {
        return;
    }
    let inset = Rect::new(
        area.x + 1,
        area.y,
        area.width.saturating_sub(2),
        area.height,
    );
    if inset.width < 8 {
        return;
    }
    let height = inset.height / panes.len() as u16;
    for (index, pane) in panes.iter().enumerate() {
        let rect = Rect::new(
            inset.x,
            inset.y + height * index as u16,
            inset.width,
            if index + 1 == panes.len() {
                inset.height - height * index as u16
            } else {
                height
            },
        );
        let columns = rect.width.saturating_sub(2);
        let body = match app.preview.get(&pane.config, columns) {
            Some(Ok(ansi)) => render_ansi(ansi),
            Some(Err(err)) => Text::styled(err.clone(), dim(theme)),
            None => Text::styled("Rendering…", dim(theme)),
        };
        frame.render_widget(
            Paragraph::new(body)
                .block(panel(&pane.title, theme, pane.selected))
                .wrap(Wrap { trim: false }),
            rect,
        );
    }
}

fn draw_lines(
    frame: &mut Frame,
    theme: &Theme,
    area: Rect,
    title: &str,
    rows: &[(String, bool)],
    selected: usize,
    hot: bool,
) {
    let block = panel(title, theme, hot);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let start = window_start(selected, rows.len(), inner.height as usize);
    for (offset, (label, on)) in rows.iter().skip(start).take(inner.height as usize).enumerate() {
        let style = if *on { theme.selected() } else { theme.base() };
        let text = fit(&format!(" {label}"), inner.width as usize);
        frame.render_widget(
            Paragraph::new(text).style(style),
            Rect::new(inner.x, inner.y + offset as u16, inner.width, 1),
        );
    }
}

fn draw_keys(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let keys = match app.screen {
        crate::app::Screen::Wizard => app.wizard_keys(),
        crate::app::Screen::Editor => app.editor_keys(),
    };
    let bar = theme.footer();
    let mut spans = vec![Span::styled("  ", bar)];
    for (index, (key, action)) in keys.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("    ", bar));
        }
        if !key.is_empty() {
            spans.push(Span::styled(
                (*key).to_string(),
                Style::default()
                    .fg(theme.fg)
                    .bg(theme.bg_deep)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(" ", bar));
        }
        spans.push(Span::styled(
            (*action).to_string(),
            Style::default().fg(theme.border).bg(theme.bg_deep),
        ));
    }
    let used = line_width(&spans);
    pad_line(&mut spans, area.width as usize, used, bar);
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_overlay(frame: &mut Frame, theme: &Theme, lines: &[&str], title: &str) {
    let width = 42.min(frame.area().width.saturating_sub(4));
    let height = (lines.len() as u16 + 2).min(frame.area().height.saturating_sub(4));
    let area = Rect::new(
        frame.area().x + (frame.area().width.saturating_sub(width)) / 2,
        frame.area().y + (frame.area().height.saturating_sub(height)) / 2,
        width,
        height,
    );
    let body = lines.join("\n");
    frame.render_widget(
        Paragraph::new(format!("\n{body}"))
            .block(panel(title, theme, false))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn panel<'a>(title: &'a str, theme: &'a Theme, hot: bool) -> Block<'a> {
    let edge = if hot { theme.accent } else { theme.border };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {title} "))
        .title_style(Style::default().fg(edge))
        .border_style(Style::default().fg(edge))
        .style(theme.base())
}

fn dim(theme: &Theme) -> Style {
    Style::default().fg(theme.border).bg(theme.bg)
}

fn clear_line(frame: &mut Frame, theme: &Theme, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(" ".repeat(area.width as usize)).style(theme.base()),
        Rect::new(area.x, area.y, area.width, 1),
    );
}

fn line_width(spans: &[Span]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

fn pad_line(spans: &mut Vec<Span>, width: usize, used: usize, style: Style) {
    if used < width {
        spans.push(Span::styled(" ".repeat(width - used), style));
    }
}

fn fit_span(text: &str, width: usize, style: Style) -> Line<'static> {
    Line::from(Span::styled(fit(text, width), style))
}

fn window_start(selected: usize, len: usize, rows: usize) -> usize {
    if rows == 0 || len <= rows {
        return 0;
    }
    let max_start = len - rows;
    selected.saturating_sub(rows / 2).min(max_start)
}

fn pad(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count >= width {
        return clip(text, width);
    }
    let mut owned = text.to_string();
    owned.extend(std::iter::repeat(' ').take(width - count));
    owned
}

fn clip(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut owned: String = text.chars().take(width.saturating_sub(1)).collect();
    owned.push('…');
    owned
}

fn fit(text: &str, width: usize) -> String {
    let clipped = clip(text, width);
    let count = clipped.chars().count();
    if count >= width {
        return clipped;
    }
    let mut owned = clipped;
    owned.extend(std::iter::repeat(' ').take(width - count));
    owned
}
