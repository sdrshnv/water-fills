use dioxus::prelude::*;
use gloo_events::EventListener;
use gloo_net::http::{Request, RequestBuilder};
use gloo_storage::{LocalStorage, Storage};
use shared::view::ViewModel;
use shared::{FillRequest, State, BOTTLE_SIZES};
use wasm_bindgen_futures::spawn_local;

const KEY_STORAGE: &str = "water-fills-api-key";

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, Copy)]
enum Action {
    Load,
    Fill(u32),
    Pay,
    Undo,
}

enum ApiError {
    Unauthorized,
    /// The server refused the action because the state had changed (e.g. on another device).
    Conflict(String),
    Other(String),
}

async fn call(action: Action, key: &str) -> Result<State, ApiError> {
    let auth = format!("Bearer {key}");
    let other = |e: &dyn std::fmt::Display| ApiError::Other(e.to_string());

    let builder: RequestBuilder = match action {
        Action::Load => Request::get("/api/state"),
        Action::Fill(_) => Request::post("/api/fill"),
        Action::Pay => Request::post("/api/pay"),
        Action::Undo => Request::post("/api/undo"),
    }
    .header("Authorization", &auth);

    let request = match action {
        Action::Fill(oz) => builder.json(&FillRequest { oz }).map_err(|e| other(&e))?,
        _ => builder.build().map_err(|e| other(&e))?,
    };
    let resp = request.send().await.map_err(|e| other(&e))?;

    match resp.status() {
        200 => resp.json::<State>().await.map_err(|e| other(&e)),
        401 => Err(ApiError::Unauthorized),
        409 => Err(ApiError::Conflict(
            resp.text().await.unwrap_or_else(|_| "Out of date".into()),
        )),
        s => Err(ApiError::Other(format!("Server error ({s})"))),
    }
}

#[component]
fn App() -> Element {
    let mut api_key = use_signal(|| LocalStorage::get::<String>(KEY_STORAGE).ok());
    let mut state = use_signal(|| None::<State>);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    // Runs one API action, keeping the UI state in sync with the server's answer.
    let mut act = move |action: Action| {
        let Some(key) = api_key() else { return };
        busy.set(true);
        spawn_local(async move {
            match call(action, &key).await {
                Ok(s) => {
                    state.set(Some(s));
                    error.set(None);
                }
                Err(ApiError::Unauthorized) => {
                    LocalStorage::delete(KEY_STORAGE);
                    api_key.set(None);
                    error.set(Some("That key was rejected. Enter it again.".into()));
                }
                Err(ApiError::Conflict(msg)) => {
                    error.set(Some(msg));
                    if let Ok(s) = call(Action::Load, &key).await {
                        state.set(Some(s));
                    }
                }
                Err(ApiError::Other(msg)) => error.set(Some(msg)),
            }
            busy.set(false);
        });
    };

    // Load whenever a key becomes available.
    use_effect(move || {
        if api_key().is_some() {
            act(Action::Load);
        }
    });

    // Refresh when the tab becomes visible again (e.g. reopening the phone browser).
    use_hook(move || {
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            let doc2 = doc.clone();
            EventListener::new(&doc, "visibilitychange", move |_| {
                if !doc2.hidden() {
                    act(Action::Load);
                }
            })
            .forget();
        }
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/main.css") }
        main {
            h1 { "Water fills" }
            if api_key().is_none() {
                KeyPrompt { error: error(), on_save: move |k: String| {
                    let _ = LocalStorage::set(KEY_STORAGE, &k);
                    error.set(None);
                    api_key.set(Some(k));
                } }
            } else {
                match state() {
                    None => rsx! { p { class: "status", if error().is_some() { "Couldn't load." } else { "Loading…" } } },
                    Some(s) => {
                        let vm = ViewModel::from(&s);
                        let busy = busy();
                        rsx! {
                            div { class: "fills",
                                for oz in BOTTLE_SIZES {
                                    button {
                                        class: "fill",
                                        disabled: busy,
                                        onclick: move |_| act(Action::Fill(oz)),
                                        "{oz} oz"
                                    }
                                }
                            }
                            section { class: "total",
                                div { class: "oz", "{vm.oz_text}" }
                                if let Some(owed) = vm.owed_text.clone() {
                                    div { class: "owed", "{owed}" }
                                }
                            }
                            button {
                                class: "pay",
                                disabled: busy || !vm.pay_enabled,
                                onclick: move |_| act(Action::Pay),
                                "Pay!"
                            }
                            button {
                                class: "undo",
                                disabled: busy || !vm.undo_enabled,
                                onclick: move |_| act(Action::Undo),
                                "{vm.undo_label}"
                            }
                        }
                    }
                }
                if let Some(msg) = error() {
                    p { class: "error", role: "alert", "{msg}" }
                }
            }
        }
    }
}

#[component]
fn KeyPrompt(error: Option<String>, on_save: EventHandler<String>) -> Element {
    let mut value = use_signal(String::new);
    rsx! {
        form {
            onsubmit: move |e| {
                e.prevent_default();
                let k = value().trim().to_string();
                if !k.is_empty() {
                    on_save.call(k);
                }
            },
            label { r#for: "key", "Access key" }
            input {
                id: "key",
                r#type: "password",
                autocomplete: "current-password",
                value: "{value}",
                oninput: move |e| value.set(e.value()),
            }
            button { class: "fill", r#type: "submit", "Save" }
            if let Some(msg) = error {
                p { class: "error", role: "alert", "{msg}" }
            }
        }
    }
}
