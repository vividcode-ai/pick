use std::time::Instant;

use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use pick_tui::app::{AppState, TuiAction, TuiApp};

// ---- Windows helper: resolve modifier state from OS-level key polling ----
// Some Windows terminals report Ctrl+Enter (and Shift+Enter) as plain
// Enter+NONE without setting the CONTROL or SHIFT modifier flags.
// These helpers query the actual physical key state via user32.
#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}
#[cfg(windows)]
const VK_CONTROL: i32 = 0x11;
#[cfg(windows)]
const VK_LCONTROL: i32 = 0xA2;
#[cfg(windows)]
const VK_RCONTROL: i32 = 0xA3;
#[cfg(windows)]
const VK_SHIFT: i32 = 0x10;
#[cfg(windows)]
const VK_LSHIFT: i32 = 0xA0;
#[cfg(windows)]
const VK_RSHIFT: i32 = 0xA1;
#[cfg(windows)]
fn windows_is_key_down(vk: i32) -> bool {
    let state = unsafe { GetAsyncKeyState(vk) };
    state < 0
}

#[cfg(windows)]
fn windows_ctrl_active() -> bool {
    windows_is_key_down(VK_CONTROL)
        || windows_is_key_down(VK_LCONTROL)
        || windows_is_key_down(VK_RCONTROL)
}

#[cfg(windows)]
fn windows_shift_active() -> bool {
    windows_is_key_down(VK_SHIFT)
        || windows_is_key_down(VK_LSHIFT)
        || windows_is_key_down(VK_RSHIFT)
}

/// Maximum age (ms) for `last_detected_modifiers` to be used as a
/// fallback when `resolve_modifiers` returns NONE for an Enter event.
const MODIFIER_FALLBACK_MS: u128 = 100;

/// Maximum age (ms) for the `just_processed_newline` dedup flag.
const NEWLINE_DEDUP_TIMEOUT_MS: u128 = 50;

/// Check whether a recent newline was processed (dedup flag with timeout).
fn check_newline_dedup(tui: &mut TuiApp, now: Instant) -> bool {
    if tui.just_processed_newline {
        if let Some(t) = tui.last_newline_time {
            if now.duration_since(t).as_millis() <= NEWLINE_DEDUP_TIMEOUT_MS {
                tui.just_processed_newline = false;
                return true;
            }
        }
        tui.just_processed_newline = false;
    }
    false
}

/// Resolve modifiers for Enter with fallback to tracked state.
fn resolve_enter_modifiers(
    tui: &TuiApp,
    event: &crossterm::event::KeyEvent,
    now: Instant,
) -> KeyModifiers {
    let os_mods = resolve_modifiers(event);
    if os_mods.intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL) {
        return os_mods;
    }
    if let Some(last_time) = tui.last_key_event_time {
        if now.duration_since(last_time).as_millis() <= MODIFIER_FALLBACK_MS
            && tui
                .last_detected_modifiers
                .intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL)
        {
            return tui.last_detected_modifiers;
        }
    }
    os_mods
}

/// Track modifier state from a key event for Enter resolution fallback.
pub(crate) fn track_modifiers(tui: &mut TuiApp, key: &crossterm::event::KeyEvent, now: Instant) {
    if key.code == KeyCode::Enter {
        let probe = resolve_modifiers(key);
        tui.last_detected_modifiers = probe;
        if probe != KeyModifiers::NONE {
            tui.last_key_event_time = Some(now);
        }
    }
    if matches!(key.code, KeyCode::Char('\n') | KeyCode::Char('\r')) {
        let mods = resolve_modifiers(key);
        tui.last_detected_modifiers = mods;
        if mods != KeyModifiers::NONE {
            tui.last_key_event_time = Some(now);
        }
    }
}

