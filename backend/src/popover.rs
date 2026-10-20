//! Where the popover opens under the tray icon.
//!
//! On macOS the tray click and the icon rect come in global points (origin at
//! the top-left of the main screen) multiplied by the scale factor of the
//! screen the icon is on. Monitor positions and sizes are global points
//! multiplied by that monitor's own scale factor. So every value is divided by
//! the clicked monitor's scale, never the window's.

/// Height of the menu bar when the icon rect is not known, in points.
pub const FALLBACK_MENU_BAR_HEIGHT: f64 = 25.0;

/// A monitor as Tauri reports it: physical position and size, and its scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

/// The tray icon rect from the click event, physical at the clicked monitor's scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl MonitorArea {
    fn left(&self) -> f64 {
        self.x / self.scale
    }

    fn top(&self) -> f64 {
        self.y / self.scale
    }

    fn right(&self) -> f64 {
        self.left() + self.width / self.scale
    }

    fn bottom(&self) -> f64 {
        self.top() + self.height / self.scale
    }

    /// The right and bottom edges belong to the neighbouring monitor.
    fn contains_logical(&self, x: f64, y: f64) -> bool {
        x >= self.left() && x < self.right() && y >= self.top() && y < self.bottom()
    }
}

/// The index of the monitor that was clicked.
///
/// `cursor` is the cursor in global points and decides when known. Without it
/// the physical click is tried at each monitor's scale, which is ambiguous when
/// monitors with different scales sit side by side, so the first match wins.
pub fn find_clicked_monitor(
    monitors: &[MonitorArea],
    cursor: Option<(f64, f64)>,
    click: (f64, f64),
) -> Option<usize> {
    cursor
        .and_then(|(x, y)| monitors.iter().position(|m| m.contains_logical(x, y)))
        .or_else(|| {
            monitors
                .iter()
                .position(|m| m.contains_logical(click.0 / m.scale, click.1 / m.scale))
        })
}

