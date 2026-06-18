use std::time::Instant;

use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use pick_tui::app::{TuiAction, TuiApp};

/// Process a single keyboard event in the input loop context.
/// Accumulates Char + Enter for paste batching, falls through to
/// TuiApp::handle_key for control keys.
pub(crate) fn process_key_event(
    tui: &mut TuiApp,
    key: crossterm::event::KeyEvent,
    now: Instant,
) -> Option<TuiAction> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

    // Ctrl+Shift+V: direct clipboard read
    if key.code == KeyCode::Char('v')
        && key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
    {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                tui.handle_paste(&text);
            }
        }
        return None;
    }

    // Route Char + Enter to paste accumulator
    let mut paste_handled = false;
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        match key.code {
            KeyCode::Char(c) => {
                tui.paste_accumulator.push(c);
                tui.last_paste_time = Some(now);
                paste_handled = true;
            }
            KeyCode::Enter if !tui.paste_accumulator.is_empty() => {
                tui.paste_accumulator.push('\n');
                tui.last_paste_time = Some(now);
                paste_handled = true;
            }
            KeyCode::Enter => {}
            _ => {}
        }
    }

    if !paste_handled {
        tui.finalize_paste_accumulator(now);
        return tui.handle_key(key.code, key.modifiers);
    }

    None
}

/// Process a single keyboard event during agent execution.
/// Same as process_key_event but Esc aborts the agent.
pub(crate) fn process_key_event_during_agent(
    tui: &mut TuiApp,
    key: crossterm::event::KeyEvent,
    now: Instant,
) -> Option<TuiAction> {
    if key.kind != KeyEventKind::Press {
        return None;
    }

    // Ctrl+Shift+V: direct clipboard read
    if key.code == KeyCode::Char('v')
        && key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
    {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Ok(text) = clipboard.get_text() {
                tui.handle_paste(&text);
            }
        }
        return None;
    }

    // Esc always aborts agent
    if key.code == KeyCode::Esc {
        tui.finalize_paste_accumulator(now);
        return Some(TuiAction::Quit);
    }

    // Route Char + Enter to paste accumulator
    let mut paste_handled = false;
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        match key.code {
            KeyCode::Char(c) => {
                tui.paste_accumulator.push(c);
                tui.last_paste_time = Some(now);
                paste_handled = true;
            }
            KeyCode::Enter if !tui.paste_accumulator.is_empty() => {
                tui.finalize_paste_accumulator(now);
            }
            KeyCode::Enter => {}
            _ => {}
        }
    }

    if !paste_handled {
        tui.finalize_paste_accumulator(now);
        return tui.handle_key(key.code, key.modifiers);
    }

    None
}

/// Drain the keyboard event channel in a tight loop (paste accumulation).
/// Returns Some(TuiAction) if a key triggers an action.
pub(crate) fn drain_key_events(
    tui: &mut TuiApp,
    evt_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crossterm::event::Event>,
    now: Instant,
) -> Option<TuiAction> {
    loop {
        let mut had_action = false;
        let mut action: Option<TuiAction> = None;

        loop {
            match evt_rx.try_recv() {
                Ok(crossterm::event::Event::Key(key))
                    if key.kind == KeyEventKind::Press
                        && !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    match key.code {
                        KeyCode::Char(c) => {
                            tui.paste_accumulator.push(c);
                            continue;
                        }
                        KeyCode::Enter => {
                            tui.paste_accumulator.push('\n');
                            continue;
                        }
                        _ => {
                            tui.finalize_paste_accumulator(now);
                            if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                                action = Some(a);
                                had_action = true;
                            }
                            continue;
                        }
                    }
                }
                Ok(crossterm::event::Event::Key(key))
                    if key.kind == KeyEventKind::Press
                        && key.modifiers.intersects(KeyModifiers::CONTROL) =>
                {
                    tui.finalize_paste_accumulator(now);
                    if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                        action = Some(a);
                        had_action = true;
                    }
                    continue;
                }
                Ok(crossterm::event::Event::Key(_)) => {
                    continue;
                }
                Ok(crossterm::event::Event::Paste(text)) => {
                    tui.finalize_paste_accumulator(now);
                    tui.handle_paste(&text);
                    continue;
                }
                Ok(crossterm::event::Event::Resize(_, _)) => {
                    continue;
                }
                Ok(_) => {
                    continue;
                }
                Err(_) => break,
            }
        }

        tui.finalize_paste_accumulator(now);

        if had_action {
            return action;
        }
        break;
    }
    None
}

/// Drain keyboard events during agent execution.
/// Same as drain_key_events but Esc aborts the agent.
pub(crate) fn drain_key_events_during_agent(
    tui: &mut TuiApp,
    evt_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crossterm::event::Event>,
    now: Instant,
    abort_on_esc: &mut bool,
) -> Option<TuiAction> {
    loop {
        let mut had_action = false;
        let mut action: Option<TuiAction> = None;

        loop {
            match evt_rx.try_recv() {
                Ok(crossterm::event::Event::Key(key))
                    if key.kind == KeyEventKind::Press
                        && !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    match key.code {
                        KeyCode::Char(c) => {
                            tui.paste_accumulator.push(c);
                            continue;
                        }
                        KeyCode::Enter => {
                            tui.finalize_paste_accumulator(now);
                            if let Some(a) = tui.handle_key(KeyCode::Enter, KeyModifiers::NONE) {
                                if matches!(a, TuiAction::Quit) {
                                    action = Some(a);
                                    had_action = true;
                                }
                            }
                            continue;
                        }
                        _ => {
                            tui.finalize_paste_accumulator(now);
                            if key.code == KeyCode::Esc {
                                *abort_on_esc = true;
                                break;
                            }
                            if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                                if matches!(a, TuiAction::Quit) {
                                    action = Some(a);
                                    had_action = true;
                                }
                            }
                            continue;
                        }
                    }
                }
                Ok(crossterm::event::Event::Key(key))
                    if key.kind == KeyEventKind::Press
                        && key.modifiers.intersects(KeyModifiers::CONTROL) =>
                {
                    tui.finalize_paste_accumulator(now);
                    if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                        if matches!(a, TuiAction::Quit) {
                            action = Some(a);
                            had_action = true;
                        }
                    }
                    continue;
                }
                Ok(crossterm::event::Event::Key(_)) => {
                    continue;
                }
                Ok(crossterm::event::Event::Paste(text)) => {
                    tui.finalize_paste_accumulator(now);
                    tui.handle_paste(&text);
                    continue;
                }
                Ok(crossterm::event::Event::Resize(_, _)) => {
                    continue;
                }
                Ok(_) => {
                    continue;
                }
                Err(_) => break,
            }
        }

        tui.finalize_paste_accumulator(now);

        if had_action {
            return action;
        }
        break;
    }
    None
}
