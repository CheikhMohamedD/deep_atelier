//! Moteur de l'éditeur, compilé en WebAssembly (ADR 0001 § 2 et § 8) : la session de l'IR
//! (commandes, undo/redo, gestes), le rendu des pages pour le canvas et la validation.
//!
//! [`Core`] porte toute la logique, en Rust ordinaire (testé nativement) ; [`Engine`] l'expose à
//! JavaScript avec `wasm-bindgen`. Les échanges se font en JSON, typés côté TypeScript par
//! `@deep-atelier/ir-types`.

use compiler_web::canvas::canvas_page;
use ir::migrate::migrate;
use ir::{Document, IdGen, PageId, Session, Transaction};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

/// Erreur d'ouverture ou de requête, avec un code stable (traduit par l'éditeur).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
}

impl EngineError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn to_json(&self) -> Value {
        json!({ "ok": false, "error": { "code": self.code, "message": self.message } })
    }
}

/// Graine du générateur d'identifiants : 8 octets aléatoires fournis par l'hôte.
pub fn seed_from_bytes(bytes: &[u8]) -> Result<u64, EngineError> {
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| EngineError::new("INVALID_SEED", "the seed must be 8 bytes"))?;
    Ok(u64::from_le_bytes(bytes))
}

/// Session d'édition d'un document.
#[derive(Debug, Clone)]
pub struct Core {
    session: Session,
}

impl Core {
    /// Ouvre un document sérialisé (migré vers la version courante de l'IR).
    pub fn open(document: &str, seed: u64) -> Result<Self, EngineError> {
        let value: Value =
            serde_json::from_str(document).map_err(|e| EngineError::new("INVALID_JSON", e.to_string()))?;
        let doc = migrate(value).map_err(|e| EngineError::new("INVALID_DOCUMENT", e.to_string()))?;
        Ok(Self {
            session: Session::new(doc, seed),
        })
    }

    /// Document neuf : une page d'accueil vide, tokens par défaut.
    pub fn blank(name: &str, seed: u64) -> Self {
        let mut ids = IdGen::from_seed(seed);
        let doc = Document::new(name, &mut ids);
        Self {
            session: Session::new(doc, seed.rotate_left(32) ^ 0x5eed),
        }
    }

    pub fn document(&self) -> &Document {
        self.session.document()
    }

    pub fn document_json(&self) -> String {
        serde_json::to_string(self.session.document()).unwrap_or_else(|_| "null".to_owned())
    }

    /// Applique une transaction : `{ ok: true, applied }` ou `{ ok: false, error }` ; rien n'est
    /// appliqué en cas d'erreur.
    pub fn apply_json(&mut self, transaction: &str) -> String {
        let result = match serde_json::from_str::<Transaction>(transaction) {
            Err(e) => EngineError::new("INVALID_TRANSACTION", e.to_string()).to_json(),
            Ok(tx) => match self.session.apply(tx) {
                Ok(applied) => json!({ "ok": true, "applied": applied }),
                Err(error) => error.to_tool_result(),
            },
        };
        result.to_string()
    }

    /// Annule : `{ ok: true, changes }` (`changes` vaut `null` s'il n'y a rien à annuler).
    pub fn undo_json(&mut self) -> String {
        match self.session.undo() {
            Ok(changes) => json!({ "ok": true, "changes": changes }),
            Err(error) => error.to_tool_result(),
        }
        .to_string()
    }

    pub fn redo_json(&mut self) -> String {
        match self.session.redo() {
            Ok(changes) => json!({ "ok": true, "changes": changes }),
            Err(error) => error.to_tool_result(),
        }
        .to_string()
    }

    pub fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.can_redo()
    }

    /// Ouvre un geste (curseur, glisser) : ses transactions forment une seule entrée d'undo.
    pub fn begin_gesture(&mut self, label: &str) {
        self.session.begin_gesture(label);
    }

    pub fn end_gesture(&mut self) {
        self.session.end_gesture();
    }

    pub fn validate_json(&self) -> String {
        serde_json::to_string(&self.session.validate()).unwrap_or_else(|_| "[]".to_owned())
    }

    /// Rendu d'une page pour le canvas (`null` si la page n'existe pas).
    pub fn canvas_page_json(&self, page: &str) -> String {
        let page = page.parse::<PageId>().ok();
        let rendered = page.and_then(|page| canvas_page(self.session.document(), &page));
        serde_json::to_string(&rendered).unwrap_or_else(|_| "null".to_owned())
    }
}

/// Moteur exposé à JavaScript.
#[wasm_bindgen]
pub struct Engine {
    core: Core,
}

#[wasm_bindgen]
impl Engine {
    /// Ouvre un document JSON ; `seed` : 8 octets aléatoires (`crypto.getRandomValues`).
    #[wasm_bindgen(constructor)]
    pub fn new(document: &str, seed: &[u8]) -> Result<Engine, JsError> {
        let seed = seed_from_bytes(seed).map_err(to_js)?;
        Core::open(document, seed).map(|core| Engine { core }).map_err(to_js)
    }

    /// Document neuf, sans passer par l'API (tests, démonstrations).
    pub fn blank(name: &str, seed: &[u8]) -> Result<Engine, JsError> {
        let seed = seed_from_bytes(seed).map_err(to_js)?;
        Ok(Engine {
            core: Core::blank(name, seed),
        })
    }

    pub fn document(&self) -> String {
        self.core.document_json()
    }

    pub fn apply(&mut self, transaction: &str) -> String {
        self.core.apply_json(transaction)
    }

    pub fn undo(&mut self) -> String {
        self.core.undo_json()
    }

