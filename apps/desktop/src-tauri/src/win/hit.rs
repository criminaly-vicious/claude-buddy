//! Decides when the overlay must take mouse input: while the cursor is over a
//! buddy or the hover balloon, and during a drag that started on one. Pure
//! state machine, fed by the mouse hook on Windows; tested on every platform.

use super::geometry::{hit, to_screen, LogicalRect, Rect};

/// What the overlay is told when the cursor enters or leaves its regions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerChange {
    /// The overlay now takes the mouse.
    pub interactive: bool,
    /// The cursor in logical pixels relative to the overlay's top-left
    /// corner, so the page can react before its first own mouse event.
    pub at: Option<(f64, f64)>,
}

pub struct HitTester {
    /// Regions as the frontend sent them, relative to the overlay.
    regions: Vec<LogicalRect>,
    origin: (i32, i32),
    scale: f64,
    /// The regions on screen, mapped with the geometry current when they arrived.
    screen: Vec<Rect>,
    cursor: Option<(i32, i32)>,
    dragging: bool,
    interactive: bool,
    /// Called whenever [`HitTester::interactive`] changes.
    on_change: Box<dyn FnMut(PointerChange) + Send>,
}

impl HitTester {
    pub fn new(on_change: Box<dyn FnMut(PointerChange) + Send>) -> Self {
        Self {
            regions: Vec::new(),
            origin: (0, 0),
            scale: 1.0,
            screen: Vec::new(),
            cursor: None,
            dragging: false,
            interactive: false,
            on_change,
        }
    }

    /// Whether the overlay should receive the mouse right now.
    #[cfg(test)]
    pub fn interactive(&self) -> bool {
        self.interactive
    }

    /// Regions in logical pixels relative to the overlay, from the frontend.
    pub fn set_regions(&mut self, regions: Vec<LogicalRect>) {
        self.regions = regions;
        self.remap();
    }

    /// The overlay's top-left corner on screen and its DPI scale.
    ///
    /// When the window only grows or shrinks around its anchored corner
    /// (`content_moved` false), what it shows stays where it was on screen,
    /// but the regions the page sent are now relative to the old corner:
    /// remapping them would move them away from the buddies until the page
    /// sends new ones, and the cursor would seem to leave. So the screen
    /// regions are kept until then. When the content really moved (another
    /// corner, monitor, DPI or taskbar), the regions are remapped at once.
    pub fn set_geometry(&mut self, origin: (i32, i32), scale: f64, content_moved: bool) {
        self.origin = origin;
        self.scale = scale;
        if content_moved {
            self.remap();
        }
    }

    /// The cursor moved. The mouse hook feeds this (Windows only).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn moved(&mut self, x: i32, y: i32) {
        self.cursor = Some((x, y));
        self.update();
    }

    /// A drag only starts on a region; it keeps the overlay interactive
    /// until the button is released, even if the cursor leaves the region.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn button_down(&mut self, x: i32, y: i32) {
        self.cursor = Some((x, y));
        self.dragging = hit(&self.screen, x, y);
        self.update();
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn button_up(&mut self, x: i32, y: i32) {
        self.cursor = Some((x, y));
        self.dragging = false;
        self.update();
    }

    fn remap(&mut self) {
        self.screen = self
            .regions
            .iter()
            .map(|r| to_screen(*r, self.origin, self.scale))
            .collect();
        self.update();
    }

    fn update(&mut self) {
        let over = self.cursor.is_some_and(|(x, y)| hit(&self.screen, x, y));
        let interactive = over || self.dragging;
        if interactive != self.interactive {
            self.interactive = interactive;
            let at = self.cursor.map(|(x, y)| {
                (
                    f64::from(x - self.origin.0) / self.scale,
                    f64::from(y - self.origin.1) / self.scale,
                )
            });
            (self.on_change)(PointerChange { interactive, at });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    fn tester() -> (HitTester, Arc<Mutex<Vec<PointerChange>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&calls);
        let mut tester = HitTester::new(Box::new(move |c| log.lock().unwrap().push(c)));
        tester.set_geometry((1000, 500), 2.0, true);
        tester.set_regions(vec![LogicalRect {
            x: 10.0,
            y: 10.0,
            width: 32.0,
            height: 32.0,
        }]);
        (tester, calls)
    }

    fn flags(calls: &Arc<Mutex<Vec<PointerChange>>>) -> Vec<bool> {
        calls
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.interactive)
            .collect()
    }

    // The region covers screen x 1020..1084, y 520..584.

    #[test]
    fn becomes_interactive_only_over_a_region() {
        let (mut t, calls) = tester();
        t.moved(0, 0);
        assert!(!t.interactive());
        t.moved(1030, 530);
        assert!(t.interactive());
        t.moved(1083, 583);
        t.moved(1084, 530);
        assert!(!t.interactive());
        assert_eq!(flags(&calls), [true, false], "notified only on change");
    }

    #[test]
    fn entering_reports_where_the_cursor_is_in_logical_pixels() {
        let (mut t, calls) = tester();
        t.moved(1030, 540);
        assert_eq!(
            calls.lock().unwrap()[0],
            PointerChange {
                interactive: true,
                at: Some((15.0, 20.0))
            }
        );
    }

    #[test]
    fn drag_started_on_a_region_stays_interactive() {
        let (mut t, _) = tester();
        t.button_down(1030, 530);
        t.moved(200, 200);
        assert!(t.interactive(), "dragging outside the region");
        t.button_up(200, 200);
        assert!(!t.interactive());
    }

    #[test]
    fn press_outside_a_region_is_not_a_drag() {
        let (mut t, _) = tester();
        t.button_down(10, 10);
        t.moved(1030, 530);
        t.moved(10, 10);
        assert!(!t.interactive());
    }

    #[test]
    fn region_changes_reevaluate_the_cursor() {
        let (mut t, calls) = tester();
        t.moved(1030, 530);
        assert!(t.interactive());
        // Buddy left: no regions under the cursor any more.
        t.set_regions(Vec::new());
        assert!(!t.interactive());
        t.set_regions(vec![LogicalRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }]);
        assert!(!t.interactive());
        assert_eq!(flags(&calls), [true, false]);
    }

    #[test]
    fn growing_around_the_anchor_keeps_the_cursor_on_the_buddy() {
        // The window grows 100 px upwards (balloon, expand): its corner moves,
        // the buddy does not. Until the page sends new regions, the old
        // screen regions still describe where the buddy is.
        let (mut t, calls) = tester();
        t.moved(1030, 530);
        t.set_geometry((1000, 400), 2.0, false);
        assert!(t.interactive(), "stale relative regions must not count");
        // The page's new regions, relative to the taller window.
        t.set_regions(vec![LogicalRect {
            x: 10.0,
            y: 60.0,
            width: 32.0,
            height: 32.0,
        }]);
        assert!(t.interactive());
        assert_eq!(flags(&calls), [true]);
    }

    #[test]
    fn moving_the_content_remaps_the_regions_at_once() {
        let (mut t, calls) = tester();
        t.moved(1030, 530);
        // Another corner or monitor: the buddies really moved.
        t.set_geometry((0, 0), 1.0, true);
        assert!(!t.interactive());
        // Overlay moved so the region is under the cursor again.
        t.set_geometry((1020, 520), 1.0, true);
        assert!(t.interactive());
        assert_eq!(flags(&calls), [true, false, true]);
    }
}
