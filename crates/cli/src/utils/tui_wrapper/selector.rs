use crossterm::cursor::Show;
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size};
use std::io::Write;

use super::{ExtendedSelectResult, Key, SelectResult, read_key};

pub fn run_list_selector<T>(
    title: &str,
    items: &[T],
    render_item: fn(&T) -> String,
) -> SelectResult {
    if items.is_empty() {
        return SelectResult::Cancelled;
    }

    if enable_raw_mode().is_err() {
        return run_fallback_selector(title, items, render_item);
    }

    let mut stdout = std::io::stdout();
    let _ = crossterm::queue!(stdout, crossterm::cursor::Hide);
    let _ = stdout.flush();

    let mut selected: usize = 0;
    let result = run_selector_loop(&mut stdout, title, items, render_item, &mut selected);

    let _ = crossterm::queue!(stdout, Show);
    let _ = stdout.flush();
    let _ = disable_raw_mode();

    result
}

fn run_selector_loop<T>(
    stdout: &mut std::io::Stdout,
    title: &str,
    items: &[T],
    render_item: fn(&T) -> String,
    selected: &mut usize,
) -> SelectResult {
    let mut search_query = String::new();
    let mut needs_render = true;
    let mut term_height = 24usize;

    loop {
        if needs_render {
            let (width, height) = size().unwrap_or((80, 24));
            term_height = height as usize;
            let _ = render_selector(
                stdout,
                title,
                items,
                render_item,
                *selected,
                &search_query,
                width,
                height,
            );
            needs_render = false;
        }

        match read_key() {
            Some(Key::Up) => {
                *selected = if *selected > 0 {
                    *selected - 1
                } else {
                    items.len() - 1
                };
                needs_render = true;
            }
            Some(Key::Down) => {
                *selected = if *selected + 1 < items.len() {
                    *selected + 1
                } else {
                    0
                };
                needs_render = true;
            }
            Some(Key::PageUp) => {
                let page = term_height.saturating_sub(5).max(3);
                *selected = selected.saturating_sub(page);
                needs_render = true;
            }
            Some(Key::PageDown) => {
                let page = term_height.saturating_sub(5).max(3);
                *selected = std::cmp::min(*selected + page, items.len() - 1);
                needs_render = true;
            }
            Some(Key::Home) => {
                *selected = 0;
                needs_render = true;
            }
            Some(Key::End) => {
                *selected = items.len() - 1;
                needs_render = true;
            }
            Some(Key::Enter) => {
                return SelectResult::Selected(*selected);
            }
            Some(Key::Esc) | Some(Key::CtrlC) => {
                return SelectResult::Cancelled;
            }
            Some(Key::Backspace) => {
                search_query.pop();
                needs_render = true;
            }
            Some(Key::Tab) => {}
            Some(Key::Left) | Some(Key::Right) => {}
            Some(Key::CtrlD) | Some(Key::CtrlE) | Some(Key::Delete) => {}
            Some(Key::Char(c)) => {
                search_query.push(c);
                needs_render = true;
                if !search_query.is_empty() {
                    let lower = search_query.to_lowercase();
                    if let Some(idx) = items
                        .iter()
                        .position(|item| render_item(item).to_lowercase().contains(&lower))
                    {
                        *selected = idx;
                    }
                }
            }
            None => {}
        }
    }
}

