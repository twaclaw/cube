//! Thin wrappers over the DOM calls this app needs.

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, EventTarget, HtmlInputElement, HtmlSelectElement, Window};

pub fn window() -> Window {
    web_sys::window().expect("a browser window")
}

pub fn document() -> Document {
    window().document().expect("a document")
}

pub fn by_id(id: &str) -> Element {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("index.html is missing #{id}"))
}

pub fn input(id: &str) -> HtmlInputElement {
    by_id(id).unchecked_into()
}

pub fn select(id: &str) -> HtmlSelectElement {
    by_id(id).unchecked_into()
}

pub fn set_html(id: &str, html: &str) {
    by_id(id).set_inner_html(html);
}

pub fn set_text(id: &str, text: &str) {
    by_id(id).set_text_content(Some(text));
}

/// Add a listener and let it live as long as the page.
pub fn on<E, F>(target: &EventTarget, event: &str, mut handler: F)
where
    E: JsCast + 'static,
    F: FnMut(E) + 'static,
{
    let closure = Closure::wrap(Box::new(move |e: web_sys::Event| {
        handler(e.unchecked_into::<E>());
    }) as Box<dyn FnMut(web_sys::Event)>);
    target
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .expect("the listener attaches");
    closure.forget();
}

pub fn on_id<E, F>(id: &str, event: &str, handler: F)
where
    E: JsCast + 'static,
    F: FnMut(E) + 'static,
{
    on(by_id(id).unchecked_ref::<EventTarget>(), event, handler);
}

pub fn request_frame(f: &Closure<dyn FnMut(f64)>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("the frame is scheduled");
}

/// Milliseconds since the page loaded.
pub fn now() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or_default()
}

/// Run `f` after the browser has had a chance to paint.
pub fn defer(ms: i32, f: impl FnOnce() + 'static) {
    let closure = Closure::once_into_js(f);
    let _ =
        window().set_timeout_with_callback_and_timeout_and_arguments_0(closure.unchecked_ref(), ms);
}

/// Offer `data` to the browser as a file download.
pub fn download(name: &str, mime: &str, data: &str) {
    let parts = js_sys::Array::of1(&JsValue::from_str(data));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)
        .expect("the blob is built");
    let url = web_sys::Url::create_object_url_with_blob(&blob).expect("an object url");
    let a: web_sys::HtmlAnchorElement = document()
        .create_element("a")
        .expect("an anchor")
        .unchecked_into();
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
}

pub fn copy_to_clipboard(text: &str) {
    let _ = window().navigator().clipboard().write_text(text);
}

/// The page's query string, as a lookup.
pub fn params() -> web_sys::UrlSearchParams {
    let search = window().location().search().unwrap_or_default();
    web_sys::UrlSearchParams::new_with_str(&search)
        .unwrap_or_else(|_| web_sys::UrlSearchParams::new().expect("empty search params"))
}

pub fn page_url() -> String {
    let location = window().location();
    let origin = location.origin().unwrap_or_default();
    let path = location.pathname().unwrap_or_default();
    format!("{origin}{path}")
}