/// The popover's top-left in global points: centered under the icon, just below
/// the menu bar, and kept inside `monitor`.
///
/// Without an icon rect the click's x and a fixed menu bar height are used.
pub fn popover_position(
    monitor: &MonitorArea,
    icon: Option<IconRect>,
    click: (f64, f64),
    window_width: f64,
) -> (f64, f64) {
    let scale = monitor.scale;
    let (center_x, y) = match icon.filter(|r| r.width > 0.0 && r.height > 0.0) {
        Some(r) => ((r.x + r.width / 2.0) / scale, (r.y + r.height) / scale),
        None => (click.0 / scale, monitor.top() + FALLBACK_MENU_BAR_HEIGHT),
    };
    // The left edge wins over the right, so a monitor narrower than the window aligns left
    let x = (center_x - window_width / 2.0)
        .min(monitor.right() - window_width)
        .max(monitor.left());
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTH: f64 = 280.0;

    /// A monitor from its origin and size in points, as Tauri would report it.
    fn monitor(x: f64, y: f64, width: f64, height: f64, scale: f64) -> MonitorArea {
        MonitorArea { x: x * scale, y: y * scale, width: width * scale, height: height * scale, scale }
    }

    /// An icon rect from points, as the tray event would report it.
    fn icon(x: f64, y: f64, width: f64, height: f64, scale: f64) -> IconRect {
        IconRect { x: x * scale, y: y * scale, width: width * scale, height: height * scale }
    }

    fn builtin() -> MonitorArea {
        monitor(0.0, 0.0, 1512.0, 982.0, 2.0)
    }

    #[test]
    fn builtin_retina_centers_under_the_icon() {
        let rect = icon(1200.0, 0.0, 30.0, 24.0, 2.0);

        let pos = popover_position(&builtin(), Some(rect), (2420.0, 20.0), WIDTH);

        assert_eq!(pos, (1075.0, 24.0));
    }

    #[test]
    fn external_non_retina_uses_its_own_scale() {
        // The window may still be on the Retina screen; that scale must not be used
        let external = monitor(1512.0, 0.0, 1920.0, 1080.0, 1.0);
        let rect = icon(3000.0, 0.0, 30.0, 24.0, 1.0);

        let pos = popover_position(&external, Some(rect), (3010.0, 10.0), WIDTH);

        assert_eq!(pos, (2875.0, 24.0));
    }

    #[test]
    fn external_left_of_builtin_has_a_negative_origin() {
        let external = monitor(-1920.0, 0.0, 1920.0, 1080.0, 1.0);
        let rect = icon(-500.0, 0.0, 30.0, 24.0, 1.0);

        let pos = popover_position(&external, Some(rect), (-490.0, 10.0), WIDTH);

        assert_eq!(pos, (-625.0, 24.0));
    }

    #[test]
    fn external_above_builtin_opens_below_its_own_menu_bar() {
        let external = monitor(-200.0, -1080.0, 1920.0, 1080.0, 1.0);
        let rect = icon(1000.0, -1080.0, 30.0, 24.0, 1.0);

        let pos = popover_position(&external, Some(rect), (1010.0, -1070.0), WIDTH);

        assert_eq!(pos, (875.0, -1056.0));
    }

    #[test]
    fn retina_external_above_builtin() {
        let external = monitor(0.0, -1440.0, 2560.0, 1440.0, 2.0);
        let rect = icon(2000.0, -1440.0, 30.0, 24.0, 2.0);

        let pos = popover_position(&external, Some(rect), (4020.0, -2860.0), WIDTH);

        assert_eq!(pos, (1875.0, -1416.0));
    }

    #[test]
    fn taller_menu_bar_follows_the_icon_bottom() {
        // A notched MacBook's menu bar is taller than 25 points
        let rect = icon(1200.0, 0.0, 30.0, 37.0, 2.0);

        let (_, y) = popover_position(&builtin(), Some(rect), (2420.0, 20.0), WIDTH);

        assert_eq!(y, 37.0);
    }

    #[test]
    fn icon_near_the_right_edge_keeps_the_window_on_screen() {
        let rect = icon(1490.0, 0.0, 22.0, 24.0, 2.0);

        let (x, _) = popover_position(&builtin(), Some(rect), (2990.0, 20.0), WIDTH);

        assert_eq!(x, 1512.0 - WIDTH);
    }

    #[test]
    fn icon_near_the_left_edge_of_a_negative_origin_monitor() {
        let external = monitor(-1920.0, 0.0, 1920.0, 1080.0, 1.0);
        let rect = icon(-1915.0, 0.0, 30.0, 24.0, 1.0);

        let (x, _) = popover_position(&external, Some(rect), (-1900.0, 10.0), WIDTH);

        assert_eq!(x, -1920.0);
    }

    #[test]
    fn monitor_narrower_than_the_window_aligns_left() {
        let tiny = monitor(100.0, 0.0, 200.0, 300.0, 1.0);
        let rect = icon(180.0, 0.0, 30.0, 24.0, 1.0);

        let (x, _) = popover_position(&tiny, Some(rect), (190.0, 10.0), WIDTH);

        assert_eq!(x, 100.0);
    }

    #[test]
    fn without_an_icon_rect_falls_back_to_the_click_and_menu_bar_height() {
        let external = monitor(-200.0, -1080.0, 1920.0, 1080.0, 1.0);

        let pos = popover_position(&external, None, (1000.0, -1070.0), WIDTH);

        assert_eq!(pos, (860.0, -1080.0 + FALLBACK_MENU_BAR_HEIGHT));
    }

    #[test]
    fn empty_icon_rect_is_treated_as_unknown() {
        let pos = popover_position(&builtin(), Some(icon(0.0, 0.0, 0.0, 0.0, 2.0)), (2000.0, 20.0), WIDTH);

        assert_eq!(pos, (860.0, FALLBACK_MENU_BAR_HEIGHT));
    }

    #[test]
    fn cursor_picks_the_external_monitor_beside_a_retina_one() {
        let monitors = [builtin(), monitor(1512.0, 0.0, 1920.0, 1080.0, 1.0)];

        // Physical (2000, 10) could also be (1000, 5) on the Retina screen
        let found = find_clicked_monitor(&monitors, Some((2000.0, 10.0)), (2000.0, 10.0));

        assert_eq!(found, Some(1));
    }

    #[test]
    fn cursor_picks_the_builtin_monitor() {
        let monitors = [builtin(), monitor(1512.0, 0.0, 1920.0, 1080.0, 1.0)];

        let found = find_clicked_monitor(&monitors, Some((1000.0, 5.0)), (2000.0, 10.0));

        assert_eq!(found, Some(0));
    }

    #[test]
    fn without_a_cursor_the_click_is_tried_at_each_scale() {
        let monitors = [builtin(), monitor(-1920.0, 0.0, 1920.0, 1080.0, 1.0)];

        let found = find_clicked_monitor(&monitors, None, (-500.0, 10.0));

        assert_eq!(found, Some(1));
    }

    #[test]
    fn a_point_on_no_monitor_finds_none() {
        let monitors = [builtin()];

        assert_eq!(find_clicked_monitor(&monitors, Some((5000.0, 5000.0)), (5000.0, 5000.0)), None);
        assert_eq!(find_clicked_monitor(&monitors, None, (90000.0, 10.0)), None);
    }

    #[test]
    fn monitor_edges_are_half_open() {
        let monitors = [builtin(), monitor(1512.0, 0.0, 1920.0, 1080.0, 1.0)];

        assert_eq!(find_clicked_monitor(&monitors, Some((1512.0, 0.0)), (0.0, 0.0)), Some(1));
        assert_eq!(find_clicked_monitor(&monitors, Some((1511.5, 0.0)), (0.0, 0.0)), Some(0));
    }

    #[test]
    fn mixed_scales_end_to_end() {
        // Clicking the external screen's icon while the window is on the Retina one
        let monitors = [builtin(), monitor(1512.0, 0.0, 1920.0, 1080.0, 1.0)];
        let rect = icon(3400.0, 0.0, 22.0, 24.0, 1.0);

        let index = find_clicked_monitor(&monitors, Some((3411.0, 10.0)), (3411.0, 10.0)).unwrap();
        let pos = popover_position(&monitors[index], Some(rect), (3411.0, 10.0), WIDTH);

        assert_eq!(pos, (1512.0 + 1920.0 - WIDTH, 24.0));
    }
}
