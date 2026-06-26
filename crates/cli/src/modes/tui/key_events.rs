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
const VK_SHIFT: i32 = 0x10;
#[cfg(windows)]
fn windows_is_key_down(vk: i32) -> bool {
    // GetAsyncKeyState returns a SHORT (i16).  The high bit (sign)
    // is set when the key is currently down, so a negative result
    // means the key is pressed.
    unsafe { GetAsyncKeyState(vk) < 0 }
}

/// Resolve the actual modifier state for a key event.
/// On Windows, crossterm's modifier reporting can be unreliable for
/// Enter (the terminal might not set dwControlKeyState for VK_RETURN).
/// We supplement it by polling the OS-level key state.
#[cfg(windows)]
fn resolve_modifiers(event: &crossterm::event::KeyEvent) -> KeyModifiers {
    let raw = event.modifiers;
    let mut m = raw;
    if windows_is_key_down(VK_CONTROL) {
        m |= KeyModifiers::CONTROL;
    }
    if windows_is_key_down(VK_SHIFT) {
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
        return tui.handle_key(key.code, key.modifiers);
    }

    // ---- Enter key: special handling ----
    // On Windows some terminals report Ctrl+Enter / Shift+Enter
    // as plain Enter+NONE without the CONTROL/SHIFT modifiers.
    // We query the OS-level key state to catch these cases.
    if key.code == KeyCode::Enter {
        let resolved = resolve_modifiers(&key);

        // If Ctrl or Shift is actually held, treat as newline insert
        // regardless of what crossterm reported.
        if resolved.intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL) {
            tui.force_flush_paste_accumulator();
            tui.editor.insert_newline_auto_indent();
            return None;
        }

        // Genuine bare Enter (no modifiers): check paste accumulator.
        if !tui.paste_accumulator.is_empty() {
            // Fast typing — text is still in the accumulator.
            // Append \n, flush, and let the editor show it.
            tui.paste_accumulator.push('\n');
            tui.last_paste_time = Some(now);
            tui.force_flush_paste_accumulator();
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
                // ASCII control character: route directly to handle_key
                tui.force_flush_paste_accumulator();
                return tui.handle_key(key.code, key.modifiers);
            }
            KeyCode::Char(c) => {
                tui.paste_accumulator.push(c);
                tui.last_paste_time = Some(now);
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
            tui.handle_paste(&text);
        }
        return None;
    }

    // Esc always aborts agent
    if key.code == KeyCode::Esc {
        tui.force_flush_paste_accumulator();
        return Some(TuiAction::Quit);
    }

    // ---- Enter: resolve modifiers on Windows ----
    if key.code == KeyCode::Enter {
        let resolved = resolve_modifiers(&key);
        if resolved.intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL) {
            tui.force_flush_paste_accumulator();
            tui.editor.insert_newline_auto_indent();
            return None;
        }
        if !tui.paste_accumulator.is_empty() {
            tui.paste_accumulator.push('\n');
            tui.last_paste_time = Some(now);
            tui.force_flush_paste_accumulator();
            return None;
        }
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
                tui.force_flush_paste_accumulator();
                return tui.handle_key(key.code, key.modifiers);
            }
            KeyCode::Char(c) => {
                tui.paste_accumulator.push(c);
                tui.last_paste_time = Some(now);
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
    _now: Instant,
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
                        tui.force_flush_paste_accumulator();
                        if let Some(a) = tui.handle_key(key.code, key.modifiers) {
                            action = Some(a);
                            had_action = true;
                        }
                        continue;
                    }
                    match key.code {
                        KeyCode::Char(c) if (c as u32) <= 0x1F => {
                            // Handle \n/\r as newline insertion (Ctrl+Enter
                            // on terminals that report it as a bare control
                            // character).  Other ASCII control chars route
                            // to handle_key as before.
                            if c == '\n' || c == '\r' {
                                tui.force_flush_paste_accumulator();
                                tui.editor.insert_newline_auto_indent();
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
                            tui.paste_accumulator.push(c);
                            continue;
                        }
                        KeyCode::Enter => {
                            // Extra Enter event in drain — the main
                            // select!/process_key_event path handles the
                            // primary event.  Only accumulate \n when
                            // already in a paste batch; otherwise skip.
                            if !tui.paste_accumulator.is_empty() {
                                tui.paste_accumulator.push('\n');
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
                    tui.force_flush_paste_accumulator();
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

        tui.force_flush_paste_accumulator();

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
    _now: Instant,
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
                            // Handle \n/\r as newline insertion (Ctrl+Enter
                            // on terminals that report it as bare control char).
                            if c == '\n' || c == '\r' {
                                tui.force_flush_paste_accumulator();
                                tui.editor.insert_newline_auto_indent();
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
                            tui.paste_accumulator.push(c);
                            continue;
                        }
                        KeyCode::Enter => {
                            // Extra Enter event in drain. Only accumulate
                            // \n if already in a paste batch; skip otherwise.
                            if !tui.paste_accumulator.is_empty() {
                                tui.paste_accumulator.push('\n');
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
                    tui.force_flush_paste_accumulator();
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

        tui.force_flush_paste_accumulator();

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

    // ===== BUG 1: Ctrl+Enter clears text =====

    #[test]
    fn test_pke_ctrl_enter_preserves_text_on_first_line() {
        // Normal typing: "hello" + Ctrl+Enter → "hello\n"
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
        assert_eq!(
            app.editor.buffer, "hello",
            "slow typing should insert into buffer"
        );

        let result = press_key(&mut app, enter(KeyModifiers::CONTROL));
        assert!(result.is_none(), "Ctrl+Enter should not submit");
        assert_eq!(
            app.editor.buffer, "hello\n",
            "BUG 1: Ctrl+Enter cleared text! buffer={:?}",
            app.editor.buffer,
        );
    }

    #[test]
    fn test_pke_ctrl_enter_text_not_lost_when_cursor_not_at_end() {
        // Type "abcdef", move cursor to position 2, Ctrl+Enter.
        // Verify "ab" is preserved, newline inserted after cursor.
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
        // Move cursor to position 2 (after "ab")
        app.editor.cursor = 2;

        let result = press_key(&mut app, enter(KeyModifiers::CONTROL));
        assert!(result.is_none());
        assert_eq!(
            app.editor.buffer, "ab\ncdef",
            "Ctrl+Enter in middle of text lost characters! buffer={:?}",
            app.editor.buffer,
        );
    }

    // ===== BUG 2: typing after Ctrl+Enter adds stray newline =====

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
        assert_eq!(app.editor.buffer, "hello\n");

        type_str(&mut app, "world");
        assert_eq!(
            app.editor.buffer, "hello\nworld",
            "BUG 2: typing after Ctrl+Enter inserted stray newline! buffer={:?}",
            app.editor.buffer,
        );
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
        assert_eq!(app.editor.buffer, "a\nb\nc");
    }

    // ===== Ctrl+Enter reported in various ways by terminals =====

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
        press_key(&mut app, char_nl(KeyModifiers::CONTROL));
        assert_eq!(app.editor.buffer, "ab\n");
        type_str(&mut app, "cd");
        assert_eq!(app.editor.buffer, "ab\ncd");
    }

    #[test]
    fn test_pke_ctrl_enter_as_char_newline_no_modifier() {
        // Worst case: terminal reports Ctrl+Enter as Char('\n')/NONE.
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
        press_key(&mut app, char_nl(KeyModifiers::NONE));
        assert_eq!(app.editor.buffer, "x\n");
        type_str(&mut app, "y");
        assert_eq!(app.editor.buffer, "x\ny");
    }

    #[test]
    fn test_pke_ctrl_enter_as_shift_modifier() {
        // Some Windows terminals report Ctrl+Enter as Enter+SHIFT.
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
        press_key(&mut app, enter(KeyModifiers::SHIFT));
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
        // Fast typing chars (in accumulator) then Ctrl+Enter.
        // Must flush, then insert newline.
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
        // Fast typing: chars stay in accumulator
        for c in "hello".chars() {
            process_key_event(&mut app, key(c), now);
        }
        assert!(
            app.editor.buffer.is_empty(),
            "buffer empty during fast typing"
        );
        assert_eq!(app.paste_accumulator, "hello");

        // Ctrl+Enter flushes accumulator + inserts newline
        let result = process_key_event(&mut app, enter(KeyModifiers::CONTROL), now);
        assert!(result.is_none());
        assert_eq!(app.editor.buffer, "hello\n");
        assert!(app.paste_accumulator.is_empty());
    }

    #[test]
    fn test_pke_fast_typing_enter_with_accumulator_adds_newline() {
        // Fast typing + plain Enter (no modifiers, text in accumulator).
        // Should add \n and flush, NOT submit.
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

        // Plain Enter with non-empty accumulator = paste-enter (add newline)
        let result = process_key_event(&mut app, enter(KeyModifiers::NONE), now);
        assert!(
            result.is_none(),
            "Enter with non-empty accumulator should not submit"
        );
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
        // Simulate: terminal sends \n (Ctrl+Enter) through the drain path.
        // drain_key_events must treat \n as newline insertion.
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
        send_newline(&tx); // drain sees Char('\n')

        let result = drain_key_events(&mut app, &mut rx, Instant::now());
        assert!(
            result.is_none(),
            "drain should not return an action for Ctrl+Enter"
        );
        assert_eq!(
            app.editor.buffer, "abc\n",
            "drain: Ctrl+Enter as \\n should insert newline, got {:?}",
            app.editor.buffer,
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
        // accumulate \n for the paste batch.  On exit drain calls
        // force_flush_paste_accumulator which moves the accumulated
        // text + \n into the editor buffer.
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
        // force_flush moved "hello\n" to the editor buffer
        assert_eq!(
            app.editor.buffer, "hello\n",
            "Enter during paste should produce 'hello\\n' in editor, got {:?}",
            app.editor.buffer,
        );
        assert!(
            app.paste_accumulator.is_empty(),
            "paste accumulator should be flushed"
        );
    }

    #[test]
    fn test_drain_during_agent_ctrl_enter_as_newline() {
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
        assert_eq!(app.editor.buffer, "abc\n");
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
        // Simulate: user is fast-typing, chars in accumulator, then Enter
        // then more chars. All through drain. On exit drain force-flushes
        // the accumulated batch ("abc\nd") into the editor buffer.
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
        // After drain exits (calling force_flush), the editor buffer
        // should have the combined text: "abc\nd"
        assert_eq!(
            app.editor.buffer, "abc\nd",
            "drain paste batch should flush 'abc\\nd' to editor, got {:?}",
            app.editor.buffer,
        );
        assert!(
            app.paste_accumulator.is_empty(),
            "accumulator should be flushed"
        );
    }
}
