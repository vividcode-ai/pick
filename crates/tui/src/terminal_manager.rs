//! Terminal manager — wraps custom_terminal::Terminal with viewport management,
//! history insertion, and synchronized rendering.

use std::io::Write;

use crossterm::SynchronizedUpdate;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::{Offset, Position, Rect};
use ratatui::text::Line;

use crate::custom_terminal::{Frame, Terminal};
use crate::insert_history::{HistoryLineWrapPolicy, insert_history_lines_with_wrap_policy};

/// Clear from the current viewport top or new viewport top to end of screen
/// on viewport change. Prevents stale shell cells from showing through.
fn clear_for_viewport_change<B: Backend + Write>(
    terminal: &mut Terminal<B>,
    new_area: Rect,
) -> std::io::Result<()>
where
    std::io::Error: From<B::Error>,
{
    let old_area = terminal.viewport_area;
    if old_area.is_empty() {
        terminal.clear_after_position(new_area.as_position())?;
    } else if old_area.y == new_area.y && old_area.height > new_area.height {
        // Viewport shrinking at same Y: clear stale overlay content below the new
        // viewport bottom so it doesn't show through after transition.
        if new_area.bottom() < old_area.bottom() {
            let writer = terminal.backend_mut();
            queue!(writer, MoveTo(0, new_area.bottom()))?;
            queue!(writer, Clear(ClearType::FromCursorDown))?;
        }
        terminal.invalidate_viewport();
    } else {
        terminal.clear_after_position(old_area.as_position())?;
    }
    Ok(())
}

/// Type alias for the terminal type used in Pick.
pub type PickTerminal = Terminal<CrosstermBackend<std::io::Stdout>>;

/// Pending history lines to be inserted into scrollback before the next draw.
struct PendingHistory {
    lines: Vec<Line<'static>>,
    wrap_policy: HistoryLineWrapPolicy,
}

/// Manages terminal lifecycle, viewport, and history insertion.
///
/// - Owns the `custom_terminal::Terminal`
/// - Manages viewport area and history row tracking
/// - Provides synchronized draw() with automatic scrollback insertion
pub struct TerminalManager<B: Backend + Write>
where
    std::io::Error: From<B::Error>,
{
    terminal: Terminal<B>,
    pending_history: Vec<PendingHistory>,
    /// Whether the first draw has occurred. On first draw, skip clearing
    /// the viewport to preserve existing terminal content above (inline mode).
    has_drawn: bool,
    /// Whether the viewport has ever reached the screen bottom.
    /// Once true, the viewport is pinned to the screen bottom on every draw
    /// to prevent content jumping when streaming tail / status disappear.
    has_reached_bottom: bool,
    /// Transient height from the previous draw. Used to detect when a transient
    /// overlay (selection popup, autocomplete, dialog) is dismissed, so we don't
    /// re-pin the viewport to screen bottom and destroy valid scrollback content.
    prev_transient_height: u16,
}

impl TerminalManager<CrosstermBackend<std::io::Stdout>> {
    /// Create a new terminal manager with the default stdout backend.
    /// Viewport starts at the current cursor position (inline mode, no screen clear).
    pub fn new() -> std::io::Result<Self> {
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::with_options(backend)?;

        // Set initial viewport to full terminal width so insert_history
        // uses correct wrap_width (not the default width=0).
        // Y stays at cursor position (inline mode).
        let screen_size = terminal.size().unwrap_or_default();
        let init_y =
            if screen_size.height > 0 && terminal.last_known_cursor_pos.y >= screen_size.height {
                terminal.last_known_cursor_pos.y % screen_size.height
            } else {
                terminal.last_known_cursor_pos.y
            };
        terminal.set_viewport_area(Rect::new(0, init_y, screen_size.width, 0));

        Ok(Self {
            terminal,
            pending_history: Vec::new(),
            has_drawn: false,
            has_reached_bottom: false,
            prev_transient_height: 0,
        })
    }
}