/// Resolve the actual modifier state for a key event.
/// On Windows, crossterm's modifier reporting can be unreliable for
/// Enter (the terminal might not set dwControlKeyState for VK_RETURN).
/// We supplement it by polling the OS-level key state.
#[cfg(windows)]
fn resolve_modifiers(event: &crossterm::event::KeyEvent) -> KeyModifiers {
    let mut m = event.modifiers;
    if windows_ctrl_active() {
        m |= KeyModifiers::CONTROL;
    }
    if windows_shift_active() {
        m |= KeyModifiers::SHIFT;
    }
    m
}
#[cfg(not(windows))]
fn resolve_modifiers(event: &crossterm::event::KeyEvent) -> KeyModifiers {
    event.modifiers
}

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
            tui.force_flush_paste_accumulator();
            tui.handle_paste(&text);
        }
        return None;
    }

    // In Selecting or ApiKeyInput state: route char keys directly to
    // search / API key input, skip paste accumulation.
    if tui.state == AppState::Selecting || tui.state == AppState::ApiKeyInput {
        tui.force_flush_paste_accumulator();
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
        track_modifiers(tui, &key, now);
        return tui.handle_key(key.code, key.modifiers);
    }

    // Track modifier state for Enter-key modifier resolution fallback.
    track_modifiers(tui, &key, now);

    // ---- Enter key: special handling ----
    // On Windows some terminals report Ctrl+Enter / Shift+Enter
    // as plain Enter+NONE without the CONTROL/SHIFT modifiers.
    // We query the OS-level key state to catch these cases, with
    // an additional fallback to recently-tracked modifiers.
    if key.code == KeyCode::Enter {
        let resolved = resolve_enter_modifiers(tui, &key, now);

        // Shift+Enter: insert newline.
        // Ctrl+Enter: no-op (neither submit nor insert newline).
        if resolved.contains(KeyModifiers::SHIFT) {
            tui.force_flush_paste_accumulator();
            tui.editor.insert_newline_auto_indent();
            tui.just_processed_newline = true;
            tui.last_newline_time = Some(now);
            return None;
        }
        if resolved.contains(KeyModifiers::CONTROL) {
            tui.force_flush_paste_accumulator();
            return None;
        }

        // During a paste burst (chars arriving <50ms apart), accumulate
        // \n so the entire paste stays in the accumulator instead of
        // flushing partial content to the editor as raw text.  Windows
        // may report pasted newlines as KeyCode::Enter (not Char('\n')),
        // so this guard is essential.
        if !tui.paste_accumulator.is_empty() {
            if let Some(t) = tui.last_paste_time
                && now.duration_since(t).as_millis() < 50
            {
                tui.paste_accumulator.push('\n');
                tui.last_paste_time = Some(now);
                return None;
            }
            tui.force_flush_paste_accumulator();
        }

        // Dedup: if a Char('\n') was just processed (which also inserted
        // a newline), this Enter event is a stale duplicate from a
        // terminal that generates both event types for a single keystroke.
        if check_newline_dedup(tui, now) {
            return None;
        }

        // Bare Enter with empty accumulator: normal submit.
        tui.force_flush_paste_accumulator();
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
                if (c == '\n' || c == '\r' || c == '\t') && check_newline_dedup(tui, now) {
                    return None;
                }
                if c == '\n' || c == '\r' || c == '\t' {
                    // During a paste burst (accumulator non-empty),
                    // push the character into the accumulator to keep the paste
                    // content together instead of flushing partial content.
                    if !tui.paste_accumulator.is_empty() {
                        tui.paste_accumulator.push(c);
                        tui.last_paste_time = Some(now);
                        return None;
                    }
                    // No paste in progress — handle each differently:
                    if c == '\t' {
                        tui.force_flush_paste_accumulator();
                        return tui.handle_key(KeyCode::Tab, key.modifiers);
                    }
                    // c == '\n' || c == '\r': use the same modifier resolution
                    // as Enter.  This handles Shift+Enter (newline),
                    // Ctrl+Enter (no-op), and bare Enter (submit) when the
                    // terminal reports a modified Enter as a bare control char.
                    tui.force_flush_paste_accumulator();
                    let resolved = resolve_enter_modifiers(tui, &key, now);
                    if resolved.contains(KeyModifiers::SHIFT) {
                        tui.editor.insert_newline_auto_indent();
                        tui.just_processed_newline = true;
                        tui.last_newline_time = Some(now);
                        return None;
                    }
                    if resolved.contains(KeyModifiers::CONTROL) {
                        return None;
                    }
                    // Bare \n/r: flush accumulator and submit.
                    tui.force_flush_paste_accumulator();
                    return tui.handle_key(KeyCode::Enter, KeyModifiers::NONE);
                }
                tui.force_flush_paste_accumulator();
                let result = tui.handle_key(key.code, key.modifiers);
                return result;
            }
            KeyCode::Char(c) => {
                tui.handle_char_for_paste(c, now);
                paste_handled = true;
            }
            _ => {}
        }
    }

    if !paste_handled {
        tui.force_flush_paste_accumulator();
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
            tui.force_flush_paste_accumulator();
            tui.handle_paste(&text);
        }
        return None;
    }

    // Track modifier state for Enter-key modifier resolution fallback.
    track_modifiers(tui, &key, now);

    // ---- Enter: resolve modifiers with fallback ----
    if key.code == KeyCode::Enter {
        let resolved = resolve_enter_modifiers(tui, &key, now);

        // Shift+Enter: insert newline.
        // Ctrl+Enter: no-op.
        if resolved.contains(KeyModifiers::SHIFT) {
            tui.force_flush_paste_accumulator();
            tui.editor.insert_newline_auto_indent();
            tui.just_processed_newline = true;
            tui.last_newline_time = Some(now);
            return None;
        }
        if resolved.contains(KeyModifiers::CONTROL) {
            tui.force_flush_paste_accumulator();
            return None;
        }
        // During a paste burst (chars arriving <50ms apart), accumulate
        // \n so the entire paste stays in the accumulator instead of
        // flushing partial content to the editor as raw text.  Windows
        // may report pasted newlines as KeyCode::Enter (not Char('\n')),
        // so this guard is essential.
        if !tui.paste_accumulator.is_empty() {
            if let Some(t) = tui.last_paste_time
                && now.duration_since(t).as_millis() < 50
            {
                tui.paste_accumulator.push('\n');
                tui.last_paste_time = Some(now);
                return None;
            }
            tui.force_flush_paste_accumulator();
        }
        if check_newline_dedup(tui, now) {
            return None;
        }

        // Bare Enter with empty accumulator: normal submit.
        tui.force_flush_paste_accumulator();
        return tui.handle_key(key.code, key.modifiers);
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
                if (c == '\n' || c == '\r' || c == '\t') && check_newline_dedup(tui, now) {
                    return None;
                }
                if c == '\n' || c == '\r' || c == '\t' {
                    // During a paste burst (accumulator non-empty),
                    // push the character into the accumulator to keep the paste
                    // content together instead of flushing partial content.
                    if !tui.paste_accumulator.is_empty() {
                        tui.paste_accumulator.push(c);
                        tui.last_paste_time = Some(now);
                        return None;
                    }
                    // No paste in progress — handle each differently:
                    if c == '\t' {
                        tui.force_flush_paste_accumulator();
                        return tui.handle_key(KeyCode::Tab, key.modifiers);
                    }
                    // c == '\n' || c == '\r': use the same modifier resolution
                    // as Enter.  This handles Shift+Enter (newline),
                    // Ctrl+Enter (no-op), and bare Enter (submit) when the
                    // terminal reports a modified Enter as a bare control char.
                    tui.force_flush_paste_accumulator();
                    let resolved = resolve_enter_modifiers(tui, &key, now);
                    if resolved.contains(KeyModifiers::SHIFT) {
                        tui.editor.insert_newline_auto_indent();
                        tui.just_processed_newline = true;
                        tui.last_newline_time = Some(now);
                        return None;
                    }
                    if resolved.contains(KeyModifiers::CONTROL) {
                        return None;
                    }
                    // Bare \n/r: flush accumulator and submit.
                    tui.force_flush_paste_accumulator();
                    return tui.handle_key(KeyCode::Enter, KeyModifiers::NONE);
                }
                tui.force_flush_paste_accumulator();
                let result = tui.handle_key(key.code, key.modifiers);
                return result;
            }
            KeyCode::Char(c) => {
                tui.handle_char_for_paste(c, now);
                paste_handled = true;
            }
            _ => {}
        }
    }

    if !paste_handled {
        tui.force_flush_paste_accumulator();
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
        let mut had_paste_push = false;
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
                        tui.force_flush_paste_accumulator();
                        if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                            action = Some(a);
                            had_action = true;
                        }
                        continue;
                    }
                    match key.code {
                        KeyCode::Char(c) if (c as u32) <= 0x1F => {
                            // Handle \n/\r as newline insertion if
                            // Shift or Ctrl is held (modified Enter).
                            // Bare \n without modifiers is also inserted
                            // as newline (it's an unprintable control char
                            // that would corrupt the buffer if inserted
                            // as text — and in drain this is a leftover
                            // from a modified Enter, not a submit action).
                            if c == '\n' || c == '\r' || c == '\t' {
                                if (c == '\n' || c == '\r')
                                    && check_newline_dedup(tui, Instant::now())
                                {
                                    continue;
                                }
                                // During a paste burst (accumulator non-empty),
                                // push the character into the accumulator so the
                                // entire paste content stays together.
                                if !tui.paste_accumulator.is_empty() {
                                    tui.paste_accumulator.push(c);
                                    tui.last_paste_time = Some(Instant::now());
                                    had_paste_push = true;
                                } else if c == '\t' {
                                    tui.force_flush_paste_accumulator();
                                    if let Some(a) = tui.handle_key(KeyCode::Tab, key.modifiers) {
                                        action = Some(a);
                                        had_action = true;
                                    }
                                } else {
                                    // \n or \r without active paste — apply
                                    // modifier-based action.
                                    if tui
                                        .last_detected_modifiers
                                        .intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL)
                                    {
                                        tui.force_flush_paste_accumulator();
                                        tui.editor.insert_newline_auto_indent();
                                        tui.just_processed_newline = true;
                                        tui.last_newline_time = Some(Instant::now());
                                    }
                                }
                                continue;
                            }
                            tui.force_flush_paste_accumulator();
                            if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                                action = Some(a);
                                had_action = true;
                            }
                            continue;
                        }
                        KeyCode::Char(c) => {
                            tui.handle_char_for_paste(c, Instant::now());
                            had_paste_push = true;
                            continue;
                        }
                        KeyCode::Enter => {
                            // Extra Enter event in drain — the main
                            // select!/process_key_event path handles the
                            // primary event.  Accumulate \n when
                            // already in a paste batch; otherwise skip.
                            if !tui.paste_accumulator.is_empty() {
                                tui.paste_accumulator.push('\n');
                                tui.last_paste_time = Some(Instant::now());
                                had_paste_push = true;
                                continue;
                            }
                            continue;
                        }
                        _ => {
                            tui.force_flush_paste_accumulator();
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
                    // Dedup: Enter, \n, or \r with CONTROL can be a
                    // duplicate of a Ctrl+Enter already handled in
                    // process_key_event (dual-event Windows terminal).
                    if matches!(
                        key.code,
                        KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r')
                    ) && check_newline_dedup(tui, Instant::now())
                    {
                        continue;
                    }
                    tui.force_flush_paste_accumulator();
                    let action_result = tui.handle_key(key.code, key.modifiers);
                    if action_result.is_none()
                        && matches!(
                            key.code,
                            KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r')
                        )
                    {
                        tui.just_processed_newline = true;
                        tui.last_newline_time = Some(Instant::now());
                    }
                    if let Some(a) = action_result {
                        action = Some(a);
                        had_action = true;
                    }
                    continue;
                }
                Ok(crossterm::event::Event::Key(key))
                    if key.kind == KeyEventKind::Press
                        && key.modifiers.intersects(KeyModifiers::ALT) =>
                {
                    tui.force_flush_paste_accumulator();
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
                    tui.force_flush_paste_accumulator();
                    tui.handle_paste(&text);
                    tui.last_paste_time = Some(Instant::now());
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

        if !had_paste_push {
            tui.finalize_paste_accumulator(now);
        }

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
    // Track modifiers from the outer-scope event that triggered draining.
    // We can't get the key event here, but last_detected_modifiers is
    // already populated by process_key_event_during_agent's track_modifiers.
    loop {
        let mut had_action = false;
        let mut had_paste_push = false;
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
                            // Handle \n, \r, and \t appropriately during
                            // paste bursts to keep the paste content together.
                            if c == '\n' || c == '\r' || c == '\t' {
                                if (c == '\n' || c == '\r')
                                    && check_newline_dedup(tui, Instant::now())
                                {
                                    continue;
                                }
                                // During a paste burst (accumulator non-empty),
                                // push the character into the accumulator so the
                                // entire paste content stays together.
                                if !tui.paste_accumulator.is_empty() {
                                    tui.paste_accumulator.push(c);
                                    tui.last_paste_time = Some(Instant::now());
                                    had_paste_push = true;
                                } else if c == '\t' {
                                    tui.force_flush_paste_accumulator();
                                    if let Some(a) = tui.handle_key(KeyCode::Tab, key.modifiers) {
                                        if matches!(a, TuiAction::Quit) {
                                            action = Some(a);
                                            had_action = true;
                                        } else if matches!(a, TuiAction::QueueMessage(_)) {
                                            action = Some(a);
                                            had_action = true;
                                        }
                                    }
                                } else {
                                    // \n or \r without active paste — apply
                                    // modifier-based action.
                                    if tui
                                        .last_detected_modifiers
                                        .intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL)
                                    {
                                        tui.force_flush_paste_accumulator();
                                        tui.editor.insert_newline_auto_indent();
                                        tui.just_processed_newline = true;
                                        tui.last_newline_time = Some(Instant::now());
                                    }
                                }
                                continue;
                            }
                            tui.force_flush_paste_accumulator();
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
                            tui.handle_char_for_paste(c, Instant::now());
                            had_paste_push = true;
                            continue;
                        }
                        KeyCode::Enter => {
                            // Extra Enter event in drain. Accumulate \n when
                            // already in a paste batch; skip otherwise.
                            if !tui.paste_accumulator.is_empty() {
                                tui.paste_accumulator.push('\n');
                                tui.last_paste_time = Some(Instant::now());
                                had_paste_push = true;
                                continue;
                            }
                            continue;
                        }
                        _ => {
                            tui.force_flush_paste_accumulator();
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
                    // Dedup: Enter, \n, or \r with CONTROL can be a
                    // duplicate of a Ctrl+Enter already handled in
                    // process_key_event_during_agent.
                    if matches!(
                        key.code,
                        KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r')
                    ) && check_newline_dedup(tui, Instant::now())
                    {
                        continue;
                    }
                    tui.force_flush_paste_accumulator();
                    let action_result = tui.handle_key(key.code, key.modifiers);
                    if action_result.is_none()
                        && matches!(
                            key.code,
                            KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r')
                        )
                    {
                        tui.just_processed_newline = true;
                        tui.last_newline_time = Some(Instant::now());
                    }
                    if let Some(a) = action_result {
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
                    tui.force_flush_paste_accumulator();
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
                    tui.force_flush_paste_accumulator();
                    tui.handle_paste(&text);
                    tui.last_paste_time = Some(Instant::now());
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

        if !had_paste_push {
            tui.finalize_paste_accumulator(now);
        }

        if had_action {
            return action;
        }
        break;
    }
    None
}

