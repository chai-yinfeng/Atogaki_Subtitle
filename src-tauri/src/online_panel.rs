//! The live caption panel owns its drag events; its WebView's Tauri window is hidden.
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSEventType, NSPanel, NSScreen, NSWindowStyleMask,
    NSWindowTitleVisibility,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

define_class!(
    #[unsafe(super(NSPanel))]
    #[name = "AtogakiOnlineCaptionPanel"]
    #[thread_kind = MainThreadOnly]
    struct OnlineCaptionPanel;

    impl OnlineCaptionPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool { true }

        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            let point = event.locationInWindow();
            let size = self.frame().size;
            // Matches the reserved 32px header in online.css. Leave the
            // native resize edges and the right-hand hide button untouched.
            if event.r#type() == NSEventType::LeftMouseDown
                && point.y >= size.height - 42.0 && point.y <= size.height - 6.0
                && point.x >= 16.0 && point.x < size.width - 84.0
            {
                self.performWindowDragWithEvent(event);
                return;
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
    let panel: Retained<OnlineCaptionPanel> = unsafe {
        msg_send![OnlineCaptionPanel::alloc(mtm),
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