fn render_selector<T>(
    stdout: &mut std::io::Stdout,
    title: &str,
    items: &[T],
    render_item: fn(&T) -> String,
    selected: usize,
    search_query: &str,
    width: u16,
    height: u16,
) -> Result<(), std::io::Error> {
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!("\x1b[1m{}\x1b[0m", title));
    lines.push(String::new());

    if search_query.is_empty() {
        lines.push("\x1b[2m  Type to search...\x1b[0m".to_string());
    } else {
        lines.push(format!("  {}", search_query));
    }
    lines.push(String::new());

    let available_height = (height as usize).saturating_sub(lines.len() + 2);
    let total = items.len();
    let start = if total > available_height {
        let half = available_height / 2;
        if selected > half {
            std::cmp::min(selected - half, total - available_height)
        } else {
            0
        }
    } else {
        0
    };
    let end = std::cmp::min(start + available_height, total);

    for i in start..end {
        let item_text = render_item(&items[i]);
        let is_selected = i == selected;
        let cursor = if is_selected {
            "\x1b[32m›\x1b[0m "
        } else {
            "  "
        };
        let styled = if is_selected {
            format!("\x1b[48;5;236m{}{}\x1b[0m", cursor, item_text)
        } else {
            format!("{}{}", cursor, item_text)
        };
        let styled = if styled.chars().count() > width as usize {
            format!(
                "{}…\x1b[0m",
                &styled
                    .chars()
                    .take(width.saturating_sub(4) as usize)
                    .collect::<String>()
            )
        } else {
            styled
        };
        lines.push(styled);
    }

    let hint = "\x1b[2m\x1b[3m↑↓ navigate · Enter select · Esc cancel\x1b[0m".to_string();
    lines.push(String::new());
    lines.push(hint);

    let output = format!("{}{}", crossterm::cursor::MoveTo(0, 0), lines.join("\r\n"));

    queue!(stdout, Clear(ClearType::All))?;
    queue!(stdout, crossterm::style::Print(output))?;
    stdout.flush()?;

    Ok(())
}

fn run_fallback_selector<T>(
    title: &str,
    items: &[T],
    render_item: fn(&T) -> String,
) -> SelectResult {
    println!("{}", title);
    for (i, item) in items.iter().enumerate() {
        println!("  {}. {}", i + 1, render_item(item));
    }
    println!("Enter number (1-{}), or 0 to cancel:", items.len());

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok()
        && let Ok(n) = input.trim().parse::<usize>()
        && n >= 1
        && n <= items.len()
    {
        return SelectResult::Selected(n - 1);
    }
    SelectResult::Cancelled
}

fn visible_width(s: &str) -> usize {
    let mut count = 0;
    let mut esc = false;
    for c in s.chars() {
        if esc {
            if c == 'm' {
                esc = false;
            }
        } else if c == '\x1b' {
            esc = true;
        } else {
            count += 1;
        }
    }
    count
}

fn visible_range<T>(
    items: &[T],
    render_item: fn(usize, &T) -> Vec<String>,
    scroll_offset: usize,
    available_height: usize,
    search_query: &str,
) -> (usize, Vec<usize>) {
    let lower = search_query.to_lowercase();
    let filtered: Vec<usize> = if lower.is_empty() {
        (0..items.len()).collect()
    } else {
        (0..items.len())
            .filter(|&i| {
                render_item(i, &items[i])
                    .iter()
                    .any(|l| l.to_lowercase().contains(&lower))
            })
            .collect()
    };

    let total = filtered.len();
    if total == 0 {
        return (0, filtered);
    }

    let start = std::cmp::min(scroll_offset, total.saturating_sub(1));
    let mut h = 0usize;
    let mut end = start;
    for i in start..total {
        let rendered = render_item(filtered[i], &items[filtered[i]]);
        let item_h: usize = rendered.len();
        if h + item_h > available_height {
            break;
        }
        h += item_h;
        end = i + 1;
    }
    (end - start, filtered)
}

fn render_selector_frame<T>(
    stdout: &mut std::io::Stdout,
    title: &str,
    items: &[T],
    render_item: fn(usize, &T) -> Vec<String>,
    selected: usize,
    search_query: &str,
    scope_name: &str,
    scroll_offset: &mut usize,
    confirm_exit: bool,
    visible_count: &mut usize,
    filtered_indices: &mut Vec<usize>,
) {
    let (width, height) = size().unwrap_or((80, 24));
    let max_list_height = (height as usize).max(9) / 3;
    let available_height = max_list_height.saturating_sub(6).max(3);

    let (vcount, filt) = visible_range(
        items,
        render_item,
        *scroll_offset,
        available_height,
        search_query,
    );
    *visible_count = vcount.max(1);
    *filtered_indices = filt;

    let total_filtered = filtered_indices.len();
    if *visible_count >= total_filtered {
        *scroll_offset = 0;
    } else {
        let last_start = total_filtered.saturating_sub(*visible_count);
        if *scroll_offset > last_start {
            *scroll_offset = last_start;
        }
    }

    render_extended_selector(
        stdout,
        title,
        items,
        render_item,
        selected,
        search_query,
        scope_name,
        width,
        *scroll_offset,
        available_height,
        confirm_exit,
    );
}

