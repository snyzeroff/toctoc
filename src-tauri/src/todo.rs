//! Listes de choses à faire partagées.
//!
//! Chaque élément porte l'heure de sa dernière modification (`ts`). Les deux ordinateurs
//! s'envoient la liste entière après chaque changement ; à la réception, on garde, élément par
//! élément, la version la plus récente. Les éléments supprimés restent sous forme de « pierre
//! tombale » (`deleted`) pour que la suppression gagne aussi chez l'autre.
use serde::{Deserialize, Serialize};

const MAX_ITEMS: usize = 300;
const MAX_TEXT: usize = 500;
const MAX_DEPTH: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TodoItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub text: String,
    /// vrai = case à cocher ; faux = simple point
    #[serde(default = "yes")]
    pub task: bool,
    #[serde(default)]
    pub done: bool,
    /// place parmi ses frères et sœurs
    #[serde(default)]
    pub order: f64,
    /// dernière modification (ms)
    #[serde(default)]
    pub ts: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TodoList {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub items: Vec<TodoItem>,
}

/// Ce que l'interface demande de changer.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum TodoOp {
    Toggle { item: String },
    Add { parent: Option<String>, text: String, task: bool },
    Remove { item: String },
    Rename { item: String, text: String },
    /// case à cocher ↔ simple point
    Kind { item: String },
}