impl<B: Backend + Write> TerminalManager<B>
where
    std::io::Error: From<B::Error>,
{
    /// Access the underlying terminal.
    pub fn terminal(&self) -> &Terminal<B> {
        &self.terminal
    }

    /// Access the underlying terminal mutably.
    pub fn terminal_mut(&mut self) -> &mut Terminal<B> {
        &mut self.terminal
    }

    /// Reset terminal state after screen clear (for /new).
    /// Clears pending history, resets has_drawn, and re-anchors the
    /// viewport at y=0 so the next draw pass starts from a clean state.
    pub fn reset_for_new_session(&mut self) -> std::io::Result<()> {
        self.pending_history.clear();
        self.has_drawn = false;
        self.has_reached_bottom = false;
        self.prev_transient_height = 0;
        let screen_size = self.terminal.size()?;
        self.terminal
            .set_viewport_area(Rect::new(0, 0, screen_size.width, 0));
        Ok(())
    }

    /// Compute any viewport adjustment needed due to terminal resize + cursor movement.
    ///
    /// On resize, if the cursor also moved, adjust the viewport y-offset proportionally
    /// so the cursor stays in the same relative position.
    fn pending_viewport(&mut self) -> std::io::Result<Option<Rect>> {
        let screen_size = self.terminal.size()?;
        let last_known_screen_size = self.terminal.last_known_screen_size;
        if screen_size != last_known_screen_size
            && let Ok(cursor_pos) = self.terminal.get_cursor_position()
        {
            let last_known_cursor_pos = self.terminal.last_known_cursor_pos;
            if cursor_pos.y != last_known_cursor_pos.y {
                let offset = Offset {
                    x: 0,
                    y: cursor_pos.y as i32 - last_known_cursor_pos.y as i32,
                };
                return Ok(Some(self.terminal.viewport_area.offset(offset)));
            }
        }
        Ok(None)
    }

    /// Queue lines for scrollback insertion before the next draw.
    /// History is written above the viewport before each frame render.
    pub fn insert_history(&mut self, lines: Vec<Line<'static>>) {
        if lines.is_empty() {
            return;
        }
        self.pending_history.push(PendingHistory {
            lines,
            wrap_policy: HistoryLineWrapPolicy::PreWrap,
        });
    }

    /// Flush all pending history lines into terminal scrollback.
    /// Returns true if any history was flushed (callers should skip
    /// clear_for_viewport_change when true, since insert_history already
    /// manages screen scrolling).
    fn flush_pending_history(&mut self) -> std::io::Result<bool> {
        let had_history = !self.pending_history.is_empty();
        for pending in self.pending_history.drain(..) {
            insert_history_lines_with_wrap_policy(
                &mut self.terminal,
                pending.lines,
                pending.wrap_policy,
            )?;
        }
        Ok(had_history)
    }

    /// Invalidate the viewport so the next draw repaints everything.
    /// Call this after insert_history to prevent stale buffer diff positions.
    pub fn invalidate_viewport(&mut self) {
        self.terminal.invalidate_viewport();
    }

    /// Update the viewport area based on current terminal size.
    pub fn update_viewport(&mut self) -> std::io::Result<()> {
        let screen_size = self.terminal.size()?;
        let area = self.terminal.viewport_area;

        // Only update if size changed
        if screen_size == self.terminal.last_known_screen_size {
            return Ok(());
        }

        // On resize, keep the viewport at the same y position but adjust
        // width and height to match the new screen size
        let new_area = Rect::new(
            0,
            area.y.min(screen_size.height.saturating_sub(1)),
            screen_size.width,
            screen_size
                .height
                .saturating_sub(area.y)
                .min(screen_size.height),
        );
        self.terminal.set_viewport_area(new_area);
        Ok(())
    }

    /// Draw a single frame with the given content height.
    ///
    /// Architecture:
    /// 1. Set viewport at current position BEFORE flush so insert_history
    ///    writes scrollback above the viewport.
    /// 2. Flush history — RI may push viewport down as scrollback grows.
    /// 3. After flush, check if viewport overflows the screen:
    ///    - If yes, scroll the full screen up via scroll region + CRLF
    ///      (matching insert_history's own scroll mechanism). The viewport
    ///      Y is adjusted to follow the physical shift. No
    ///      clear_for_viewport_change needed — the scroll blanked rows.
    ///    - If no overflow but RI pushed viewport down, align the area.
    /// 4. Draw frame via buffer-diffed rendering.
    pub fn draw(
        &mut self,
        content_height: u16,
        transient_height: u16,
        render_fn: impl FnOnce(&mut Frame),
    ) -> std::io::Result<()> {
        std::io::stdout().sync_update(|_| {
            let size = self.terminal.size()?;
            let visible_height = content_height.min(size.height);
            let current_top = self.terminal.viewport_area.y;

            // 1. Set viewport at its current position BEFORE flush so
            //    insert_history writes scrollback above the viewport.
            let pre_area = Rect::new(0, current_top, size.width, visible_height);
            if pre_area != self.terminal.viewport_area {
                if !self.has_drawn {
                    self.terminal.clear_after_position(pre_area.as_position())?;
                } else {
                    clear_for_viewport_change(&mut self.terminal, pre_area)?;
                }
                self.terminal.set_viewport_area(pre_area);
                self.terminal.invalidate_viewport();
            }

            // 2. Flush scrollback — written above viewport.
            //    RI may push viewport down as scrollback grows.
            self.flush_pending_history()?;

            // 3. After flush, check overflow.
            let final_y = self.terminal.viewport_area.y;
            let overflow = final_y
                .saturating_add(visible_height)
                .saturating_sub(size.height);

            if overflow > 0 {
                // Scroll the full screen up using scroll region + CRLF,
                // matching the mechanism used by insert_history which
                // correctly populates the terminal's history buffer.
                let writer = self.terminal.backend_mut();
                write!(writer, "\x1b[r")?;
                queue!(writer, MoveTo(0, size.height.saturating_sub(1)))?;
                for _ in 0..overflow {
                    queue!(writer, Print("\r\n"))?;
                }
                Write::flush(writer)?;

                let adjusted_y = final_y.saturating_sub(overflow);
                let area = Rect::new(0, adjusted_y, size.width, visible_height);
                self.terminal.set_viewport_area(area);
                self.terminal.invalidate_viewport();
            } else if final_y != current_top {
                // RI pushed viewport down but no overflow: align area.
                let area = Rect::new(0, final_y, size.width, visible_height);
                if area != self.terminal.viewport_area {
                    if !self.has_drawn {
                        self.terminal.clear_after_position(area.as_position())?;
                    } else {
                        clear_for_viewport_change(&mut self.terminal, area)?;
                    }
                    self.terminal.set_viewport_area(area);
                    self.terminal.invalidate_viewport();
                }
            }

            // 3x. Detect transient overlay dismissal.
            // When transient_height drops from >0 to 0, a transient overlay
            // (selection popup, autocomplete, dialog) just ended. During the
            // overlay the viewport Y was set correctly relative to scrollback
            // (step 3 overflow adjusted it). Don't re-pin to screen bottom
            // now — that would call clear_for_viewport_change which destroys
            // valid scrollback content, creating blank rows.
            let transient_just_ended = self.prev_transient_height > 0 && transient_height == 0;
            self.prev_transient_height = transient_height;
            if transient_just_ended && self.has_reached_bottom {
                self.has_reached_bottom = false;
            }

            // 3a. Pin viewport to scrollback bottom.
            //
            // The viewport naturally tracks the scrollback bottom: every
            // insert_history call pushes the viewport down as content is
            // written above it.
            //
            // Once scrollback fills the screen ("reached bottom"), pin the
            // viewport to the screen bottom — at that point scrollback
            // bottom ≡ screen bottom. The pin also handles viewport resizing
            // when content height changes (e.g. stream tail ends while
            // autocomplete is active), clearing stale cells below the new
            // viewport area.
            //
            // Crucially, never *unpin* just because transient_height > 0
            // (autocomplete, selection popups, dialogs). The original code
            // did this, which caused the viewport to drift away from the
            // scrollback bottom whenever a transient overlay appeared.
            if self.has_reached_bottom {
                let cur = self.terminal.viewport_area;
                let pinned_y = size.height.saturating_sub(visible_height);
                if pinned_y != cur.y {
                    let new_area = Rect::new(0, pinned_y, size.width, visible_height);
                    clear_for_viewport_change(&mut self.terminal, new_area)?;
                    self.terminal.set_viewport_area(new_area);
                    self.terminal.invalidate_viewport();
                }
            } else {
                let viewport_bottom = self.terminal.viewport_area.bottom();
                let effective_bottom = viewport_bottom.saturating_sub(transient_height);
                if effective_bottom >= size.height {
                    self.has_reached_bottom = true;
                }
            }

            // 4. Draw the frame
            self.terminal.draw(|frame| {
                render_fn(frame);
            })
        })??;
        self.has_drawn = true;
        Ok(())
    }

    /// Clean up terminal state on exit.
    /// Positions cursor after the TUI content and clears leftover blank rows.
    pub fn cleanup(&mut self) -> std::io::Result<()> {
        use crossterm::queue;

        let screen_size = self.terminal.size().unwrap_or_default();
        let bottom = self.terminal.viewport_area.bottom();
        let cursor_y = bottom.min(screen_size.height.saturating_sub(1));
        self.terminal
            .set_cursor_position(Position { x: 0, y: cursor_y })?;
        queue!(
            self.terminal.backend_mut(),
            crossterm::style::SetAttribute(crossterm::style::Attribute::Reset),
        )?;
        if bottom < screen_size.height {
            queue!(
                self.terminal.backend_mut(),
                Clear(ClearType::FromCursorDown)
            )?;
        }
        self.terminal.show_cursor()?;
        self.terminal.reset_cursor_style()?;
        std::io::stdout().flush()?;
        Ok(())
    }
}

impl<B: Backend + Write> Drop for TerminalManager<B>
where
    std::io::Error: From<B::Error>,
{
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
