use std::time::Instant;

use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use pick_tui::app::{AppState, TuiAction, TuiApp};

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
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && let Ok(text) = clipboard.get_text()
        {
            tui.handle_paste(&text);
        }
        return None;
    }

    // In Selecting or ApiKeyInput state: route char keys directly to
    // search / API key input, skip paste accumulation.
    if tui.state == AppState::Selecting || tui.state == AppState::ApiKeyInput {
        tui.finalize_paste_accumulator(now);
        // In ApiKeyInput state, Ctrl+V reads clipboard and pastes
        if tui.state == AppState::ApiKeyInput
            && key.code == KeyCode::Char('v')
            && key.modifiers == KeyModifiers::CONTROL
        {
            if let Ok(mut clipboard) = arboard::Clipboard::new()
                && let Ok(text) = clipboard.get_text()
            {
                tui.handle_paste(&text);
            }
            return None;
        }
        return tui.handle_key(key.code, key.modifiers);
    }

    // Route Char + Enter to paste accumulator, but NOT ASCII control
    // characters (U+0000–U+001F). On Windows, crossterm can report Ctrl+C
    // as Char('\x03') / NONE and Ctrl+D as Char('\x04') / NONE instead of
    // the usual (Char('c'), CONTROL) / (Char('d'), CONTROL) form. Those
    // control characters must be routed to handle_key — not the paste
    // accumulator — otherwise Ctrl+C / Ctrl+D are silently swallowed and
    // the user can never quit.
    let mut paste_handled = false;
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        match key.code {
            KeyCode::Char(c) if c as u32 <= 0x1F => {
                // ASCII control character: route directly to handle_key
                tui.finalize_paste_accumulator(now);
                return tui.handle_key(key.code, key.modifiers);
            }
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
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && let Ok(text) = clipboard.get_text()
        {
            tui.handle_paste(&text);
        }
        return None;
    }

    // Esc always aborts agent
    if key.code == KeyCode::Esc {
        tui.finalize_paste_accumulator(now);
        return Some(TuiAction::Quit);
    }

    // Route Char + Enter to paste accumulator, but NOT ASCII control
    // characters (same as process_key_event above).
    let mut paste_handled = false;
    if !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        match key.code {
            KeyCode::Char(c) if c as u32 <= 0x1F => {
                tui.finalize_paste_accumulator(now);
                return tui.handle_key(key.code, key.modifiers);
            }
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
                    // During selecting or api-key input, route chars directly
                    if tui.state == AppState::Selecting || tui.state == AppState::ApiKeyInput {
                        tui.finalize_paste_accumulator(now);
                        if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                            action = Some(a);
                            had_action = true;
                        }
                        continue;
                    }
                    match key.code {
                        KeyCode::Char(c) if (c as u32) <= 0x1F => {
                            // ASCII control character (e.g. \x03 = Ctrl+C,
                            // \x04 = Ctrl+D arriving without CONTROL
                            // modifier on Windows). Route to handle_key.
                            tui.finalize_paste_accumulator(now);
                            if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                                action = Some(a);
                                had_action = true;
                            }
                            continue;
                        }
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
                Ok(crossterm::event::Event::Key(key))
                    if key.kind == KeyEventKind::Press
                        && key.modifiers.intersects(KeyModifiers::ALT) =>
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
                        KeyCode::Char(c) if (c as u32) <= 0x1F => {
                            tui.finalize_paste_accumulator(now);
                            if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                                if matches!(a, TuiAction::Quit) {
                                    action = Some(a);
                                    had_action = true;
                                } else if matches!(a, TuiAction::QueueMessage(_)) {
                                    action = Some(a);
                                    had_action = true;
                                }
                            }
                            continue;
                        }
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
                                } else if matches!(a, TuiAction::QueueMessage(_)) {
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
                                } else if matches!(a, TuiAction::QueueMessage(_)) {
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
                        } else if matches!(
                            a,
                            TuiAction::QueueMessage(_) | TuiAction::QueueFollowUp(_)
                        ) {
                            action = Some(a);
                            had_action = true;
                        }
                    }
                    continue;
                }
                Ok(crossterm::event::Event::Key(key))
                    if key.kind == KeyEventKind::Press
                        && key.modifiers.intersects(KeyModifiers::ALT) =>
                {
                    tui.finalize_paste_accumulator(now);
                    if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                        if matches!(
                            a,
                            TuiAction::Quit
                                | TuiAction::QueueMessage(_)
                                | TuiAction::QueueFollowUp(_)
                        ) {
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
