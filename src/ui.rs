use crate::{
    app::{App, Hit, Intent, Overlay, PANES, SCREENS},
    aurora::{shortcut_line, Chrome},
    editor::{self, Action},
    preview,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use similar::{ChangeTag, TextDiff};
use unicode_width::UnicodeWidthStr;
fn chrome(a: &App) -> Chrome {
    Chrome {
        base: a.theme.base(),
        muted: Style::default().fg(a.theme.muted).bg(a.theme.bg),
        accent: Style::default().fg(a.theme.accent).bg(a.theme.bg),
        data: Style::default()
            .fg(*a.theme.gradient.last().unwrap_or(&a.theme.accent))
            .bg(a.theme.bg),
        border: Style::default().fg(a.theme.border).bg(a.theme.bg),
        deep: a.theme.footer(),
    }
}
fn panel(title: String, c: Chrome, focus: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focus { c.accent } else { c.border })
        .title(format!(" {title} "))
        .title_style(if focus { c.accent } else { c.muted })
        .style(c.base)
}
fn hit(a: &mut App, area: Rect, intent: Intent) {
    if !area.is_empty() {
        a.hits.push(Hit { area, intent });
    }
}
pub fn draw(f: &mut Frame, a: &mut App) {
    a.hits.clear();
    let c = chrome(a);
    f.render_widget(Block::default().style(c.base), f.area());
    let root = c.shell(f, f.area());
    a.editor_body = None;
    if [0, 2].contains(&a.screen) && root.width >= 40 && root.height >= 15 {
        draw_studio(f, a, c, root);
        return;
    }
    if root.height < 9 || root.width < 22 {
        f.render_widget(
            Paragraph::new("Aster\nResize to at least 22×11\n(q) quit").style(c.accent),
            root,
        );
        if a.overlay.is_some() {
            draw_overlay(f, a, c);
        }
        return;
    }
    let bands = Layout::vertical([
        Constraint::Length(if f.area().height <= 24 {
            2
        } else {
            crate::aurora::title_height(root.height)
        }),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(root);
    let state = format!(
        "{} · {}",
        if a.workspace.dirty() {
            "● modified"
        } else {
            "✓ saved"
        },
        a.theme.name
    );
    c.title(
        f,
        bands[0],
        "ASTER",
        "Starship prompt studio",
        &state,
        c.data,
    );
    let mut x = bands[1].x + 1;
    for (name, intent, active) in [
        ("(1) Edit prompt", Intent::Screen(0), a.screen == 0),
        ("(2) Presets", Intent::Screen(2), a.screen == 2),
        (
            "(3) Advanced",
            Intent::Advanced,
            [1, 3, 4].contains(&a.screen),
        ),
    ] {
        let width = (UnicodeWidthStr::width(name) + 2) as u16;
        if x + width <= bands[1].right() {
            let r = Rect::new(x, bands[1].y, width, 1);
            f.render_widget(
                Paragraph::new(format!(" {name} ")).style(if active {
                    a.theme.selected()
                } else {
                    c.muted
                }),
                r,
            );
            hit(a, r, intent);
            x += width;
        }
    }
    x = bands[1].x + 1;
    let mut y = bands[1].y + 1;
    let actions = if a.screen == 0 {
        vec![
            ("Add part", Intent::Edit(Action::Add)),
            ("New prompt", Intent::New),
            ("Save", Intent::Save),
            ("Undo", Intent::Undo),
            ("Redo", Intent::Redo),
        ]
    } else {
        vec![
            ("Save", Intent::Save),
            ("Undo", Intent::Undo),
            ("Review changes", Intent::Review),
            ("Back to editor", Intent::Screen(0)),
        ]
    };
    for (name, intent) in actions {
        let label = format!("[{name}]");
        let width = (UnicodeWidthStr::width(label.as_str()) + 1) as u16;
        if x + width > bands[1].right() {
            x = bands[1].x + 1;
            y += 1;
        }
        if y >= bands[1].y + 3 {
            break;
        }
        let r = Rect::new(x, y, width, 1);
        f.render_widget(Paragraph::new(label).style(c.accent), r);
        hit(a, r, intent);
        x += width;
    }
    x = bands[1].x + 1;
    for (p, name) in PANES[a.screen].iter().enumerate() {
        let label = format!(" [{name}] ");
        let width = UnicodeWidthStr::width(label.as_str()) as u16;
        if x + width <= bands[1].right() {
            let r = Rect::new(x, bands[1].y + 3, width, 1);
            f.render_widget(
                Paragraph::new(label).style(if a.pane() == p {
                    a.theme.selected()
                } else {
                    c.muted
                }),
                r,
            );
            hit(a, r, Intent::Pane(p));
            x += width;
        }
    }
    let context = if a.screen == 1 {
        format!(
            " {} · {} · /{}",
            a.workspace.path.display(),
            ["All", "Configured", "Active"][a.module_filter],
            a.filter
        )
    } else {
        format!(
            " {}{}",
            if [1, 3, 4].contains(&a.screen) {
                format!("Advanced / {} · ", SCREENS[a.screen])
            } else {
                String::new()
            },
            a.workspace.path.display()
        )
    };
    f.render_widget(Paragraph::new(context).style(c.muted), bands[2]);
    let content = crate::aurora::body(bands[3]);
    let count = PANES[a.screen].len();
    if f.area().width <= 80 || f.area().height <= 24 {
        draw_pane(f, a, c, content, a.pane());
    } else if f.area().width < 100 {
        let constraints = vec![Constraint::Ratio(1, count as u32); count];
        let rects = Layout::vertical(constraints).split(content);
        for (p, r) in rects.iter().enumerate() {
            draw_pane(f, a, c, *r, p);
        }
    } else {
        let constraints = if count == 3 {
            vec![
                Constraint::Percentage(28),
                Constraint::Percentage(34),
                Constraint::Percentage(38),
            ]
        } else {
            vec![Constraint::Percentage(65), Constraint::Percentage(35)]
        };
        let rects = Layout::horizontal(constraints).split(content);
        for (p, r) in rects.iter().enumerate() {
            draw_pane(f, a, c, *r, p);
        }
    }
    let footer = c.footer(f, bands[4]);
    let keys: Vec<(&str, &str)> = if a.raw_editing {
        vec![
            ("Ctrl+Enter", "apply"),
            ("Esc", "leave input"),
            ("F6", "diagnostics"),
        ]
    } else if a.screen == 3 && a.pane() == 0 {
        vec![
            ("Enter", "edit TOML"),
            ("Ctrl+S", "save"),
            ("Ctrl+K", "actions"),
        ]
    } else if a.rows().is_empty() {
        vec![
            ("PgUp/PgDn", "scroll"),
            ("Ctrl+K", "actions"),
            ("v", "scenario"),
            ("Ctrl+S", "save"),
        ]
    } else {
        match (a.screen, a.pane()) {
            (0, 0) => vec![
                ("Enter", "settings"),
                ("[ ]", "reorder"),
                ("r", "side"),
                ("Ctrl+K", "actions"),
                ("Ctrl+S", "save"),
            ],
            (1, 0) => vec![
                ("Enter", "properties"),
                ("/", "search"),
                ("f", "filter"),
                ("Ctrl+K", "actions"),
            ],
            (2, 0) => vec![
                ("Enter", "review preset"),
                ("Ctrl+K", "actions"),
                ("Ctrl+S", "save"),
            ],
            (4, 2) => vec![("Enter", "choose"), ("Ctrl+K", "actions"), ("q", "quit")],
            _ => vec![
                ("Enter", "edit"),
                ("←→", "sections"),
                ("F6", "pane"),
                ("Ctrl+S", "save"),
                ("Ctrl+K", "actions"),
                ("?", "help"),
                ("q", "quit"),
            ],
        }
    };
    let footer_chrome = Chrome {
        accent: c.accent.bg(a.theme.bg_deep),
        muted: c.muted.bg(a.theme.bg_deep),
        ..c
    };
    let line = fitted_keys(&keys, footer.width, footer_chrome);
    f.render_widget(
        Paragraph::new(line),
        Rect::new(footer.x, footer.y, footer.width, 1),
    );
    if footer.height > 1 {
        let status = if a.notice.is_empty() {
            fitted_keys(
                &[
                    ("n", "Create new"),
                    ("v", "scenario"),
                    ("Ctrl+Z/Y", "undo/redo"),
                ],
                footer.width,
                footer_chrome,
            )
        } else {
            Line::styled(a.notice.clone(), footer_chrome.muted)
        };
        f.render_widget(
            Paragraph::new(status).style(c.muted),
            Rect::new(
                footer.x,
                footer.y + 1,
                footer.width,
                footer.height.saturating_sub(1),
            ),
        );
    }
    if a.overlay.is_some() {
        draw_overlay(f, a, c);
    }
}
// The everyday studio has one stable layout: preview above, parts and values below.
fn draw_studio(f: &mut Frame, a: &mut App, c: Chrome, root: Rect) {
    let title_height = if root.height >= 30 { 5 } else { 1 };
    let preview_height = if root.height >= 30 { 8 } else { 6 };
    let [title, toolbar, preview, editing, body, footer] = Layout::vertical([
        Constraint::Length(title_height),
        Constraint::Length(if root.width < 68 { 2 } else { 1 }),
        Constraint::Length(preview_height),
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .areas(root);
    if title_height > 1 {
        c.title(
            f,
            title,
            "ASTER",
            "Prompt editor",
            if a.workspace.dirty() {
                "● Unsaved changes"
            } else {
                "✓ Saved"
            },
            c.data,
        );
    } else {
        f.render_widget(
            Paragraph::new(format!(
                " ASTER  ·  {}",
                if a.workspace.dirty() {
                    "Unsaved changes"
                } else {
                    "Saved"
                }
            ))
            .style(c.accent),
            title,
        );
    }
    let mut x = toolbar.x;
    let mut y = toolbar.y;
    let actions = if a.screen == 0 {
        vec![
            ("s Save", Intent::Save),
            ("u Undo", Intent::Undo),
            ("U Redo", Intent::Redo),
            ("a Add part", Intent::Edit(Action::Add)),
            ("p Presets", Intent::Screen(2)),
            ("t Source", Intent::Screen(3)),
        ]
    } else {
        vec![
            ("b Back", Intent::Screen(0)),
            ("a Use preset", Intent::AdoptPreset),
            ("s Save", Intent::Save),
        ]
    };
    for (label, intent) in actions {
        let (key, label) = label.split_once(' ').unwrap_or((label, ""));
        let text = format!("({key}) {label}");
        let width = (UnicodeWidthStr::width(text.as_str()) + 1) as u16;
        if x + width > toolbar.right() {
            x = toolbar.x;
            y += 1;
        }
        if y < toolbar.bottom() && x + width <= toolbar.right() {
            let r = Rect::new(x, y, width, 1);
            let line = Line::from(vec![
                Span::styled(
                    format!("({key})"),
                    c.data.add_modifier(ratatui::style::Modifier::BOLD),
                ),
                Span::styled(format!(" {label}"), c.accent),
            ]);
            f.render_widget(Paragraph::new(line), r);
            hit(a, r, intent);
            x += width;
        }
    }
    let block = panel("Live prompt — changes appear here".into(), c, false);
    let inside = block.inner(preview);
    f.render_widget(block, preview);
    let scenario = format!(" {} ▾ ", preview::SCENARIOS[a.scenario.index]);
    let sw = UnicodeWidthStr::width(scenario.as_str()) as u16;
    if sw + 22 < preview.width {
        let r = Rect::new(preview.right() - sw - 2, preview.y, sw, 1);
        f.render_widget(Paragraph::new(scenario).style(c.muted), r);
        hit(a, r, Intent::Scenarios);
    }
    match &a.preview.result {
        Some(Ok(text)) => {
            let text = text.strip_prefix("Left prompt\n").unwrap_or(text);
            let (left, right) = text.split_once("\n\nRight prompt\n").unwrap_or((text, ""));
            let right = right.trim_end();
            if right.trim().is_empty() {
                f.render_widget(Paragraph::new(preview_text(left, c)), inside);
            } else {
                let right_width = preview::render_ansi(right)
                    .lines
                    .iter()
                    .map(|line| line.width())
                    .max()
                    .unwrap_or(0)
                    .min((inside.width / 2) as usize) as u16;
                let [left_area, right_area] = Layout::horizontal([
                    Constraint::Min(1),
                    Constraint::Length(right_width.max(1)),
                ])
                .areas(inside);
                f.render_widget(Paragraph::new(preview_text(left, c)), left_area);
                f.render_widget(
                    Paragraph::new(preview_text(right, c))
                        .alignment(ratatui::layout::Alignment::Right),
                    right_area,
                );
            }
        }
        Some(Err(error)) => f.render_widget(
            Paragraph::new(error.clone())
                .style(c.muted)
                .wrap(Wrap { trim: false }),
            inside,
        ),
        None => f.render_widget(Paragraph::new("Updating preview…").style(c.muted), inside),
    }
    let selected = if a.screen == 2 {
        format!(
            "Previewing preset: {}",
            a.presets
                .get(a.selections[2][0])
                .map(String::as_str)
                .unwrap_or("None")
        )
    } else if a.prompt_settings {
        "Editing: Whole prompt".into()
    } else {
        let part = a.segments().get(a.selections[0][0]).cloned();
        format!(
            "Editing: {}{}",
            part.as_ref()
                .and_then(|p| p.module.as_deref())
                .map(editor::name)
                .unwrap_or_else(|| "Select a prompt part".into()),
            part.map(|p| if p.right {
                " · right prompt"
            } else {
                " · left prompt"
            })
            .unwrap_or("")
        )
    };
    let selected = if a.screen == 0 && a.pane() == 1 {
        let label = editor::controls(a)
            .get(a.selections[0][1])
            .map(|c| c.label.split(':').next().unwrap_or(&c.label).to_string())
            .unwrap_or_default();
        if label.is_empty() {
            selected
        } else {
            format!("{selected} → {label}")
        }
    } else {
        selected
    };
    f.render_widget(
        Paragraph::new(format!(" {selected}")).style(a.theme.selected()),
        editing,
    );
    a.editor_body = Some(body);
    let [parts, settings] = Layout::horizontal([
        Constraint::Length((body.width / 3).clamp(18, 28)),
        Constraint::Min(1),
    ])
    .areas(body);
    if a.screen == 2 {
        draw_pane(f, a, c, parts, 0);
        let block = panel("Selected preset".into(), c, false);
        let inner = block.inner(settings);
        f.render_widget(block, settings);
        let name = a
            .presets
            .get(a.selections[2][0])
            .cloned()
            .unwrap_or_else(|| "No presets available".into());
        f.render_widget(Paragraph::new(format!("{name}\n\nSee the prompt above.\n\nUse preset replaces your draft.\nUndo restores your previous prompt.")).style(c.muted).wrap(Wrap{trim:false}),inner);
    } else {
        let block = panel("1. Pick a part".into(), c, a.pane() == 0);
        let mut inner = block.inner(parts);
        f.render_widget(block, parts);
        if inner.height > 0 {
            let r = Rect::new(inner.x, inner.y, inner.width, 1);
            f.render_widget(
                Paragraph::new("Whole prompt…").style(if a.prompt_settings {
                    a.theme.selected()
                } else {
                    c.accent
                }),
                r,
            );
            hit(a, r, Intent::Edit(Action::WholePrompt));
            inner.y += 1;
            inner.height -= 1;
        }
        let rows = a.rows_for_parts();
        let selected = a.selections[0][0].min(rows.len().saturating_sub(1));
        a.selections[0][0] = selected;
        let start = selected.saturating_sub(inner.height.saturating_sub(1) as usize);
        for (offset, label) in rows
            .iter()
            .skip(start)
            .take(inner.height as usize)
            .enumerate()
        {
            let n = start + offset;
            let r = Rect::new(inner.x, inner.y + offset as u16, inner.width, 1);
            f.render_widget(
                Paragraph::new(format!(
                    "{}{}",
                    if n == selected && !a.prompt_settings {
                        "› "
                    } else {
                        "  "
                    },
                    label
                ))
                .style(if n == selected && !a.prompt_settings {
                    a.theme.selected()
                } else {
                    c.base
                }),
                r,
            );
            hit(a, r, Intent::FocusSelect(0, n));
        }
        let block = panel("2. Change its settings".into(), c, a.pane() == 1);
        let inner = block.inner(settings);
        f.render_widget(block, settings);
        let controls = editor::controls(a);
        let selected = a.selections[0][1].min(controls.len().saturating_sub(1));
        a.selections[0][1] = selected;
        let help_height = if inner.height >= 7 { 3 } else { 1 };
        let capacity = inner.height.saturating_sub(help_height) as usize;
        let start = selected.saturating_sub(capacity.saturating_sub(1));
        for (offset, control) in controls.iter().skip(start).take(capacity).enumerate() {
            let n = start + offset;
            let r = Rect::new(inner.x, inner.y + offset as u16, inner.width, 1);
            let mut line = Line::from(if n == selected { "› " } else { "  " });
            if matches!(control.action, Action::Color(_)) {
                if let Some((_, value)) = control.label.split_once(": ") {
                    let palette = a
                        .workspace
                        .doc
                        .get("palette")
                        .and_then(toml_edit::Item::as_str);
                    let resolved = palette
                        .and_then(|p| {
                            a.workspace
                                .doc
                                .get("palettes")?
                                .get(p)?
                                .get(value)?
                                .as_str()
                        })
                        .unwrap_or(value);
                    line.spans.push(match color_swatch(resolved) {
                        Some(color) => Span::styled("██ ", Style::default().fg(color)),
                        None => Span::styled("░░ ", c.muted),
                    });
                }
            }
            line.spans.push(Span::raw(control.label.clone()));
            f.render_widget(
                Paragraph::new(line).style(if n == selected && a.pane() == 1 {
                    a.theme.selected()
                } else {
                    c.base
                }),
                r,
            );
            hit(a, r, Intent::FocusSelect(1, n));
        }
        if let Some(control) = controls.get(selected) {
            let r = Rect::new(
                inner.x,
                inner.bottom().saturating_sub(help_height),
                inner.width,
                help_height,
            );
            f.render_widget(
                Paragraph::new(control.help.clone())
                    .style(c.muted)
                    .wrap(Wrap { trim: false }),
                r,
            );
        }
    }
    f.render_widget(
        Paragraph::new(fitted_keys(
            &[
                ("Tab", "sides"),
                ("Enter", "edit"),
                ("Ctrl+S", "save"),
                ("q", "quit"),
            ],
            footer.width,
            Chrome {
                accent: c.data,
                muted: c.accent,
                ..c
            },
        )),
        Rect::new(footer.x, footer.y, footer.width, 1),
    );
    let status = if a.notice.is_empty() {
        a.workspace.path.display().to_string()
    } else {
        a.notice.clone()
    };
    f.render_widget(
        Paragraph::new(status).style(c.muted),
        Rect::new(footer.x, footer.y + 1, footer.width, 1),
    );
    if a.overlay.is_some() {
        draw_overlay(f, a, c);
    }
}
fn fitted_keys(items: &[(&str, &str)], width: u16, c: Chrome) -> Line<'static> {
    let mut fit = vec![];
    let mut used = 0;
    for (k, v) in items {
        let len = UnicodeWidthStr::width(*k)
            + UnicodeWidthStr::width(*v)
            + 3
            + if fit.is_empty() { 0 } else { 2 };
        if used + len > width as usize {
            break;
        }
        used += len;
        fit.push((*k, *v));
    }
    shortcut_line(&fit, c.accent, c.muted)
}
fn draw_pane(f: &mut Frame, a: &mut App, c: Chrome, area: Rect, p: usize) {
    let focus = a.pane() == p;
    let block = panel(
        format!(
            "{}{}",
            PANES[a.screen][p],
            if focus { " · Focus" } else { "" }
        ),
        c,
        focus,
    );
    let mut inner = block.inner(area);
    f.render_widget(block, area);
    hit(a, area, Intent::Pane(p));
    if inner.is_empty() {
        return;
    }
    if [0, 1, 2].contains(&a.screen) && p == 2 && inner.height >= 4 {
        let label = format!("[Scenario: {} ▾]", preview::SCENARIOS[a.scenario.index]);
        let r = Rect::new(inner.x, inner.y, inner.width, 1);
        f.render_widget(Paragraph::new(label).style(c.accent), r);
        hit(a, r, Intent::Scenarios);
        inner.y += 1;
        inner.height -= 1;
    }
    if a.screen == 1 && p == 1 && inner.height > 4 {
        if let Some(module) = a.module() {
            let rendered = a
                .preview
                .visibility
                .as_ref()
                .filter(|(n, _)| n == &module)
                .map(|(_, visible)| {
                    if *visible {
                        "Visible in this preview"
                    } else {
                        "Hidden in this preview"
                    }
                })
                .unwrap_or("Shown when detection conditions match");
            let description = module_description(&module);
            let text = format!(
                "{description}\n{} · {}\n{rendered} · conditional on detection",
                if a.enabled(&module) {
                    "Turned on"
                } else {
                    "Turned off"
                },
                if a.included(&module) {
                    "Included in prompt"
                } else {
                    "Not in prompt"
                }
            );
            f.render_widget(
                Paragraph::new(text).style(c.muted),
                Rect::new(inner.x, inner.y, inner.width, 3),
            );
            inner.y += 4;
            inner.height -= 4;
        }
    }
    if a.screen == 1 && p == 1 && inner.height >= 5 {
        if let Some((_, key, val, _)) = a.properties().get(a.selections[1][1]) {
            let kind = if ["true", "false"].contains(&val.as_str()) {
                "boolean"
            } else if val.trim().parse::<i64>().is_ok() {
                "number"
            } else if val.trim().starts_with('[') {
                "array"
            } else {
                "TOML value"
            };
            let text = format!("{key} ({kind}) — {}", editor::property_help(key));
            let height = 3.min(inner.height / 2);
            f.render_widget(
                Paragraph::new(text)
                    .style(c.muted)
                    .wrap(Wrap { trim: false }),
                Rect::new(inner.x, inner.bottom() - height, inner.width, height),
            );
            inner.height -= height;
        }
    }
    if a.screen == 0 && p == 1 && inner.height >= 4 {
        let mut x = inner.x;
        let mut y = inner.y;
        for (n, label) in editor::GROUPS.iter().enumerate() {
            let label = format!(" {label} ");
            let width = UnicodeWidthStr::width(label.as_str()) as u16;
            if x + width > inner.right() {
                x = inner.x;
                y += 1;
            }
            if y >= inner.bottom().saturating_sub(2) {
                break;
            }
            let r = Rect::new(x, y, width.min(inner.width), 1);
            f.render_widget(
                Paragraph::new(label).style(if a.setting_group == n {
                    a.theme.selected()
                } else {
                    c.muted
                }),
                r,
            );
            hit(a, r, Intent::Edit(Action::Group(n)));
            x += width;
        }
        let used = y - inner.y + 1;
        inner.y += used;
        inner.height = inner.height.saturating_sub(used);
        let module = a.module();
        let label = module
            .as_deref()
            .map(editor::name)
            .unwrap_or_else(|| "Prompt layout".into());
        f.render_widget(
            Paragraph::new(label).style(c.data),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        inner.y += 1;
        inner.height -= 1;
    }
    if a.screen == 0 && [0, 1].contains(&p) && inner.height >= 4 {
        let help = if p == 0 {
            a.module()
                .map(|m| {
                    format!(
                        "{}\n{} Select a part, then open Settings (Enter).",
                        a.part_status(&m),
                        editor::description(&m)
                    )
                })
                .unwrap_or_else(|| {
                    "Custom decorations are preserved. Edit them under Advanced → Edit format."
                        .into()
                })
        } else {
            editor::controls(a)
                .get(a.selections[0][1])
                .map(|c| c.help.clone())
                .unwrap_or_else(|| "Select an option and press Enter to change it.".into())
        };
        let height = if inner.height >= 10 { 3 } else { 2 };
        let r = Rect::new(inner.x, inner.bottom() - height, inner.width, height);
        f.render_widget(
            Paragraph::new(help)
                .style(c.muted)
                .wrap(Wrap { trim: false }),
            r,
        );
        inner.height -= height;
    }
    let current = a.pane();
    a.panes[a.screen] = p;
    let rows = a.rows();
    a.panes[a.screen] = current;
    if !rows.is_empty() {
        let selected = a.selections[a.screen][p].min(rows.len() - 1);
        a.selections[a.screen][p] = selected;
        let height = inner.height as usize;
        let start = selected
            .saturating_sub(height.saturating_sub(1))
            .min(rows.len().saturating_sub(height));
        for (offset, text) in rows.iter().skip(start).take(height).enumerate() {
            let n = start + offset;
            let r = Rect::new(inner.x, inner.y + offset as u16, inner.width, 1);
            let prefix = if n == selected { "› " } else { "  " };
            f.render_widget(
                Paragraph::new(format!("{prefix}{text}")).style(if n == selected {
                    a.theme.selected()
                } else {
                    c.base
                }),
                r,
            );
            hit(a, r, Intent::FocusSelect(p, n));
        }
        return;
    }
    let scroll = a.scrolls[a.screen][p].min(u16::MAX as usize) as u16;
    match (a.screen, p) {
        (0, 2) | (1, 2) | (2, 2) => {
            let compact = inner.height < 10;
            let metadata = format!(
                "status {} · {} ms · {} jobs\n{}",
                a.scenario.status, a.scenario.duration, a.scenario.jobs, a.scenario.directory
            );
            let intro = if compact {
                format!(
                    "{} · {} cols\n",
                    preview::SCENARIOS[a.scenario.index],
                    a.scenario.width
                )
            } else {
                format!(
                    "{} · {} columns\n{metadata}\n\n",
                    preview::SCENARIOS[a.scenario.index],
                    a.scenario.width
                )
            };
            let mut text = Text::from(intro);
            match &a.preview.result {
                Some(Ok(s)) => text.extend(preview_text(s, c).lines),
                Some(Err(e)) => text.extend(Text::styled(e.clone(), c.accent).lines),
                None => text.extend(Text::styled("Rendering… (Ctrl+X) cancel", c.muted).lines),
            }
            if compact {
                text.extend(Text::styled(format!("\n{metadata}"), c.muted).lines);
            }
            text.extend(Text::styled("\nConditional modules appear only when detected in this directory.\nCustom commands are disabled until approved in Actions.",c.muted).lines);
            f.render_widget(
                Paragraph::new(text).style(c.base).scroll((
                    scroll,
                    a.horizontal[a.screen][p].min(u16::MAX as usize) as u16,
                )),
                inner,
            );
        }
        (2, 1) => {
            let text=a.preset_text.as_ref().map(|(n,s)|format!("{n}\n\nReplacement is staged. (Enter) review, (a) adopt in dialog.\n\n{s}")).unwrap_or_else(||"Select a preset.\n\nAster originals appear first, followed by installed Starship presets.".into());
            f.render_widget(
                Paragraph::new(text)
                    .style(c.muted)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );
        }
        (3, 0) => draw_toml(f, a, c, inner),
        (3, 1) => {
            let text = diagnostics(a);
            f.render_widget(
                Paragraph::new(text)
                    .style(c.muted)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );
        }
        (4, 0) => {
            let text=format!("{}\n\nTarget\n{}\n\nBackup\n{}\n\nExisting targets get a unique backup before atomic replacement. External changes block saving. Save as requires a new path.\n\nCustom preview approval: {}",diagnostics(a),a.workspace.path.display(),a.workspace.backup.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||"Created beside target on save".into()),if a.approved==preview::custom_definitions(&a.workspace.doc.to_string())&&!a.approved.is_empty(){"approved for current definitions"}else{"not approved"});
            f.render_widget(
                Paragraph::new(text)
                    .style(c.muted)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );
        }
        (4, 1) => {
            let diff = TextDiff::from_lines(&a.workspace.baseline, &a.workspace.raw);
            let lines = diff
                .iter_all_changes()
                .map(|ch| {
                    let (prefix, style) = match ch.tag() {
                        ChangeTag::Delete => ("−", Style::default().fg(a.theme.error)),
                        ChangeTag::Insert => ("+", c.data),
                        ChangeTag::Equal => (" ", c.muted),
                    };
                    Line::styled(
                        format!("{prefix}{}", ch.value().trim_end_matches('\n')),
                        style,
                    )
                })
                .collect::<Vec<_>>();
            f.render_widget(
                Paragraph::new(lines).scroll((
                    scroll,
                    a.horizontal[a.screen][p].min(u16::MAX as usize) as u16,
                )),
                inner,
            );
        }
        _ => {
            let text = match (a.screen, p) {
                (0, 0) => "No prompt parts yet. Choose Add part or New prompt above.",
                (1, 0) => "No matching modules. (/) search, (f) change filter.",
                (0, 1) | (1, 1) => "No properties. Actions can add a TOML property.",
                (2, 0) => "No presets available. Install Starship and reopen Aster.",
                _ => "Use Actions for the available controls.",
            };
            f.render_widget(
                Paragraph::new(text)
                    .style(c.muted)
                    .wrap(Wrap { trim: false }),
                inner,
            );
        }
    }
}
fn diagnostics(a: &App) -> String {
    if let Some(e) = &a.workspace.load_error {
        return format!(
            "Source unavailable\n{e}\n\nFix file access and reopen. Saving is blocked."
        );
    }
    if let Some(e) = &a.workspace.error {
        return format!("Invalid TOML\n{e}\n\nText retained. (Ctrl+Enter) apply after correction.");
    }
    if let Err(e) = crate::config::validate(&a.workspace.doc) {
        return format!("Invalid property\n{e}\n\nCorrect the value before saving.");
    }
    let raw = a.workspace.raw != a.workspace.doc.to_string();
    format!("✓ TOML parses\n{}\n\nStarship preview diagnostics appear in Preview. Enabled modules may be absent when directory or runtime conditions are unmet.\n\n(Enter) edit TOML\n(Ctrl+Enter) apply\n(Esc) leave input\n(F6) change pane",if raw{"Raw changes await Apply before other controls"}else{"Shared draft is current"})
}
fn draw_toml(f: &mut Frame, a: &mut App, c: Chrome, area: Rect) {
    let line = a.workspace.raw[..a.raw_cursor.min(a.workspace.raw.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count();
    let start = if a.raw_editing {
        line.saturating_sub(area.height as usize / 2)
    } else {
        a.scrolls[3][0]
    };
    let start_byte = a.workspace.raw[..a.raw_cursor.min(a.workspace.raw.len())]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let column = UnicodeWidthStr::width(
        &a.workspace.raw[start_byte..a.raw_cursor.min(a.workspace.raw.len())],
    );
    let horizontal = if a.raw_editing {
        column.saturating_sub(area.width.saturating_sub(8) as usize)
    } else {
        a.horizontal[3][0]
    };
    let rows = a
        .workspace
        .raw
        .lines()
        .enumerate()
        .skip(start)
        .take(area.height as usize)
        .map(|(i, s)| {
            let style = if s.trim_start().starts_with('#') {
                c.muted
            } else if s.trim_start().starts_with('[') {
                c.accent
            } else {
                c.data
            };
            Line::from(vec![
                Span::styled(format!("{:>4}  ", i + 1), c.muted),
                Span::styled(s.to_string(), style),
            ])
        })
        .collect::<Vec<_>>();
    f.render_widget(
        Paragraph::new(rows).scroll((0, horizontal.min(u16::MAX as usize) as u16)),
        area,
    );
    if a.raw_editing {
        let start_byte = a.workspace.raw[..a.raw_cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let column = UnicodeWidthStr::width(&a.workspace.raw[start_byte..a.raw_cursor]);
        let x = area.x + 6 + column.saturating_sub(horizontal) as u16;
        let y = area.y + (line - start) as u16;
        if x < area.right() && y < area.bottom() {
            f.set_cursor_position((x, y));
        }
    }
}
fn draw_overlay(f: &mut Frame, a: &mut App, c: Chrome) {
    a.hits.clear();
    let available = a.editor_body.unwrap_or(f.area());
    let padding = if a.editor_body.is_some() { 0 } else { 4 };
    let width = available.width.saturating_sub(padding).min(100);
    let height = available.height.saturating_sub(padding).min(30);
    let area = Rect::new(
        available.x + (available.width - width) / 2,
        available.y + (available.height - height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, area);
    let title = match a.overlay.as_ref().unwrap() {
        Overlay::Menu {
            navigation: true, ..
        } => "Navigation",
        Overlay::Menu { .. } => "Actions",
        Overlay::Picker { title, .. } => title,
        Overlay::Input(i) => &i.title,
        Overlay::Wizard => "Create new · staged",
        Overlay::Help => "Help",
        Overlay::Quit => "Unsaved changes",
        Overlay::Approval => "Approve custom commands",
        Overlay::Preset(_) => "Replace draft with preset",
    }
    .to_string();
    hit(a, area, Intent::Scroll(0));
    let block = panel(title, c, true);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height < 2 {
        return;
    }
    let [body, buttons] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).areas(inner);
    let mut key_items = vec![("Esc", "cancel")];
    match a.overlay.as_ref().unwrap() {
        Overlay::Picker {
            help,
            query,
            selected,
            ..
        } => {
            let help = help.clone();
            let query = query.clone();
            let selected = *selected;
            let items = a.picker_items();
            let header: u16 = if body.height >= 8 { 3 } else { 2 };
            f.render_widget(
                Paragraph::new(help)
                    .style(c.muted)
                    .wrap(Wrap { trim: false }),
                Rect::new(
                    body.x,
                    body.y,
                    body.width,
                    header.saturating_sub(1).min(body.height),
                ),
            );
            if body.height >= header {
                f.render_widget(
                    Paragraph::new(format!("Search: {query}")).style(c.data),
                    Rect::new(body.x, body.y + header - 1, body.width, 1),
                );
            }
            let capacity = body.height.saturating_sub(header) as usize;
            let start = selected.saturating_sub(capacity.saturating_sub(1));
            for (offset, (label, intent)) in items.iter().skip(start).take(capacity).enumerate() {
                let i = start + offset;
                let r = Rect::new(body.x, body.y + header + offset as u16, body.width, 1);
                let mut line = Line::from(vec![Span::styled(
                    if i == selected { "› " } else { "  " },
                    if i == selected {
                        a.theme.selected()
                    } else {
                        c.base
                    },
                )]);
                if let Intent::SetColor(_, color) = intent {
                    if let Some(color) = color_swatch(color) {
                        line.spans
                            .push(Span::styled("■ ", Style::default().fg(color)));
                    }
                }
                if let Intent::SetPalette(n) = intent {
                    for color in editor::PALETTES[*n].1.iter().take(3) {
                        if let Some(color) = color_swatch(color) {
                            line.spans
                                .push(Span::styled("■", Style::default().fg(color)));
                        }
                    }
                    line.spans.push(Span::raw(" "));
                }
                line.spans.push(Span::styled(
                    label.clone(),
                    if i == selected {
                        a.theme.selected()
                    } else {
                        c.base
                    },
                ));
                f.render_widget(Paragraph::new(line), r);
                hit(a, r, Intent::Select(i));
            }
            key_items.insert(0, ("Enter", "choose"));
        }
        Overlay::Menu {
            navigation,
            query,
            selected,
        } => {
            let selected = *selected;
            let items = a.menu(*navigation, query);
            let text = format!("Search: {query}");
            f.render_widget(
                Paragraph::new(text).style(c.data),
                Rect::new(body.x, body.y, body.width, 1),
            );
            let capacity = body.height.saturating_sub(1) as usize;
            let start = selected.saturating_sub(capacity.saturating_sub(1));
            for (offset, (label, _)) in items.iter().skip(start).take(capacity).enumerate() {
                let i = start + offset;
                let r = Rect::new(body.x, body.y + 1 + offset as u16, body.width, 1);
                f.render_widget(
                    Paragraph::new(format!("{} {label}", if i == selected { "›" } else { " " }))
                        .style(if i == selected {
                            a.theme.selected()
                        } else {
                            c.base
                        }),
                    r,
                );
                hit(a, r, Intent::Select(i));
            }
            key_items.insert(0, ("Enter", "choose"));
        }
        Overlay::Input(i) => {
            let column = UnicodeWidthStr::width(&i.text[..i.cursor]);
            let horizontal = column.saturating_sub(body.width.saturating_sub(1) as usize);
            f.render_widget(
                Paragraph::new(i.text.clone())
                    .style(c.data)
                    .scroll((0, horizontal.min(u16::MAX as usize) as u16)),
                Rect::new(body.x, body.y, body.width, 1),
            );
            if body.height > 2 {
                let hint=match &i.target {
                    crate::app::InputTarget::Plain(_)=>"Enter the text or symbol directly. Leave it empty to remove the text. Enter applies; Esc cancels.",
                    crate::app::InputTarget::Number(_)=>"Enter a whole number, zero or greater. Enter applies; Esc cancels.",
                    crate::app::InputTarget::Color(_)=>"Enter a hex color, for example #7dcfff. Enter applies; Esc cancels.",
                    crate::app::InputTarget::Property(_)=>"Advanced TOML value: strings use quotes; arrays use [ ]. Space toggles booleans. Esc cancels.",
                    _=>"Enter applies this value; Esc keeps the previous value.",
                };
                let explanation = if i.help.is_empty() {
                    hint.into()
                } else {
                    format!("{}\n\n{hint}", i.help)
                };
                let info = i.error.as_deref().unwrap_or(&explanation);
                f.render_widget(
                    Paragraph::new(info)
                        .style(c.muted)
                        .wrap(Wrap { trim: false }),
                    Rect::new(body.x, body.y + 2, body.width, body.height - 2),
                );
            }
            if body.width > 0 {
                f.set_cursor_position((
                    body.x + (column - horizontal).min(body.width as usize - 1) as u16,
                    body.y,
                ));
            }
            key_items.insert(0, ("Enter", "apply"));
        }
        Overlay::Quit => {
            f.render_widget(Paragraph::new(format!("Draft changes have not been saved.\n{}\n\nReview before saving, discard to quit, or cancel to keep editing.",a.workspace.path.display())).style(c.muted).wrap(Wrap{trim:false}),body);
            key_items = vec![("r", "review"), ("d", "discard & quit"), ("Esc", "cancel")];
        }
        Overlay::Approval => {
            let text=format!("Approve these definitions for this session? Starship may execute command and when scripts in the preview directory. Changes revoke approval.\n\n{}",preview::custom_definitions(&a.workspace.doc.to_string()));
            f.render_widget(
                Paragraph::new(text)
                    .style(c.muted)
                    .wrap(Wrap { trim: false })
                    .scroll((a.overlay_scroll as u16, 0)),
                body,
            );
            key_items.insert(0, ("a", "approve"));
        }
        Overlay::Preset(s) => {
            let text = format!("This replaces the valid draft. Undo restores it.\n\n{s}");
            f.render_widget(
                Paragraph::new(text)
                    .style(c.muted)
                    .wrap(Wrap { trim: false })
                    .scroll((a.overlay_scroll as u16, 0)),
                body,
            );
            key_items.insert(0, ("a", "adopt"));
        }
        Overlay::Help => {
            f.render_widget(Paragraph::new("1: Edit prompt   2: Presets   3: Advanced\nTab: next main area   F2: searchable Navigation\nF6 / Shift+F6: panes   Ctrl+K: actions\nArrows / j / k: select   PgUp / PgDn: scroll\nEnter: edit   Space: enable / disable module\nCtrl+S: Save   Ctrl+Z / Ctrl+Y: undo / redo\n/: module search   f: All / Configured / Active\nn: guided builder   v: preview scenario\n[ / ]: reorder piece   r: left / right placement\nAdvanced: Delete removes the selected property\n← / →: horizontal scroll in Preview and TOML\nTOML: Enter to type, Ctrl+Enter applies; F6 leaves input\nCtrl+X: cancel preview   q / Ctrl+C: guarded quit\n\nActions expose formats, scenario values, new properties, appearance, presets and custom command approval.\n\nEditor appearance is independent of prompt colors.").style(c.muted).wrap(Wrap{trim:false}).scroll((a.overlay_scroll as u16,0)),body);
        }
        Overlay::Wizard => {
            let wizard = a.wizard.as_ref().unwrap();
            let step = wizard.step;
            let question = wizard.question("");
            let progress = wizard.progress().1.join(" › ");
            f.render_widget(
                Paragraph::new(format!("{} · {question}\n{progress}", step + 1)).style(c.accent),
                Rect::new(body.x, body.y, body.width, 2.min(body.height)),
            );
            let mut content = Rect::new(
                body.x,
                body.y + 2,
                body.width,
                body.height.saturating_sub(2),
            );
            let preview_area = if content.width >= 85 {
                let [controls, preview] =
                    Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)])
                        .areas(content);
                content = controls;
                Some(preview)
            } else if a.wizard_preview {
                Some(content)
            } else {
                None
            };
            if let Some(preview_area) = preview_area {
                let block = panel("Live staged preview".into(), c, a.wizard_preview);
                let inner = block.inner(preview_area);
                f.render_widget(block, preview_area);
                let text = match &a.preview.result {
                    Some(Ok(s)) => preview_text(s, c),
                    Some(Err(e)) => Text::styled(e.clone(), c.muted),
                    None => Text::styled("Rendering…", c.muted),
                };
                f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
            }
            if content.width < 85 && a.wizard_preview && body.width < 85 {
                content.height = 0;
            }

            if !wizard.is_confirm() && wizard.hex.is_none() && content.height >= 5 {
                let detail = wizard.choice(wizard.cursor()).2;
                let detail_area = Rect::new(content.x, content.bottom() - 3, content.width, 3);
                f.render_widget(
                    Paragraph::new(detail)
                        .style(c.muted)
                        .wrap(Wrap { trim: false }),
                    detail_area,
                );
                content.height = content.height.saturating_sub(4);
            }
            if wizard.is_confirm() {
                let summary = wizard
                    .summary()
                    .into_iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                f.render_widget(Paragraph::new(format!("{summary}\n\nFinish adopts this prompt without saving. Cancel retains the existing draft.")).style(c.data).wrap(Wrap{trim:false}),content);
            } else if let Some(hex) = wizard.hex_value() {
                f.render_widget(
                    Paragraph::new(format!("Color: {hex}")).style(c.data),
                    content,
                );
            } else {
                let capacity = content.height as usize;
                let selected = wizard.cursor();
                let start = selected.saturating_sub(capacity.saturating_sub(1));
                for (offset, i) in (start..wizard.choice_count()).take(capacity).enumerate() {
                    let (label, state, _) = wizard.choice(i);
                    let text = format!(
                        "{} {label} · {state}",
                        if i == selected { "›" } else { " " }
                    );
                    f.render_widget(
                        Paragraph::new(text).style(if i == selected {
                            a.theme.selected()
                        } else {
                            c.base
                        }),
                        Rect::new(content.x, content.y + offset as u16, content.width, 1),
                    );
                    a.hits.push(Hit {
                        area: Rect::new(content.x, content.y + offset as u16, content.width, 1),
                        intent: Intent::BuilderSelect(i),
                    });
                }
            }
            key_items = if wizard.hex.is_some() {
                vec![("Enter", "keep"), ("Esc", "cancel color")]
            } else {
                match step {
                    2 => vec![
                        ("Space", "include"),
                        ("[ ]", "reorder"),
                        ("Enter", "next"),
                        ("F6", "preview"),
                        ("Esc", "back"),
                        ("x", "cancel"),
                    ],
                    3 => vec![
                        ("←→", "color"),
                        ("c", "hex"),
                        ("t", "light/dark"),
                        ("Enter", "next"),
                        ("Esc", "back"),
                        ("x", "cancel"),
                    ],
                    _ => vec![
                        ("Enter", if step == 5 { "adopt" } else { "next" }),
                        ("F6", "preview"),
                        ("Esc", "back"),
                        ("x", "cancel"),
                    ],
                }
            };
        }
    }
    let builder = matches!(a.overlay, Some(Overlay::Wizard));
    let mut row = buttons.y;
    let mut x = buttons.x;
    let mut current = vec![];
    for (key, label) in key_items {
        let width = (UnicodeWidthStr::width(key) + UnicodeWidthStr::width(label) + 3) as u16;
        if x + width > buttons.right() {
            f.render_widget(
                Paragraph::new(shortcut_line(&current, c.accent, c.muted)),
                Rect::new(buttons.x, row, buttons.width, 1),
            );
            current.clear();
            row += 1;
            x = buttons.x;
        }
        if row >= buttons.bottom() || width > buttons.width {
            break;
        }
        let intent = if builder {
            match key {
                "Enter" => Intent::BuilderNext,
                "F6" => Intent::BuilderPreview,
                "Space" => Intent::BuilderToggle,
                "←→" => Intent::BuilderColor(1),
                "c" => Intent::BuilderHex,
                "t" => Intent::BuilderText,
                "[" => Intent::BuilderReorder(-1),
                "]" => Intent::BuilderReorder(1),
                "Esc" => Intent::BuilderBack,
                "x" => Intent::BuilderCancel,
                _ => Intent::ModalCancel,
            }
        } else {
            match key {
                "Enter" => Intent::ModalAccept,
                "a" => Intent::ModalApprove,
                "d" => Intent::ModalDiscard,
                "r" => Intent::ModalReview,
                _ => Intent::ModalCancel,
            }
        };
        hit(a, Rect::new(x, row, width, 1), intent);
        current.push((key, label));
        x += width + 2;
    }
    if !current.is_empty() && row < buttons.bottom() {
        f.render_widget(
            Paragraph::new(shortcut_line(&current, c.accent, c.muted)),
            Rect::new(buttons.x, row, buttons.width, 1),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    #[test]
    fn toolbar_hotkeys_and_short_descriptions() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().join("prompt.toml"), false, false);
        a.workspace
            .edit(|d| d["format"] = toml_edit::value("$directory$character"));
        let draft = a.workspace.raw.clone();
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        a.on_key(key('u'));
        assert_ne!(a.workspace.raw, draft);
        a.on_key(key('U'));
        assert_eq!(a.workspace.raw, draft);
        a.on_key(key('a'));
        let items = a.picker_items();
        assert!(!items.is_empty());
        assert!(items.iter().all(|(label, _)| label.contains(" · ")));
        assert!(items
            .iter()
            .any(|(label, _)| label == "Programming languages · Detected language versions"));
        a.on_key(key('u'));
        assert_eq!(a.workspace.raw, draft);
        a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        a.on_key(key('p'));
        assert_eq!(a.screen, 2);
        a.on_key(key('b'));
        assert_eq!(a.screen, 0);
        a.on_key(key('t'));
        assert_eq!(a.screen, 3);
        a.on_key(key('b'));
        assert_eq!(a.screen, 0);
        let mut t = Terminal::new(TestBackend::new(60, 20)).unwrap();
        t.draw(|f| draw(f, &mut a)).unwrap();
        for intent in [
            Intent::Save,
            Intent::Undo,
            Intent::Redo,
            Intent::Edit(Action::Add),
            Intent::Screen(2),
            Intent::Screen(3),
        ] {
            assert!(a.hits.iter().any(|hit| hit.intent == intent));
        }
        a.on_key(key('s'));
        assert!(!a.workspace.dirty());
        assert!(a.workspace.path.exists());
    }
    #[test]
    fn simple_editor_layout_and_clicks() {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().join("prompt.toml"), false, false);
        a.workspace
            .adopt(crate::presets::get("Aster · Prism").unwrap().to_string())
            .unwrap();
        a.preview.result = Some(Ok(
            "Left prompt\n╭─ ✦ ~/Projects  main\n╰─ ❯\n\nRight prompt\n".into(),
        ));
        for (w, h) in [(120, 40), (100, 30), (80, 24), (60, 20)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            let mut pinned = None;
            for p in [0, 1] {
                a.panes[0] = p;
                t.draw(|f| draw(f, &mut a)).unwrap();
                let body = a.editor_body.unwrap();
                if let Some(previous) = pinned {
                    assert_eq!(body.y, previous);
                } else {
                    pinned = Some(body.y);
                }
                assert!(a.hits.iter().any(|hit| hit.intent == Intent::Save));
                assert!(a
                    .hits
                    .iter()
                    .any(|hit| hit.intent == Intent::FocusSelect(0, 0)));
                assert!(a
                    .hits
                    .iter()
                    .any(|hit| hit.intent == Intent::FocusSelect(1, 1)));
                assert!(!a.hits.iter().any(|hit| matches!(
                    hit.intent,
                    Intent::Edit(Action::Group(_)) | Intent::Pane(2)
                )));
                assert!(a
                    .hits
                    .iter()
                    .all(|hit| hit.area.right() <= w && hit.area.bottom() <= h));
            }
            a.dispatch(Intent::Edit(Action::Text(editor::path(
                "directory",
                "read_only",
            ))));
            t.draw(|f| draw(f, &mut a)).unwrap();
            assert_eq!(a.editor_body.unwrap().y, pinned.unwrap());
            assert!(a
                .hits
                .iter()
                .all(|hit| hit.area.y >= a.editor_body.unwrap().y));
            let screen = t
                .backend()
                .buffer()
                .content
                .chunks(w as usize)
                .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
                .collect::<Vec<_>>();
            assert!(screen.iter().any(|line| line.contains("~/Projects")));
            assert!(screen
                .iter()
                .any(|line| line.contains("Editing: Current folder")));
            a.overlay = None;
            if w == 80 {
                t.draw(|f| draw(f, &mut a)).unwrap();
                println!(
                    "{}",
                    t.backend()
                        .buffer()
                        .content
                        .chunks(w as usize)
                        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
        let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
        a.panes[0] = 1;
        t.draw(|f| draw(f, &mut a)).unwrap();
        let hit = a
            .hits
            .iter()
            .find(|hit| hit.intent == Intent::FocusSelect(1, 1))
            .unwrap()
            .clone();
        a.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.area.x,
            row: hit.area.y,
            modifiers: KeyModifiers::NONE,
        });
        assert!(matches!(a.overlay, Some(Overlay::Picker { .. })));
        t.draw(|f| draw(f, &mut a)).unwrap();
        let hit = a
            .hits
            .iter()
            .find(|hit| hit.intent == Intent::Select(3))
            .unwrap()
            .clone();
        a.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.area.x,
            row: hit.area.y,
            modifiers: KeyModifiers::NONE,
        });
        assert!(a.overlay.is_none());
        assert!(a.workspace.raw.contains("fg:green"));
    }
    #[test]
    fn all_screens_panes_and_overlays_resize() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().join("s.toml"), false, false);
        a.workspace.edit(|d| {
            d["format"] = toml_edit::value("$directory${custom.界}$character");
            d["custom"]["界"]["command"] = toml_edit::value("echo x");
        });
        for (w, h) in [
            (120, 40),
            (100, 30),
            (80, 24),
            (60, 20),
            (22, 11),
            (1, 1),
            (60, 20),
            (120, 40),
        ] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            for s in 0..5 {
                a.screen = s;
                for p in 0..PANES[s].len() {
                    a.panes[s] = p;
                    t.draw(|f| draw(f, &mut a)).unwrap();
                    assert!(a
                        .hits
                        .iter()
                        .all(|hit| hit.area.x + hit.area.width <= w
                            && hit.area.y + hit.area.height <= h));
                }
            }
            for intent in [
                Intent::Actions,
                Intent::Navigation,
                Intent::Help,
                Intent::Approve,
                Intent::Quit,
                Intent::EditLeft,
                Intent::New,
            ] {
                a.dispatch(intent);
                t.draw(|f| draw(f, &mut a)).unwrap();
                a.overlay = None;
                a.wizard = None;
            }
            a.overlay = Some(Overlay::Preset(
                "fixture replacement\nformat = '$character'".into(),
            ));
            t.draw(|f| draw(f, &mut a)).unwrap();
            a.overlay = None;
            a.dispatch(Intent::New);
            for step in 0..6 {
                a.wizard.as_mut().unwrap().step = step;
                t.draw(|f| draw(f, &mut a)).unwrap();
            }
            a.wizard.as_mut().unwrap().hex = Some("#B5A0FF".into());
            t.draw(|f| draw(f, &mut a)).unwrap();
            a.overlay = None;
            a.wizard = None;
            a.dispatch(Intent::AddKey);
            if let Some(Overlay::Input(i)) = &mut a.overlay {
                i.text = "界🚀".repeat(100);
                i.cursor = i.text.len();
                i.error = Some("Invalid value; original draft retained".into());
            }
            t.draw(|f| draw(f, &mut a)).unwrap();
            a.overlay = None;
            a.screen = 3;
            a.panes[3] = 0;
            a.raw_editing = true;
            a.raw_cursor = a.workspace.raw.len();
            t.draw(|f| draw(f, &mut a)).unwrap();
            a.raw_editing = false;
        }
    }
    #[test]
    fn mouse_properties_and_guarded_modal_targets() {
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().join("s.toml"), false, false);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        a.screen = 0;
        a.panes[0] = 1;
        terminal.draw(|f| draw(f, &mut a)).unwrap();
        let row = a
            .hits
            .iter()
            .find(|h| matches!(h.intent, Intent::FocusSelect(1, 1)))
            .unwrap()
            .clone();
        a.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: row.area.x,
            row: row.area.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(a.selected(), 1);
        assert_eq!(a.pane(), 1);
        a.dispatch(Intent::AddKey);
        if let Some(Overlay::Input(input)) = &mut a.overlay {
            input.text = "rust.disabled = 7".into();
            input.cursor = input.text.len();
        }
        terminal.draw(|f| draw(f, &mut a)).unwrap();
        assert!(a.hits.iter().all(|h| !matches!(
            h.intent,
            Intent::Screen(_) | Intent::Save | Intent::FocusSelect(_, _)
        )));
        let accept = a
            .hits
            .iter()
            .find(|h| h.intent == Intent::ModalAccept)
            .unwrap()
            .clone();
        a.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: accept.area.x,
            row: accept.area.y,
            modifiers: KeyModifiers::NONE,
        });
        assert!(!a.workspace.dirty());
        assert!(!a.workspace.path.exists());
        assert!(matches!(&a.overlay,Some(Overlay::Input(input)) if input.error.is_some()));
    }
}