// ---- Process-key-event integration tests ----
// These call `process_key_event` directly, the exact entry-point used by
// the event loop, with various modifier and paste-accumulator states.
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn enter(mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, mods)
    }
    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }
    fn char_nl(mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(KeyCode::Char('\n'), mods)
    }

    /// Simulate typing a character at NORMAL speed (chars >50ms apart trigger
    /// the paste-accumulator timer flush).  The real event loop calls
    /// finalize_paste_accumulator every 20ms — here we call it after each
    /// char with a late-enough timestamp to trigger the flush.
    fn type_char_slow(app: &mut TuiApp, c: char) {
        let now = Instant::now();
        process_key_event(app, key(c), now);
        // The 20ms timer in the event loop calls finalize_paste_accumulator,
        // which flushes if last_paste_time > 50ms ago.  Use a timestamp far
        // enough ahead to guarantee flush.
        app.finalize_paste_accumulator(now + std::time::Duration::from_millis(100));
    }

    /// Simulate typing a whole string at normal speed.
    fn type_str(app: &mut TuiApp, s: &str) {
        for c in s.chars() {
            type_char_slow(app, c);
        }
    }

    /// Simulate pressing a modifier key (Ctrl+Enter, plain Enter, etc).
    /// In the real event loop, the select! catches the first key event and
    /// dispatches it via process_key_event; remaining events are drained via
    /// drain_key_events which calls force_flush on exit.  Here we simulate
    /// just the process_key_event + final flush.
    fn press_key(app: &mut TuiApp, evt: KeyEvent) -> Option<TuiAction> {
        let now = Instant::now();
        let result = process_key_event(app, evt, now);
        app.finalize_paste_accumulator(now);
        result
    }

    // ===== Ctrl+Enter (no-op) =====

    #[test]
    fn test_pke_ctrl_enter_preserves_text_on_first_line() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "hello");
        assert_eq!(app.editor.buffer, "hello");

        let result = press_key(&mut app, enter(KeyModifiers::CONTROL));
        assert!(result.is_none(), "Ctrl+Enter should not submit");
        assert_eq!(
            app.editor.buffer, "hello",
            "Ctrl+Enter should not modify buffer"
        );
    }

    #[test]
    fn test_pke_ctrl_enter_text_not_lost_when_cursor_not_at_end() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "abcdef");
        app.editor.cursor = 2;

        let result = press_key(&mut app, enter(KeyModifiers::CONTROL));
        assert!(result.is_none());
        assert_eq!(
            app.editor.buffer, "abcdef",
            "Ctrl+Enter should not modify buffer"
        );
    }

    #[test]
    fn test_pke_ctrl_enter_then_type_no_stray_newline() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "hello");
        press_key(&mut app, enter(KeyModifiers::CONTROL));
        assert_eq!(app.editor.buffer, "hello");

        type_str(&mut app, "world");
        assert_eq!(app.editor.buffer, "helloworld");
    }

    #[test]
    fn test_pke_multiple_ctrl_enter() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "a");
        press_key(&mut app, enter(KeyModifiers::CONTROL));
        type_str(&mut app, "b");
        press_key(&mut app, enter(KeyModifiers::CONTROL));
        type_str(&mut app, "c");
        assert_eq!(app.editor.buffer, "abc");
    }

    // ===== Ctrl+Enter reported as \n+CONTROL or \n+NONE =====

    #[test]
    fn test_pke_ctrl_enter_as_char_newline_with_control() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "ab");
        let result = press_key(&mut app, char_nl(KeyModifiers::CONTROL));
        assert!(result.is_none(), "Ctrl+Enter should not submit");
        assert_eq!(app.editor.buffer, "ab", "\\n+CONTROL should be no-op");
    }

    #[test]
    fn test_pke_ctrl_enter_as_char_newline_no_modifier() {
        // \n+NONE with no recent modifier state: treat as bare Enter → submit.
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "x");
        let result = press_key(&mut app, char_nl(KeyModifiers::NONE));
        assert!(
            matches!(result, Some(TuiAction::Submit(t)) if t == "x"),
            "\\n+NONE should submit text"
        );
        assert!(app.editor.buffer.is_empty(), "editor cleared after submit");
    }

    #[test]
    fn test_pke_ctrl_enter_as_shift_modifier() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "z");
        let result = press_key(&mut app, enter(KeyModifiers::SHIFT));
        assert!(result.is_none(), "Shift+Enter should not submit");
        assert_eq!(app.editor.buffer, "z\n");
    }

    // ===== Plain Enter must still submit =====

    #[test]
    fn test_pke_plain_enter_submits_text() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        type_str(&mut app, "submit me");
        assert_eq!(app.editor.buffer, "submit me");

        let result = press_key(&mut app, enter(KeyModifiers::NONE));
        assert!(
            matches!(&result, Some(TuiAction::Submit(t)) if t == "submit me"),
            "plain Enter should submit text, got {:?}",
            result,
        );
    }

    // ===== Fast typing + Enter =====

    #[test]
    fn test_pke_fast_typing_ctrl_enter_flushes_accumulator() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        let now = Instant::now();
        for c in "hello".chars() {
            process_key_event(&mut app, key(c), now);
        }
        assert_eq!(app.paste_accumulator, "hello");

        // Ctrl+Enter flushes accumulator, no-op (no newline, no submit)
        let result = process_key_event(&mut app, enter(KeyModifiers::CONTROL), now);
        assert!(result.is_none());
        assert_eq!(
            app.editor.buffer, "hello",
            "Ctrl+Enter flushes but no newline"
        );
        assert!(app.paste_accumulator.is_empty());
    }

    #[test]
    fn test_pke_fast_typing_enter_with_accumulator_adds_newline() {
        // When chars and Enter arrive within <50ms (paste burst), Enter
        // accumulates \n in the accumulator instead of flushing and submitting.
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        let now = Instant::now();
        for c in "fast".chars() {
            process_key_event(&mut app, key(c), now);
        }
        assert_eq!(app.paste_accumulator, "fast");

        // Enter at same timestamp is treated as paste burst → accumulate \n
        let result = process_key_event(&mut app, enter(KeyModifiers::NONE), now);
        assert!(result.is_none());
        assert_eq!(app.paste_accumulator, "fast\n");

        // After idle timeout, finalize flushes to editor
        app.finalize_paste_accumulator(now + std::time::Duration::from_millis(100));
        assert_eq!(app.editor.buffer, "fast\n");
    }

    // ===== drain_key_events integration tests =====
    // These test the drain path that processes leftover events in the channel.
    // The drain functions handle \n/\r as Ctrl+Enter and skip bare Enter events.

    /// Helper: send an Enter event into the channel.
    fn send_enter(tx: &tokio::sync::mpsc::UnboundedSender<crossterm::event::Event>) {
        let _ = tx.send(crossterm::event::Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
    }

    /// Helper: send a Char('\n') event (Ctrl+Enter from some terminals).
    fn send_newline(tx: &tokio::sync::mpsc::UnboundedSender<crossterm::event::Event>) {
        let _ = tx.send(crossterm::event::Event::Key(KeyEvent::new(
            KeyCode::Char('\n'),
            KeyModifiers::NONE,
        )));
    }

    /// Helper: send a regular char event.
    fn send_char(tx: &tokio::sync::mpsc::UnboundedSender<crossterm::event::Event>, c: char) {
        let _ = tx.send(crossterm::event::Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::NONE,
        )));
    }

    #[test]
    fn test_drain_ctrl_enter_as_newline() {
        // \n+NONE in drain with no modifiers tracked: drain skips it (no-op).
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        app.editor.insert_str("abc");
        app.editor.cursor = 3;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        send_newline(&tx);

        let result = drain_key_events(&mut app, &mut rx, Instant::now());
        assert!(result.is_none());
        assert_eq!(
            app.editor.buffer, "abc",
            "drain \\n+NONE should not modify buffer"
        );
    }

    #[test]
    fn test_drain_bare_enter_skipped() {
        // Simulate: stray Enter event in the drain channel.
        // With empty accumulator, drain must skip it entirely.
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        app.editor.insert_str("abc");

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        send_enter(&tx); // should be skipped (empty accumulator)

        let result = drain_key_events(&mut app, &mut rx, Instant::now());
        assert!(
            result.is_none(),
            "bare Enter in drain should not produce action"
        );
        assert_eq!(
            app.editor.buffer, "abc",
            "bare Enter in drain should not modify buffer, got {:?}",
            app.editor.buffer,
        );
    }

    #[test]
    fn test_drain_enter_during_paste_accumulates() {
        // If there IS text in the paste accumulator, drain's Enter should
        // accumulate \n for the paste batch.  During a paste burst drain
        // does NOT force-flush — the accumulator keeps growing so the full
        // paste content stays together for the 100-char threshold check.
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        app.paste_accumulator.push_str("hello");
        app.last_paste_time = Some(Instant::now());

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        send_enter(&tx); // drain should accumulate \n since accumulator is non-empty

        let _ = drain_key_events(&mut app, &mut rx, Instant::now());
        // During a paste burst, drain does NOT flush — the \n stays in the
        // accumulator alongside "hello".  Flush happens later via the timer
        // (finalize_paste_accumulator) or the next non-paste event.
        assert_eq!(
            app.paste_accumulator, "hello\n",
            "Enter during paste should produce 'hello\\n' in accumulator, got {:?}",
            app.paste_accumulator,
        );
        assert!(
            app.editor.buffer.is_empty(),
            "editor buffer should be empty during a paste burst"
        );
    }

    #[test]
    fn test_drain_during_agent_ctrl_enter_as_newline() {
        // \n+NONE in agent drain with no modifiers tracked: no-op.
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        app.editor.insert_str("abc");

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        send_newline(&tx);

        let mut abort = false;
        let result = drain_key_events_during_agent(&mut app, &mut rx, Instant::now(), &mut abort);
        assert!(result.is_none());
        assert!(!abort, "\\n should not trigger abort");
        assert_eq!(
            app.editor.buffer, "abc",
            "\\n+NONE in agent drain should not modify buffer"
        );
    }

    #[test]
    fn test_drain_during_agent_bare_enter_skipped() {
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        app.editor.insert_str("abc");

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        send_enter(&tx); // empty accumulator, should skip

        let mut abort = false;
        let result = drain_key_events_during_agent(&mut app, &mut rx, Instant::now(), &mut abort);
        assert!(result.is_none());
        assert!(!abort);
        assert_eq!(app.editor.buffer, "abc");
    }

    #[test]
    fn test_drain_accumulator_enter_then_char_produces_newline() {
        // Simulate: chars in accumulator, then Enter, then more chars.
        // All through drain. During a paste burst drain does NOT force-flush
        // — the accumulator keeps growing to keep the full paste content
        // together for the 100-char threshold check.
        let mut app = pick_tui::app::TuiApp::new_inner(
            "anthropic",
            "claude-sonnet-4-20250514",
            "Pick",
            "0.1",
            vec![],
            vec![],
            "/tmp",
            None,
            "off",
            None,
            "test",
            "",
        );
        app.paste_accumulator.push_str("abc");
        app.last_paste_time = Some(Instant::now());

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        send_enter(&tx); // accumulate \n in paste batch
        send_char(&tx, 'd'); // accumulate 'd' in paste batch

        let result = drain_key_events(&mut app, &mut rx, Instant::now());
        assert!(
            result.is_none(),
            "drain with batch should not produce action"
        );
        // During a paste burst, drain does NOT flush — "abc\nd" stays in
        // the accumulator.  Flush happens later via the timer
        // (finalize_paste_accumulator) or the next non-paste event.
        assert_eq!(
            app.paste_accumulator, "abc\nd",
            "drain paste batch should accumulate 'abc\\nd', got {:?}",
            app.paste_accumulator,
        );
        assert!(
            app.editor.buffer.is_empty(),
            "editor buffer should be empty during a paste burst"
        );
    }
}