fn handle_selector_key_event<T>(
    key: Option<Key>,
    items: &[T],
    render_item: fn(usize, &T) -> Vec<String>,
    selected: &mut usize,
    scroll_offset: &mut usize,
    search_query: &mut String,
    visible_count: &mut usize,
    filtered_indices: &mut Vec<usize>,
    confirm_exit: &mut bool,
    needs_render: &mut bool,
) -> Option<ExtendedSelectResult> {
    if !matches!(key, Some(Key::CtrlC)) {
        *confirm_exit = false;
    }

    match key {
        Some(Key::CtrlC) => {
            if *confirm_exit {
                return Some(ExtendedSelectResult::Cancelled);
            }
            *confirm_exit = true;
            *needs_render = true;
        }
        Some(Key::Up) => {
            if *selected > 0 {
                *selected -= 1;
            } else {
                *selected = items.len() - 1;
                let (_, height) = size().unwrap_or((80, 24));
                let max_list_height = (height as usize).max(9) / 3;
                let available_height = max_list_height.saturating_sub(6).max(3);
                let (vcount, filt) =
                    visible_range(items, render_item, 0, available_height, search_query);
                *filtered_indices = filt;
                *visible_count = vcount.max(1);
                let total = filtered_indices.len();
                *scroll_offset = if *visible_count >= total {
                    0
                } else {
                    total.saturating_sub(*visible_count)
                };
            }
            if *selected < *scroll_offset {
                *scroll_offset = scroll_offset.saturating_sub(1);
            }
            *needs_render = true;
        }
        Some(Key::Down) => {
            if *selected + 1 < items.len() {
                *selected += 1;
            } else {
                *selected = 0;
                *scroll_offset = 0;
                *needs_render = true;
                return None;
            }
            let (_, height) = size().unwrap_or((80, 24));
            let max_list_height = (height as usize).max(9) / 3;
            let available_height = max_list_height.saturating_sub(6).max(3);
            let (vcount, _) = visible_range(
                items,
                render_item,
                *scroll_offset,
                available_height,
                search_query,
            );
            *visible_count = vcount.max(1);
            if *selected >= *scroll_offset + *visible_count {
                loop {
                    *scroll_offset += 1;
                    if *scroll_offset >= *selected {
                        break;
                    }
                    let (v2, _) = visible_range(
                        items,
                        render_item,
                        *scroll_offset,
                        available_height,
                        search_query,
                    );
                    *visible_count = v2.max(1);
                    if *selected < *scroll_offset + *visible_count {
                        break;
                    }
                }
            }
            *needs_render = true;
        }
        Some(Key::PageUp) => {
            let (_, height) = size().unwrap_or((80, 24));
            let max_list_height = (height as usize).max(9) / 3;
            let available_height = max_list_height.saturating_sub(6).max(3);
            let (vcount, _) = visible_range(
                items,
                render_item,
                *scroll_offset,
                available_height,
                search_query,
            );
            *visible_count = vcount.max(1);
            if *selected >= *visible_count {
                *selected = selected.saturating_sub(*visible_count);
            } else {
                *selected = 0;
            }
            if *selected < *scroll_offset {
                *scroll_offset = scroll_offset.saturating_sub(*visible_count);
            }
            *needs_render = true;
        }
        Some(Key::PageDown) => {
            let (_, height) = size().unwrap_or((80, 24));
            let max_list_height = (height as usize).max(9) / 3;
            let available_height = max_list_height.saturating_sub(6).max(3);
            let (vcount, _) = visible_range(
                items,
                render_item,
                *scroll_offset,
                available_height,
                search_query,
            );
            *visible_count = vcount.max(1);
            *selected = std::cmp::min(*selected + *visible_count, items.len().saturating_sub(1));
            if *selected >= *scroll_offset + *visible_count {
                loop {
                    *scroll_offset += 1;
                    if *scroll_offset >= *selected {
                        break;
                    }
                    let (v2, _) = visible_range(
                        items,
                        render_item,
                        *scroll_offset,
                        available_height,
                        search_query,
                    );
                    *visible_count = v2.max(1);
                    if *selected < *scroll_offset + *visible_count {
                        break;
                    }
                }
            }
            *needs_render = true;
        }
        Some(Key::Home) => {
            *selected = 0;
            *scroll_offset = 0;
            *needs_render = true;
        }
        Some(Key::End) => {
            *selected = items.len() - 1;
            let (_, height) = size().unwrap_or((80, 24));
            let max_list_height = (height as usize).max(9) / 3;
            let available_height = max_list_height.saturating_sub(6).max(3);
            let (vcount, filt) =
                visible_range(items, render_item, 0, available_height, search_query);
            *filtered_indices = filt;
            *visible_count = vcount.max(1);
            let total = filtered_indices.len();
            *scroll_offset = if *visible_count >= total {
                0
            } else {
                total.saturating_sub(*visible_count)
            };
            *needs_render = true;
        }
        Some(Key::Enter) => {
            return Some(ExtendedSelectResult::Selected(*selected));
        }
        Some(Key::Esc) => {
            return Some(ExtendedSelectResult::Cancelled);
        }
        Some(Key::Backspace) => {
            search_query.pop();
            *needs_render = true;
        }
        Some(Key::Tab) => {
            return Some(ExtendedSelectResult::ToggleScope);
        }
        Some(Key::Delete) => {
            return Some(ExtendedSelectResult::Delete(*selected));
        }
        Some(Key::CtrlD) => {
            return Some(ExtendedSelectResult::Delete(*selected));
        }
        Some(Key::CtrlE) => {
            return Some(ExtendedSelectResult::Preview(*selected));
        }
        Some(Key::Left) | Some(Key::Right) => {}
        Some(Key::Char(c)) => {
            search_query.push(c);
            *needs_render = true;
            if !search_query.is_empty() {
                let lower = search_query.to_lowercase();
                if let Some(idx) = (0..items.len()).find(|&i| {
                    let rendered = render_item(i, &items[i]);
                    rendered.iter().any(|l| l.to_lowercase().contains(&lower))
                }) {
                    *selected = idx;
                    let (_, height) = size().unwrap_or((80, 24));
                    let max_list_height = (height as usize).max(9) / 3;
                    let available_height = max_list_height.saturating_sub(6).max(3);
                    let (vcount, _) = visible_range(
                        items,
                        render_item,
                        *scroll_offset,
                        available_height,
                        search_query,
                    );
                    *visible_count = vcount.max(1);
                    if *selected < *scroll_offset || *selected >= *scroll_offset + *visible_count {
                        *scroll_offset = (*selected).saturating_sub(*visible_count / 2);
                    }
                }
            }
        }
        None => {}
    }
    None
}