fn module_description(module: &str) -> String {
    let text = match module {
        "directory" => "Current directory and read-only state",
        "git_branch" => "Branch in the current repository",
        "git_status" => "Repository changes and ahead / behind state",
        "cmd_duration" => "Duration above the configured threshold",
        "status" => "Previous command's exit status",
        "jobs" => "Background jobs above the configured threshold",
        "os" => "Operating system and configurable symbols",
        "character" => "Prompt character based on exit status",
        "time" => "Current time in the configured format",
        "hostname" => "Hostname, conditional on SSH settings",
        "username" => "Username, conditional on user settings",
        _ if module.starts_with("custom.") => "Custom command output and detection rules",
        _ => "Detected environment or project information",
    };
    text.into()
}

fn preview_text(s: &str, c: Chrome) -> Text<'static> {
    let mut text = preview::render_ansi(s);
    for line in &mut text.lines {
        for span in &mut line.spans {
            if span.style.bg.is_none() || span.style.bg == Some(Color::Reset) {
                span.style.bg = c.base.bg;
            }
            if span.style.fg.is_none() || span.style.fg == Some(Color::Reset) {
                span.style.fg = c.base.fg;
            }
        }
    }
    text
}

fn color_swatch(value: &str) -> Option<Color> {
    Some(match value {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "purple" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::Gray,
        "bright-black" => Color::DarkGray,
        "bright-red" => Color::LightRed,
        "bright-green" => Color::LightGreen,
        "bright-yellow" => Color::LightYellow,
        "bright-blue" => Color::LightBlue,
        "bright-purple" => Color::LightMagenta,
        "bright-cyan" => Color::LightCyan,
        "bright-white" => Color::White,
        v if v.starts_with('#') && v.len() == 7 => Color::Rgb(
            u8::from_str_radix(&v[1..3], 16).ok()?,
            u8::from_str_radix(&v[3..5], 16).ok()?,
            u8::from_str_radix(&v[5..7], 16).ok()?,
        ),
        _ => return None,
    })
}
