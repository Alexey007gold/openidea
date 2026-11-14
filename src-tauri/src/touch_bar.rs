use serde::Deserialize;
use specta::Type;

pub const TOUCH_BAR_EVENT: &str = "openidea://touch-bar";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TouchBarIcon {
    PreviousDifference,
    NextDifference,
    PreviousFile,
    NextFile,
    StepOver,
    StepInto,
    StepOut,
    Resume,
    Stop,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TouchBarButton {
    pub action: String,
    pub title: String,
    pub icon: TouchBarIcon,
    pub show_title: bool,
    pub enabled: bool,
}

#[cfg(any(target_os = "macos", test))]
fn same_layout(current: &[TouchBarButton], next: &[TouchBarButton]) -> bool {
    current.len() == next.len()
        && current.iter().zip(next).all(|(a, b)| {
            a.action == b.action && a.title == b.title && a.icon == b.icon && a.show_title == b.show_title
        })
}

#[cfg(target_os = "macos")]
pub use native::{forget, set};

#[cfg(target_os = "macos")]
mod native {
    use super::{TOUCH_BAR_EVENT, TouchBarButton, TouchBarIcon, same_layout};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{NSButtonTouchBarItem, NSImage, NSTouchBar, NSTouchBarItem, NSView};
    use objc2_foundation::{NSArray, NSSet, NSString};
    use std::{cell::RefCell, collections::HashMap};
    use tauri::{Emitter, WebviewWindow};

    impl TouchBarIcon {
        fn symbol(self) -> &'static str {
            match self {
                Self::PreviousDifference => "arrow.up",
                Self::NextDifference => "arrow.down",
                Self::PreviousFile => "arrow.left",
                Self::NextFile => "arrow.right",
                Self::StepOver => "arrow.turn.down.right",
                Self::StepInto => "arrow.down.to.line",
                Self::StepOut => "arrow.up.to.line",
                Self::Resume => "play.fill",
                Self::Stop => "stop.fill",
            }
        }
    }

    struct Press {
        window: WebviewWindow,
        action: String,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "OpenIdeaTouchBarPress"]
        #[ivars = Press]
        struct Target;

        unsafe impl NSObjectProtocol for Target {}

        impl Target {
            #[unsafe(method(press:))]
            fn press(&self, _sender: Option<&AnyObject>) {
                let Press { window, action } = self.ivars();
                if let Err(e) = window.emit_to(window.label(), TOUCH_BAR_EVENT, action) {
                    tracing::warn!("failed to deliver touch bar action {action}: {e}");
                }
            }
        }
    );

    impl Target {
        fn new(press: Press, mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(press);
            unsafe { msg_send![super(this), init] }
        }
    }

    struct Bar {
        buttons: Vec<TouchBarButton>,
        touch_bar: Retained<NSTouchBar>,
        items: Vec<Retained<NSButtonTouchBarItem>>,
        _targets: Vec<Retained<Target>>,
    }

    thread_local! {
        static BARS: RefCell<HashMap<String, Bar>> = RefCell::default();
    }

    pub fn set(window: &WebviewWindow, buttons: Vec<TouchBarButton>) -> tauri::Result<()> {
        let owner = window.clone();
        window.with_webview(move |webview| {
            let Some(mtm) = MainThreadMarker::new() else { return };
            let view = unsafe { &*webview.inner().cast::<NSView>() };
            let label = owner.label();
            BARS.with_borrow_mut(|bars| {
                if buttons.is_empty() {
                    if bars.remove(label).is_some() {
                        view.setTouchBar(None);
                    }
                    return;
                }
                if let Some(bar) = bars.get_mut(label).filter(|bar| same_layout(&bar.buttons, &buttons)) {
                    for (item, button) in bar.items.iter().zip(&buttons) {
                        item.setEnabled(button.enabled);
                    }
                    bar.buttons = buttons;
                    return;
                }
                let bar = build(&owner, buttons, mtm);
                view.setTouchBar(Some(&bar.touch_bar));
                bars.insert(label.to_owned(), bar);
            });
        })
    }

    pub fn forget(label: &str) {
        BARS.with_borrow_mut(|bars| bars.remove(label));
    }

    fn item(button: &TouchBarButton, receiver: &AnyObject, mtm: MainThreadMarker) -> Retained<NSButtonTouchBarItem> {
        let identifier = NSString::from_str(&button.action);
        let title = NSString::from_str(&button.title);
        let symbol = NSString::from_str(button.icon.symbol());
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol, Some(&title));
        let target = Some(receiver);
        let action = Some(sel!(press:));
        let item = unsafe {
            match image {
                Some(image) if button.show_title => {
                    NSButtonTouchBarItem::buttonTouchBarItemWithIdentifier_title_image_target_action(
                        &identifier,
                        &title,
                        &image,
                        target,
                        action,
                        mtm,
                    )
                }
                Some(image) => NSButtonTouchBarItem::buttonTouchBarItemWithIdentifier_image_target_action(
                    &identifier,
                    &image,
                    target,
                    action,
                    mtm,
                ),
                None => NSButtonTouchBarItem::buttonTouchBarItemWithIdentifier_title_target_action(
                    &identifier,
                    &title,
                    target,
                    action,
                    mtm,
                ),
            }
        };
        item.setEnabled(button.enabled);
        item.setCustomizationLabel(Some(&title));
        item
    }

    fn build(window: &WebviewWindow, buttons: Vec<TouchBarButton>, mtm: MainThreadMarker) -> Bar {
        let mut items = Vec::with_capacity(buttons.len());
        let mut targets = Vec::with_capacity(buttons.len());
        for button in &buttons {
            let target = Target::new(Press { window: window.clone(), action: button.action.clone() }, mtm);
            items.push(item(button, &target, mtm));
            targets.push(target);
        }
        let touch_bar = NSTouchBar::new(mtm);
        let templates: Vec<&NSTouchBarItem> = items.iter().map(|item| &***item).collect();
        touch_bar.setTemplateItems(&NSSet::from_slice(&templates));
        let identifiers: Vec<Retained<NSString>> = items.iter().map(|item| item.identifier()).collect();
        touch_bar.setDefaultItemIdentifiers(&NSArray::from_retained_slice(&identifiers));
        Bar { buttons, touch_bar, items, _targets: targets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(action: &str, enabled: bool) -> TouchBarButton {
        TouchBarButton {
            action: action.into(),
            title: action.into(),
            icon: TouchBarIcon::StepOver,
            show_title: true,
            enabled,
        }
    }

    #[test]
    fn enablement_alone_keeps_the_layout() {
        assert!(same_layout(&[button("debug.stepOver", true)], &[button("debug.stepOver", false)]));
    }

    #[test]
    fn other_buttons_change_the_layout() {
        assert!(!same_layout(&[button("debug.stepOver", true)], &[button("debug.stepInto", true)]));
        assert!(!same_layout(&[button("debug.stepOver", true)], &[]));
    }
}
