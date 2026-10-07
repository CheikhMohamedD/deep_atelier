//! Outils partagés des tests d'intégration.
#![allow(dead_code)]

use ir::command::{Command, NodeSpec};
use ir::{Applied, Document, IdGen, NodeId, NodeRef, Origin, Scope, Session, Transaction};

pub fn session() -> Session {
    let doc = Document::new("Test", &mut IdGen::from_seed(11));
    Session::new(doc, 99)
}

pub fn home_root(session: &Session) -> NodeId {
    session.document().pages[0].root.clone()
}

pub fn json<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("valid JSON fixture")
}

pub fn spec(value: serde_json::Value) -> NodeSpec {
    json(value)
}

pub fn run(session: &mut Session, commands: Vec<Command>) -> Applied {
    session
        .apply(Transaction::user("test", commands))
        .expect("transaction applies")
}

pub fn run_json(session: &mut Session, commands: serde_json::Value) -> Applied {
    run(session, json(commands))
}

pub fn try_run(
    session: &mut Session,
    origin: Origin,
    scope: Scope,
    commands: serde_json::Value,
) -> Result<Applied, ir::CommandError> {
    session.apply(Transaction {
        label: "test".into(),
        origin,
        scope,
        commands: json(commands),
    })
}

pub fn r(id: &NodeId) -> NodeRef {
    NodeRef::from(id)
}

pub fn ai() -> Origin {
    Origin::Ai { run_id: "run-1".into() }
}