#[allow(unused_assignments)]
pub fn run_extended_selector<T>(
    title: &str,
    items: &[T],
    render_item: fn(usize, &T) -> Vec<String>,
    scope_name: &str,
) -> ExtendedSelectResult {
    if items.is_empty() {
        return ExtendedSelectResult::Cancelled;
    }

    if enable_raw_mode().is_err() {
        return ExtendedSelectResult::Cancelled;
    }

    let mut stdout = std::io::stdout();
    let _ = queue!(stdout, crossterm::cursor::Hide);
    let _ = stdout.flush();

    let mut selected: usize = 0;
    let mut scroll_offset: usize = 0;
    let mut search_query = String::new();
    let mut needs_render = true;
    let mut visible_count = 1usize;
    let mut filtered_indices: Vec<usize> = Vec::new();
    let mut confirm_exit = false;
    let result = loop {
        if needs_render {
            render_selector_frame(
                &mut stdout,
                title,
                items,
                render_item,
                selected,
                &search_query,
                scope_name,
                &mut scroll_offset,
                confirm_exit,
                &mut visible_count,
                &mut filtered_indices,
            );
            needs_render = false;
        }

        let key = read_key();

        if let Some(result) = handle_selector_key_event(
            key,
            items,
            render_item,
            &mut selected,
            &mut scroll_offset,
            &mut search_query,
            &mut visible_count,
            &mut filtered_indices,
            &mut confirm_exit,
            &mut needs_render,
        ) {
            break result;
        }
    };

    let _ = crossterm::queue!(stdout, Show);
    let _ = stdout.flush();
    let _ = disable_raw_mode();
    result
}

