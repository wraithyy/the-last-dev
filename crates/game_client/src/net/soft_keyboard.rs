//! Web only: a hidden HTML input that summons the soft keyboard on touch devices and gives
//! desktop browsers paste for free. While a lobby field is focused it is the text source.

use std::cell::{Cell, OnceCell};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{HtmlElement, HtmlInputElement};

struct Kb {
    input: HtmlInputElement,
    /// Read by the tap handler: iOS only opens the keyboard from inside a user gesture.
    want: Rc<Cell<bool>>,
    _tap: Closure<dyn FnMut(web_sys::PointerEvent)>,
}

thread_local! {
    static KB: OnceCell<Option<Kb>> = const { OnceCell::new() };
}

fn build() -> Option<Kb> {
    let doc = web_sys::window()?.document()?;
    let input: HtmlInputElement = doc.create_element("input").ok()?.dyn_into().ok()?;
    // 16 px font: iOS zooms the page for anything smaller.
    input
        .set_attribute(
            "style",
            "position:fixed;left:0;bottom:0;width:1px;height:1px;opacity:0;border:0;padding:0;font-size:16px;",
        )
        .ok()?;
    for (k, v) in [
        ("autocomplete", "off"),
        ("autocapitalize", "off"),
        ("spellcheck", "false"),
        ("aria-label", "Text field"),
    ] {
        input.set_attribute(k, v).ok()?;
    }
    doc.body()?.append_child(&input).ok()?;
    let want = Rc::new(Cell::new(false));
    let tap = {
        let (input, want) = (input.clone(), want.clone());
        // Touch only: refocusing on a mouse release blurs the canvas mid-click, Bevy drops the
        // pressed button on focus loss and the click on another field or button is lost.
        Closure::<dyn FnMut(web_sys::PointerEvent)>::new(move |e: web_sys::PointerEvent| {
            if want.get() && e.pointer_type() == "touch" {
                let _ = input.focus();
            }
        })
    };
    doc.add_event_listener_with_callback("pointerup", tap.as_ref().unchecked_ref())
        .ok()?;
    Some(Kb {
        input,
        want,
        _tap: tap,
    })
}

fn with<R>(f: impl FnOnce(&Kb) -> R) -> Option<R> {
    KB.with(|cell| cell.get_or_init(build).as_ref().map(f))
}

pub fn show(value: &str) {
    with(|k| {
        k.input.set_value(value);
        k.want.set(true);
        let _ = k.input.focus();
    });
}

pub fn value() -> Option<String> {
    with(|k| k.input.value())
}

pub fn set_value(value: &str) {
    with(|k| k.input.set_value(value));
}

/// Hands keyboard focus back to the game canvas, which needs it for Esc and gameplay keys.
pub fn hide() {
    with(|k| {
        k.want.set(false);
        let _ = k.input.blur();
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("bevy"))
            .and_then(|e| e.dyn_into::<HtmlElement>().ok());
        if let Some(c) = canvas {
            let _ = c.focus();
        }
    });
}