    pub fn redo(&mut self) -> String {
        self.core.redo_json()
    }

    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.core.can_undo()
    }

    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.core.can_redo()
    }

    #[wasm_bindgen(js_name = beginGesture)]
    pub fn begin_gesture(&mut self, label: &str) {
        self.core.begin_gesture(label);
    }

    #[wasm_bindgen(js_name = endGesture)]
    pub fn end_gesture(&mut self) {
        self.core.end_gesture();
    }

    pub fn validate(&self) -> String {
        self.core.validate_json()
    }

    #[wasm_bindgen(js_name = canvasPage)]
    pub fn canvas_page(&self, page: &str) -> String {
        self.core.canvas_page_json(page)
    }
}

fn to_js(error: EngineError) -> JsError {
    JsError::new(&format!("{}: {}", error.code, error.message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn landing() -> Core {
        let json = serde_json::to_string(&compiler_web::demo::landing()).expect("JSON");
        Core::open(&json, 7).expect("opens")
    }

    fn parse(text: &str) -> Value {
        serde_json::from_str(text).expect("JSON")
    }

    #[test]
    fn opens_a_document_and_renders_its_pages() {
        let core = landing();
        let page = core.document().pages[0].id.to_string();
        let canvas = parse(&core.canvas_page_json(&page));
        assert_eq!(canvas["page"], page.as_str());
        assert!(canvas["nodes"].as_array().is_some_and(|n| !n.is_empty()));
        assert_eq!(parse(&core.canvas_page_json("p_zzzzzzzzzz")), Value::Null);
        assert_eq!(parse(&core.canvas_page_json("not an id")), Value::Null);
        // Le document sérialisé se relit à l'identique.
        let again = Core::open(&core.document_json(), 1).expect("reopens");
        assert_eq!(again.document(), core.document());
    }

    #[test]
    fn refuses_unreadable_documents_with_a_code() {
        assert_eq!(Core::open("{", 1).unwrap_err().code, "INVALID_JSON");
        assert_eq!(Core::open("{\"version\": 99}", 1).unwrap_err().code, "INVALID_DOCUMENT");
        assert_eq!(seed_from_bytes(&[1, 2, 3]).unwrap_err().code, "INVALID_SEED");
        assert_eq!(seed_from_bytes(&[1, 0, 0, 0, 0, 0, 0, 0]), Ok(1));
    }

    #[test]
    fn applies_undoes_and_redoes_transactions() {
        let mut core = Core::blank("Site", 42);
        assert!(!core.can_undo());
        let root = core.document().pages[0].root.to_string();
        let tx = json!({
            "label": "Titre", "origin": { "kind": "user" },
            "commands": [{ "op": "insert_nodes", "parent": root, "nodes": [
                { "ref": "$title", "kind": { "type": "Text", "role": { "kind": "Heading", "level": "h1" },
                  "content": [{ "text": "Bonjour" }] } }
            ] }]
        });
        let result = parse(&core.apply_json(&tx.to_string()));
        assert_eq!(result["ok"], true, "{result}");
        let created = result["applied"]["created"]["$title"]
            .as_str()
            .expect("created id")
            .to_owned();
        assert!(
            result["applied"]["changes"]["nodes"]
                .as_array()
                .is_some_and(|n| !n.is_empty())
        );
        assert!(core.can_undo());

        let page = core.document().pages[0].id.to_string();
        let canvas = core.canvas_page_json(&page);
        assert!(canvas.contains("Bonjour") && canvas.contains(&created));

        let undone = parse(&core.undo_json());
        assert_eq!(undone["ok"], true);
        assert!(!core.canvas_page_json(&page).contains("Bonjour"));
        assert!(core.can_redo());
        assert_eq!(parse(&core.redo_json())["ok"], true);
        assert!(core.canvas_page_json(&page).contains("Bonjour"));
        // Rien à annuler au-delà de l'historique.
        assert_eq!(parse(&core.undo_json())["ok"], true);
        assert_eq!(parse(&core.undo_json())["changes"], Value::Null);
    }

    #[test]
    fn errors_leave_the_document_unchanged() {
        let mut core = Core::blank("Site", 3);
        let before = core.document_json();
        let invalid = parse(&core.apply_json("{"));
        assert_eq!(invalid["error"]["code"], "INVALID_TRANSACTION");
        let tx = json!({
            "label": "Fantôme", "origin": { "kind": "user" },
            "commands": [{ "op": "delete_nodes", "nodes": ["n_zzzzzzzzzz"] }]
        });
        let refused = parse(&core.apply_json(&tx.to_string()));
        assert_eq!(refused["ok"], false);
        assert!(refused["error"]["code"].is_string());
        assert_eq!(core.document_json(), before);
        assert!(!core.can_undo());
    }

    #[test]
    fn a_gesture_is_one_undo_entry_and_validation_reports_issues() {
        let mut core = Core::blank("Site", 9);
        let root = core.document().pages[0].root.to_string();
        core.begin_gesture("Glisser");
        for _ in 0..3 {
            let tx = json!({
                "label": "Boîte", "origin": { "kind": "user" },
                "commands": [{ "op": "insert_nodes", "parent": root, "nodes": [{ "kind": { "type": "Box" } }] }]
            });
            assert_eq!(parse(&core.apply_json(&tx.to_string()))["ok"], true);
        }
        core.end_gesture();
        assert_eq!(parse(&core.undo_json())["ok"], true);
        assert!(!core.can_undo());
        assert_eq!(core.document().nodes.len(), 1);
        let issues = parse(&core.validate_json());
        // Une page vide n'a ni `main` ni `h1` : des avertissements, pas d'erreur d'intégrité.
        assert!(issues.as_array().is_some_and(|i| !i.is_empty()));
    }
}