fn clean_text(s: &str) -> String {
    s.trim().chars().filter(|c| !c.is_control() || *c == '\t').take(MAX_TEXT).collect()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

impl TodoList {
    /// Nettoie une liste venue de l'interface ou de l'autre ordinateur.
    pub fn sanitize(&mut self) {
        self.title = self.title.as_deref().map(clean_text).filter(|t| !t.is_empty());
        self.items.retain(|i| valid_id(&i.id));
        self.items.truncate(MAX_ITEMS * 2);
        let ids: Vec<String> = self.items.iter().map(|i| i.id.clone()).collect();
        for i in self.items.iter_mut() {
            i.text = clean_text(&i.text);
            if !i.order.is_finite() {
                i.order = 0.0;
            }
            if i.parent.as_deref().map(|p| p == i.id || !ids.iter().any(|x| x == p)).unwrap_or(false) {
                i.parent = None;
            }
            if !i.task {
                i.done = false;
            }
        }
        // pas de boucle ni de profondeur excessive
        for idx in 0..self.items.len() {
            if self.depth(&self.items[idx].id) > MAX_DEPTH {
                self.items[idx].parent = None;
            }
        }
    }

    fn get(&self, id: &str) -> Option<&TodoItem> {
        self.items.iter().find(|i| i.id == id)
    }

    fn depth(&self, id: &str) -> usize {
        let mut d = 0;
        let mut cur = self.get(id).and_then(|i| i.parent.clone());
        while let Some(p) = cur {
            d += 1;
            if d > MAX_DEPTH + 1 {
                break;
            }
            cur = self.get(&p).and_then(|i| i.parent.clone());
        }
        d
    }

    /// Fusionne la liste de l'autre ordinateur. Renvoie vrai si quelque chose a changé ici.
    pub fn merge(&mut self, other: &TodoList) -> bool {
        let mut changed = false;
        for it in &other.items {
            let len = self.items.len();
            match self.items.iter_mut().find(|l| l.id == it.id) {
                Some(l) => {
                    if it.ts > l.ts || (it.ts == l.ts && it != l && it.deleted) {
                        *l = it.clone();
                        changed = true;
                    }
                }
                None if len < MAX_ITEMS * 2 => {
                    self.items.push(it.clone());
                    changed = true;
                }
                None => {}
            }
        }
        if self.title.is_none() && other.title.is_some() {
            self.title = other.title.clone();
            changed = true;
        }
        if changed {
            self.sanitize();
        }
        changed
    }

    fn stamp(prev: u64, now: u64) -> u64 {
        now.max(prev + 1)
    }

    /// Applique un changement demandé ici. Renvoie vrai s'il a eu lieu.
    pub fn apply(&mut self, op: TodoOp, now: u64, new_id: String) -> bool {
        match op {
            TodoOp::Toggle { item } => match self.items.iter_mut().find(|i| i.id == item && !i.deleted) {
                Some(i) if i.task => {
                    i.done = !i.done;
                    i.ts = Self::stamp(i.ts, now);
                    true
                }
                _ => false,
            },
            TodoOp::Kind { item } => match self.items.iter_mut().find(|i| i.id == item && !i.deleted) {
                Some(i) => {
                    i.task = !i.task;
                    i.done = false;
                    i.ts = Self::stamp(i.ts, now);
                    true
                }
                None => false,
            },
            TodoOp::Rename { item, text } => {
                let text = clean_text(&text);
                match self.items.iter_mut().find(|i| i.id == item && !i.deleted) {
                    Some(i) if !text.is_empty() && i.text != text => {
                        i.text = text;
                        i.ts = Self::stamp(i.ts, now);
                        true
                    }
                    _ => false,
                }
            }
            TodoOp::Add { parent, text, task } => {
                let text = clean_text(&text);
                let live = self.items.iter().filter(|i| !i.deleted).count();
                if text.is_empty() || live >= MAX_ITEMS || self.items.len() >= MAX_ITEMS * 2 {
                    return false;
                }
                let parent = parent.filter(|p| self.get(p).map(|i| !i.deleted).unwrap_or(false));
                if let Some(p) = &parent {
                    if self.depth(p) + 1 > MAX_DEPTH {
                        return false;
                    }
                }
                let order = self
                    .items
                    .iter()
                    .filter(|i| i.parent == parent)
                    .map(|i| i.order)
                    .fold(0.0, f64::max)
                    + 1.0;
                self.items.push(TodoItem { id: new_id, parent, text, task, done: false, order, ts: now, deleted: false });
                true
            }
            TodoOp::Remove { item } => {
                // l'élément et tout ce qui est dessous
                let mut gone = vec![item];
                let mut i = 0;
                while i < gone.len() {
                    let kids: Vec<String> = self
                        .items
                        .iter()
                        .filter(|x| x.parent.as_deref() == Some(gone[i].as_str()))
                        .map(|x| x.id.clone())
                        .collect();
                    gone.extend(kids);
                    i += 1;
                }
                let mut changed = false;
                for it in self.items.iter_mut().filter(|x| gone.contains(&x.id) && !x.deleted) {
                    it.deleted = true;
                    it.ts = Self::stamp(it.ts, now);
                    changed = true;
                }
                changed
            }
        }
    }

    /// Texte simple de la liste (aperçu de notification, copie).
    pub fn plain(&self) -> String {
        let mut out = String::new();
        if let Some(t) = &self.title {
            out.push_str(t);
            out.push('\n');
        }
        self.plain_level(None, 0, &mut out);
        out.trim_end().to_string()
    }

    fn plain_level(&self, parent: Option<&str>, depth: usize, out: &mut String) {
        let mut kids: Vec<&TodoItem> =
            self.items.iter().filter(|i| !i.deleted && i.parent.as_deref() == parent).collect();
        kids.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap_or(std::cmp::Ordering::Equal));
        for k in kids {
            let mark = if !k.task { "•" } else if k.done { "☑" } else { "☐" };
            out.push_str(&format!("{}{} {}\n", "  ".repeat(depth), mark, k.text));
            if depth < MAX_DEPTH + 1 {
                self.plain_level(Some(&k.id), depth + 1, out);
            }
        }
    }

    #[cfg(test)]
    pub fn counts(&self) -> (usize, usize) {
        let tasks = self.items.iter().filter(|i| !i.deleted && i.task);
        let (mut done, mut all) = (0, 0);
        for t in tasks {
            all += 1;
            if t.done {
                done += 1;
            }
        }
        (done, all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, parent: Option<&str>, text: &str, ts: u64) -> TodoItem {
        TodoItem { id: id.into(), parent: parent.map(Into::into), text: text.into(), task: true, done: false, order: 1.0, ts, deleted: false }
    }

    fn list() -> TodoList {
        TodoList {
            title: Some("Courses".into()),
            items: vec![item("a", None, "Pain", 1), item("b", None, "Fruits", 1), item("c", Some("b"), "Pommes", 1)],
        }
    }

    #[test]
    fn ops_and_merge_converge() {
        let mut mine = list();
        let mut theirs = list();
        // en même temps : je coche « Pain », l'autre ajoute « Poires » sous « Fruits »
        assert!(mine.apply(TodoOp::Toggle { item: "a".into() }, 10, "x".into()));
        assert!(theirs.apply(TodoOp::Add { parent: Some("b".into()), text: "Poires".into(), task: true }, 11, "d".into()));
        let (m0, t0) = (mine.clone(), theirs.clone());
        mine.merge(&t0);
        theirs.merge(&m0);
        assert_eq!(mine.plain(), theirs.plain());
        assert!(mine.plain().contains("☑ Pain"));
        assert!(mine.plain().contains("\n  ☐ Poires"));
        // une suppression emporte les sous-éléments, des deux côtés
        assert!(mine.apply(TodoOp::Remove { item: "b".into() }, 20, "y".into()));
        theirs.merge(&mine);
        assert!(!theirs.plain().contains("Pommes"));
        assert_eq!(theirs.counts(), (1, 1));
        // une vieille version ne revient pas en arrière
        assert!(!theirs.merge(&list()));
        assert!(theirs.plain().contains("☑ Pain"));
    }

    #[test]
    fn sanitize_rejects_loops() {
        let mut l = TodoList {
            title: None,
            items: vec![item("a", Some("b"), "A", 1), item("b", Some("a"), "B", 1), item("../x", None, "bad", 1)],
        };
        l.sanitize();
        assert_eq!(l.items.len(), 2);
        assert!(l.items.iter().any(|i| i.parent.is_none()));
    }
}
