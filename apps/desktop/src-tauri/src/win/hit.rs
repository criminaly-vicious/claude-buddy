//! Decides when the overlay must take mouse input: while the cursor is over a
//! buddy or the hover card, and during a drag that started on one. Pure state
//! machine, fed by the mouse hook on Windows; tested on every platform.

use super::geometry::{hit, to_screen, LogicalRect, Rect};

pub struct HitTester {
    regions: Vec<LogicalRect>,
    origin: (i32, i32),
    scale: f64,
    screen: Vec<Rect>,
    cursor: Option<(i32, i32)>,
    dragging: bool,
    interactive: bool,
    /// Called with the new value whenever [`HitTester::interactive`] changes.
    on_change: Box<dyn FnMut(bool) + Send>,
}

impl HitTester {
    pub fn new(on_change: Box<dyn FnMut(bool) + Send>) -> Self {
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
        self.recompute();
    }

    /// The overlay's top-left corner on screen and its DPI scale.
    pub fn set_geometry(&mut self, origin: (i32, i32), scale: f64) {
        self.origin = origin;
        self.scale = scale;
        self.recompute();
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

    fn recompute(&mut self) {
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
            (self.on_change)(interactive);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    fn tester() -> (HitTester, Arc<Mutex<Vec<bool>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&calls);
        let mut tester = HitTester::new(Box::new(move |v| log.lock().unwrap().push(v)));
        tester.set_geometry((1000, 500), 2.0);
        tester.set_regions(vec![LogicalRect {
            x: 10.0,
            y: 10.0,
            width: 32.0,
            height: 32.0,
        }]);
        (tester, calls)
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
        assert_eq!(
            *calls.lock().unwrap(),
            [true, false],
            "notified only on change"
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
    fn region_and_geometry_changes_reevaluate_the_cursor() {
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
        // Overlay moved so the region is under the cursor again.
        t.set_geometry((1025, 525), 1.0);
        assert!(t.interactive());
        assert_eq!(*calls.lock().unwrap(), [true, false, true]);
    }
}
