//! Pure logic for the Windows shell integration (sleep timer, flyout placement, window geometry, resume/network
//! detection, taskbar progress, settings). No Windows or Tauri calls live here, so all of it is tested on any OS.

pub mod anchor;
pub mod atomic;
pub mod audiodev;
pub mod cli;
pub mod datadir;
pub mod envfile;
pub mod geometry;
pub mod power;
pub mod progress;
pub mod settings;
pub mod sleep;
pub mod toast;
pub mod tray_controls;
pub mod update;
pub mod winui;

/// A rectangle in physical (device) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub const fn right(&self) -> i32 {
        self.x + self.w
    }
    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub const fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
    pub fn intersection(&self, o: &Rect) -> Option<Rect> {
        let (x, y) = (self.x.max(o.x), self.y.max(o.y));
        let (r, b) = (self.right().min(o.right()), self.bottom().min(o.bottom()));
        (r > x && b > y).then(|| Rect::new(x, y, r - x, b - y))
    }
    pub fn contains(&self, p: (i32, i32)) -> bool {
        p.0 >= self.x && p.0 < self.right() && p.1 >= self.y && p.1 < self.bottom()
    }
}