fn render_extended_selector<T>(
    stdout: &mut std::io::Stdout,
    title: &str,
    items: &[T],
    render_item: fn(usize, &T) -> Vec<String>,
    selected: usize,
    search_query: &str,
    scope_name: &str,
    width: u16,
    scroll_offset: usize,
    available_height: usize,
    confirm_exit: bool,
) {
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!(
        "\x1b[1m{}\x1b[0m  \x1b[2m[{}]\x1b[0m",
        title, scope_name
    ));
    lines.push(String::new());

    if search_query.is_empty() {
        lines.push("\x1b[2m  Type to search...\x1b[0m".to_string());
    } else {
        lines.push(format!("  {}", search_query));
    }
    lines.push(String::new());

    let lower_query = search_query.to_lowercase();
    let filtered_indices: Vec<usize> = if lower_query.is_empty() {
        (0..items.len()).collect()
    } else {
        (0..items.len())
            .filter(|&i| {
                render_item(i, &items[i])
                    .iter()
                    .any(|l| l.to_lowercase().contains(&lower_query))
            })
            .collect()
    };

    if filtered_indices.is_empty() {
        lines.push(format!(
            "\x1b[2m  No sessions match \"{}\"\x1b[0m",
            search_query
        ));
    } else {
        let total_filtered = filtered_indices.len();
        let safe_offset = std::cmp::min(scroll_offset, total_filtered.saturating_sub(1));

        let mut used_h = 0usize;
        let mut end = safe_offset;
        for i in safe_offset..total_filtered {
            let rendered = render_item(filtered_indices[i], &items[filtered_indices[i]]);
            let item_h: usize = rendered.len();
            if used_h + item_h > available_height {
                break;
            }
            used_h += item_h;
            end = i + 1;
        }

        for page_idx in safe_offset..end {
            let actual_idx = filtered_indices[page_idx];
            let rendered_lines = render_item(actual_idx, &items[actual_idx]);
            let is_selected = actual_idx == selected;
            let first_content_idx = rendered_lines
                .iter()
                .position(|l| !l.trim().is_empty())
                .unwrap_or(0);
            for (li, item_line) in rendered_lines.iter().enumerate() {
                if item_line.trim().is_empty() {
                    lines.push(item_line.clone());
                    continue;
                }
                if is_selected {
                    let is_first_line = li == first_content_idx;
                    if is_first_line {
                        let text = item_line.trim_start();
                        let display = if visible_width(text) + 2 > width as usize {
                            let max_txt = (width as usize).saturating_sub(4);
                            format!("{}\u{2026}", text.chars().take(max_txt).collect::<String>())
                        } else {
                            text.to_string()
                        };
                        lines.push(format!("\x1b[1;38;5;208m\u{203a}\x1b[0m {}", display));
                    } else {
                        lines.push(format!("  {}", item_line.trim_start()));
                    }
                } else {
                    let text = format!("  {}", item_line);
                    let truncated = if visible_width(&text) > width as usize {
                        format!(
                            "{}\u{2026}",
                            &text
                                .chars()
                                .take(width.saturating_sub(4) as usize)
                                .collect::<String>()
                        )
                    } else {
                        text
                    };
                    lines.push(truncated);
                }
            }
        }

        lines.push(String::new());
        lines.push(format!(
            "\x1b[2mItems {}-{} of {}\x1b[0m",
            safe_offset + 1,
            end,
            total_filtered,
        ));
    }

    if confirm_exit {
        lines.push("\x1b[33m  Press Ctrl+C again to exit\x1b[0m".to_string());
    }

    lines.push(String::new());
    let hint = "\x1b[2m\u{2191}\u{2193} navigate \u{b7} Enter select \u{b7} Esc cancel \u{b7} \
         Tab scope \u{b7} Ctrl+E preview \u{b7} Delete remove\x1b[0m"
        .to_string();
    lines.push(hint);

    let output = format!("{}{}", crossterm::cursor::MoveTo(0, 0), lines.join("\r\n"));

    let _ = queue!(stdout, Clear(ClearType::All));
    let _ = queue!(stdout, crossterm::style::Print(output));
    let _ = stdout.flush();
}
