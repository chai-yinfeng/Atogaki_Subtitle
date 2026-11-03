//! The live caption panel owns its drag events; its WebView's Tauri window is hidden.
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSEventType, NSPanel, NSScreen, NSWindowStyleMask,
    NSWindowTitleVisibility,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::cell::Cell;

#[derive(Clone, Copy)]
struct HeightDrag {
    frame: NSRect,
    mouse_y: f64,
    top: bool,
}
#[derive(Default)]
struct PanelIvars {
    height_drag: Cell<Option<HeightDrag>>,
}

fn height_frame(frame: NSRect, delta: f64, top: bool, min: f64, max: f64) -> NSRect {
    let height = (frame.size.height + if top { delta } else { -delta }).clamp(min, max.max(min));
    NSRect::new(
        NSPoint::new(
            frame.origin.x,
            if top {
                frame.origin.y
            } else {
                frame.origin.y + frame.size.height - height
            },
        ),
        NSSize::new(frame.size.width, height),
    )
}

define_class!(
    #[unsafe(super(NSPanel))]
    #[name = "AtogakiOnlineCaptionPanel"]
    #[thread_kind = MainThreadOnly]
    #[ivars = PanelIvars]
    struct OnlineCaptionPanel;

    impl OnlineCaptionPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool { !self.ignoresMouseEvents() }

        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            let kind = event.r#type();
            if kind == NSEventType::LeftMouseDragged {
                if let Some(drag) = self.ivars().height_drag.get() {
                    let max = self.screen().map(|s| {
                        let visible = s.visibleFrame();
                        if drag.top { visible.origin.y + visible.size.height - drag.frame.origin.y }
                        else { drag.frame.origin.y + drag.frame.size.height - visible.origin.y }
                    }).unwrap_or(self.maxSize().height);
                    self.setFrame_display(height_frame(drag.frame, self.convertPointToScreen(event.locationInWindow()).y - drag.mouse_y, drag.top, self.minSize().height, max), true);
                    return;
                }
            }
            if kind == NSEventType::LeftMouseUp && self.ivars().height_drag.take().is_some() { return; }
            if kind == NSEventType::LeftMouseDown {
                // FullSizeContentView's event base coordinates can exclude
                // titlebar insets. Hit-test against the actual screen frame.
                let frame = self.frame();
                let mouse = self.convertPointToScreen(event.locationInWindow());
                let point = NSPoint::new(mouse.x - frame.origin.x, mouse.y - frame.origin.y);
                let size = frame.size;
                // Reserve top/bottom 8pt for vertical resizing before title drag.
                // Native AppKit still owns horizontal edges and corner handling.
                if point.x > 12.0 && point.x < size.width - 12.0
                    && (point.y <= 8.0 || point.y >= size.height - 8.0) {
                    self.ivars().height_drag.set(Some(HeightDrag { frame: self.frame(), mouse_y: self.convertPointToScreen(event.locationInWindow()).y, top: point.y >= size.height - 8.0 }));
                    return;
                }
                // Matches the reserved 32px header in online.css.
                if point.y >= size.height - 42.0 && point.y <= size.height - 8.0
                    && point.x >= 16.0 && point.x < size.width - 84.0 {
                    self.performWindowDragWithEvent(event);
                    return;
                }
            }
            unsafe { let _: () = msg_send![super(self), sendEvent: event]; }
        }
    }
);

pub(crate) fn new(
    frame: NSRect,
    screen: Option<&NSScreen>,
    mtm: MainThreadMarker,
) -> Retained<NSPanel> {
    // Keep AppKit's standard resize frame with a transparent, full-size
    // titlebar. Do not attach Tao's delegate: it assumes all notifications refer to the original NSWindow.
    let allocated = OnlineCaptionPanel::alloc(mtm).set_ivars(PanelIvars::default());
    let panel: Retained<OnlineCaptionPanel> = unsafe {
        msg_send![super(allocated),
            initWithContentRect: frame,
            styleMask: NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable
                | NSWindowStyleMask::FullSizeContentView | NSWindowStyleMask::NonactivatingPanel,
            backing: NSBackingStoreType::Buffered,
            defer: false]
    };
    panel.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    panel.setTitlebarAppearsTransparent(true);
    panel.setFrame_display(frame, false);
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setAlphaValue(1.0); // Only CSS background is translucent; text stays opaque.
    panel.setHasShadow(false);
    if let Some(screen) = screen {
        let visible = screen.visibleFrame();
        let width = frame.size.width.min(visible.size.width - 32.0);
        panel.setFrame_display(
            NSRect::new(
                NSPoint::new(
                    visible.origin.x + (visible.size.width - width) / 2.0,
                    visible.origin.y + 24.0,
                ),
                NSSize::new(width, frame.size.height),
            ),
            false,
        );
    }
    panel.into_super()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertical_edges_keep_the_opposite_edge_and_width_fixed() {
        let frame = NSRect::new(NSPoint::new(30.0, 100.0), NSSize::new(800.0, 280.0));
        let top = height_frame(frame, 60.0, true, 200.0, 800.0);
        assert_eq!(top.origin, frame.origin);
        assert_eq!(top.size, NSSize::new(800.0, 340.0));
        let bottom = height_frame(frame, 40.0, false, 200.0, 800.0);
        assert_eq!(bottom.origin.y + bottom.size.height, 380.0);
        assert_eq!(bottom.size, NSSize::new(800.0, 240.0));
        assert_eq!(
            height_frame(frame, -500.0, true, 200.0, 800.0).size.height,
            200.0
        );
        assert_eq!(
            height_frame(frame, 1000.0, true, 200.0, 800.0).size.height,
            800.0
        );
    }
}
