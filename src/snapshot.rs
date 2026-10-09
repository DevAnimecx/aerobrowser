use crate::browser::Browser;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};

/// Everything `delta_payload` needs to build one delta response, grouped so the
/// function signature stays readable.
struct DeltaInput<'a> {
    lines: &'a [String],
    elements: &'a [Value],
    /// Backend node id -> the entry emitted for it in this snapshot.
    fingerprints: &'a HashMap<i64, crate::browser::DeltaEntry>,
    previous: Option<&'a crate::browser::SnapshotDelta>,
    url: Value,
    title: Value,
    truncated: bool,
    names_truncated: bool,
    args: &'a Value,
}
impl Browser {
    pub(crate) async fn snapshot(&mut self, a: &Value) -> Result<Value> {
        self.invalidate();
        self.document = Some(self.document_id().await?);
        let root_backend = if a["selector"].is_string() {
            let object = self.element(a).await?;
            let r = self
                .call("DOM.describeNode", json!({"objectId":object}))
                .await?;
            let _ = self
                .call(
                    "Runtime.releaseObjectGroup",
                    json!({"objectGroup":"aero-action"}),
                )
                .await;
            Some(
                r["node"]["backendNodeId"]
                    .as_i64()
                    .ok_or_else(|| anyhow!("Missing node ID"))?,
            )
        } else {
            None
        };
        let tree = self.call("Accessibility.getFullAXTree", json!({})).await?;
        let nodes = tree["nodes"]
            .as_array()
            .ok_or_else(|| anyhow!("Engine has no accessibility tree"))?;
        let map: HashMap<String, &Value> = nodes
            .iter()
            .filter_map(|n| n["nodeId"].as_str().map(|id| (id.to_owned(), n)))
            .collect();
        let root = if let Some(id) = root_backend {
            nodes.iter().find(|n| n["backendDOMNodeId"] == id)
        } else {
            nodes.first()
        };
        let mut queue = VecDeque::new();
        if let Some(n) = root {
            queue.push_back((n, 0usize));
        }
        let mut seen = HashSet::new();
        let mut used_backend = HashSet::new();
        let mut elements = Vec::new();
        let mut lines = Vec::new();
        let max = a["max_nodes"].as_u64().unwrap_or(200).clamp(1, 1000) as usize;
        let depth_limit = a["max_depth"].as_u64().unwrap_or(100) as usize;
        let mut truncated = false;
        let mut names_truncated = false;
        while let Some((n, depth)) = queue.pop_front() {
            let node_id = n["nodeId"].as_str().unwrap_or("");
            if !seen.insert(node_id.to_owned()) {
                continue;
            }
            if depth > depth_limit {
                truncated = true;
                continue;
            }
            if let Some(children) = n["childIds"].as_array() {
                for id in children.iter().rev() {
                    if let Some(n) = id.as_str().and_then(|id| map.get(id)) {
                        queue.push_front((n, depth + 1));
                    }
                }
            }
            if n["ignored"] == true {
                continue;
            }
            let role = n["role"]["value"].as_str().unwrap_or("unknown");
            let focusable = n["properties"].as_array().is_some_and(|p| {
                p.iter()
                    .any(|v| v["name"] == "focusable" && v["value"]["value"] == true)
            });
            if a["interactive_only"] == true
                && !focusable
                && ![
                    "button",
                    "link",
                    "textbox",
                    "checkbox",
                    "radio",
                    "combobox",
                    "slider",
                    "spinbutton",
                    "menuitem",
                    "tab",
                    "searchbox",
                    "switch",
                ]
                .contains(&role)
            {
                continue;
            }
            if elements.len() >= max {
                truncated = true;
                break;
            }
            if let Some(id) = n["backendDOMNodeId"].as_i64() {
                let name = n["name"]["value"].as_str().unwrap_or("");
                let clipped: String = name.chars().take(240).collect();
                if clipped.len() < name.len() {
                    names_truncated = true;
                }
                let key = format!("e{}_{}", self.generation, elements.len() + 1);
                self.refs.insert(key.clone(), id);
                used_backend.insert(id);
                let indent = if a["compact"] == true || a["interactive_only"] == true {
                    String::new()
                } else {
                    "  ".repeat(depth.min(100))
                };
                lines.push(format!("{indent}{role} {} [ref={key}]", json!(clipped)));
                elements.push(json!({"ref":key,"role":role,"name":clipped,"depth":depth}));
            }
        }
        if a["cursor_interactive"] == true && elements.len() < max {
            let selector = a["selector"].as_str().unwrap_or("html");
            let script=format!("(()=>{{const root=document.querySelector({});return root?Array.from(root.querySelectorAll('*')).slice(0,5000).filter(e=>getComputedStyle(e).cursor==='pointer').slice(0,200):[]}})()",json!(selector));
            let result = self
                .call(
                    "Runtime.evaluate",
                    json!({"expression":script,"objectGroup":"aero-cursor"}),
                )
                .await?;
            if let Some(object) = result["result"]["objectId"].as_str() {
                let props = self
                    .call(
                        "Runtime.getProperties",
                        json!({"objectId":object,"ownProperties":true}),
                    )
                    .await?;
                if let Some(props) = props["result"].as_array() {
                    for p in props {
                        if p["name"]
                            .as_str()
                            .is_none_or(|s| s.parse::<usize>().is_err())
                        {
                            continue;
                        }
                        if elements.len() >= max {
                            truncated = true;
                            break;
                        }
                        if let Some(object) = p["value"]["objectId"].as_str() {
                            let n = self
                                .call("DOM.describeNode", json!({"objectId":object}))
                                .await?;
                            if let Some(id) = n["node"]["backendNodeId"].as_i64() {
                                if !used_backend.insert(id) {
                                    continue;
                                }
                                let label=self.call("Runtime.callFunctionOn",json!({"objectId":object,"functionDeclaration":"function(){return (this.getAttribute('aria-label')||this.innerText||'').slice(0,240)}","returnByValue":true})).await?;
                                let name = label["result"]["value"].as_str().unwrap_or("");
                                let key = format!("e{}_{}", self.generation, elements.len() + 1);
                                self.refs.insert(key.clone(), id);
                                lines.push(format!("cursor-target {} [ref={key}]", json!(name)));
                                elements
                                    .push(json!({"ref":key,"role":"cursor-target","name":name}));
                            }
                        }
                    }
                }
            }
            let _ = self
                .call(
                    "Runtime.releaseObjectGroup",
                    json!({"objectGroup":"aero-cursor"}),
                )
                .await;
        }
        let url = self.eval("location.href").await?;
        let title = self.eval("document.title").await?;
        let document = self.document;

        // Fingerprint each element on the fields an agent actually acts on, so a
        // rename, role change or visibility change registers but a re-render with
        // identical semantics does not. Keyed by backend node id so identity
        // survives the ref reissue that happens on every snapshot.
        let mut fingerprints: HashMap<i64, crate::browser::DeltaEntry> = HashMap::new();
        for element in &elements {
            let reference = element["ref"].as_str().unwrap_or_default().to_owned();
            let fingerprint = format!(
                "{}\u{1f}{}\u{1f}{}\u{1f}{}",
                element["role"].as_str().unwrap_or_default(),
                element["name"].as_str().unwrap_or_default(),
                element["depth"].as_u64().unwrap_or_default(),
                element["disabled"].as_bool().unwrap_or_default(),
            );
            // Match the element back to its backend node via the ref table.
            if let Some(node) = self.refs.get(&reference).copied() {
                fingerprints.insert(
                    node,
                    crate::browser::DeltaEntry {
                        reference,
                        fingerprint,
                    },
                );
            }
        }

        let previous = self.snapshot_deltas.get(&self.active).cloned();
        self.snapshot_deltas.insert(
            self.active.clone(),
            crate::browser::SnapshotDelta {
                entries: fingerprints.clone(),
                document,
                url: url.as_str().unwrap_or_default().to_owned(),
            },
        );

        if a["delta"] == true {
            let current = DeltaInput {
                lines: &lines,
                elements: &elements,
                fingerprints: &fingerprints,
                previous: previous.as_ref(),
                url,
                title,
                truncated,
                names_truncated,
                args: a,
            };
            return Ok(self.delta_payload(&current));
        }

        Ok(
            json!({"text":lines.join("\n"),"elements":elements,"truncated":truncated,"names_truncated":names_truncated,"generation":self.generation,"url":url,"title":title,"cursor_scan_limit":if a["cursor_interactive"]==true{json!(5000)}else{Value::Null}}),
        )
    }

    /// Builds a delta snapshot: added or changed elements only, plus explicit
    /// tombstones for refs that disappeared. A first call, a document change, or a
    /// URL change returns a full snapshot, because a partial answer would be
    /// misleading in exactly the situations where the agent has least context.
    fn delta_payload(&self, input: &DeltaInput<'_>) -> Value {
        let DeltaInput {
            lines,
            elements,
            fingerprints,
            previous,
            url,
            title,
            truncated,
            names_truncated,
            args,
        } = input;
        let (lines, elements, fingerprints, url, title, args) = (
            *lines,
            *elements,
            *fingerprints,
            url.clone(),
            title.clone(),
            *args,
        );
        let compact = args["compact"] == true || args["interactive_only"] == true;
        let full = json!({
            "text": lines.join("\n"),
            "elements": elements,
            "truncated": truncated,
            "names_truncated": names_truncated,
            "generation": self.generation,
            "url": url,
            "title": title,
        });

        let url_now = url.as_str().unwrap_or_default();
        let Some(previous) = previous.filter(|p| p.document == self.document && p.url == url_now)
        else {
            return {
                let mut v = full;
                v["delta"] = json!({"baseline":false,"reason":if previous_is_none(*previous){"no previous snapshot on this tab"}else{"document or URL changed since last snapshot"},"added":elements.len(),"changed":0,"removed":0});
                v
            };
        };

        let mut added = Vec::new();
        let mut changed = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        let mut delta_lines: Vec<String> = Vec::new();

        for element in elements.iter() {
            let reference = element["ref"].as_str().unwrap_or_default();
            // Identity comes from the backend node, since the ref itself is
            // generation-scoped and reissued on every snapshot.
            let node = fingerprints
                .iter()
                .find(|(_, entry)| entry.reference == reference)
                .map(|(node, _)| *node);
            match node.and_then(|node| {
                let current = fingerprints.get(&node)?;
                previous.entries.get(&node).map(|seen| (seen, current))
            }) {
                // Present before and identical: nothing to report.
                Some((seen, current)) if seen.fingerprint == current.fingerprint => {}
                Some((_, current)) => {
                    changed.push(element.clone());
                    delta_lines.push(format!(
                        "~ changed {} {} [ref={}]",
                        element["role"].as_str().unwrap_or_default(),
                        json!(element["name"].as_str().unwrap_or_default()),
                        current.reference
                    ));
                }
                None => {
                    added.push(element.clone());
                    let indent = if compact {
                        String::new()
                    } else {
                        "  ".repeat(element["depth"].as_u64().unwrap_or(0).min(100) as usize)
                    };
                    delta_lines.push(format!(
                        "{indent}+ {} {} [ref={}]",
                        element["role"].as_str().unwrap_or_default(),
                        json!(element["name"].as_str().unwrap_or_default()),
                        reference
                    ));
                }
            }
        }
        for (node, entry) in &previous.entries {
            if !fingerprints.contains_key(node) {
                // Report the ref the agent actually holds, not a reissued one.
                removed.push(entry.reference.clone());
                delta_lines.push(format!("- removed [ref={}]", entry.reference));
            }
        }
        removed.sort();

        json!({
            "text": delta_lines.join("\n"),
            "elements": added,
            "changed": changed,
            "removed": removed,
            "truncated": truncated,
            "names_truncated": names_truncated,
            "generation": self.generation,
            "url": url,
            "title": title,
            "delta": {
                "baseline": false,
                "added": added.len(),
                "changed": changed.len(),
                "removed": removed.len(),
            },
        })
    }
}

/// Distinguishes "no baseline yet" from "baseline invalidated" for the caller.
fn previous_is_none(previous: Option<&crate::browser::SnapshotDelta>) -> bool {
    previous.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::DeltaEntry;

    fn entry(reference: &str, fingerprint: &str) -> DeltaEntry {
        DeltaEntry {
            reference: reference.to_owned(),
            fingerprint: fingerprint.to_owned(),
        }
    }

    /// A snapshot with no prior baseline must return everything, not a fragment.
    #[test]
    fn delta_without_baseline_returns_everything() {
        let baseline = crate::browser::SnapshotDelta::default();
        assert!(previous_is_none(None));
        assert!(
            !previous_is_none(Some(&baseline)),
            "an existing but invalidated baseline is not the same as no baseline"
        );
    }

    /// Identity must be the backend node, never the ref. Refs are generation
    /// scoped and reissued on every snapshot; keying on them would make every
    /// element look both added and removed.
    #[test]
    fn delta_entries_are_keyed_by_node_not_ref() {
        let mut entries = HashMap::new();
        entries.insert(7i64, entry("e1_1", "button\u{1f}Save"));
        entries.insert(9i64, entry("e1_2", "textbox\u{1f}Email"));
        let snapshot = crate::browser::SnapshotDelta {
            entries,
            document: Some(1),
            url: "https://example.com".into(),
        };

        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[&7].reference, "e1_1");
        assert_eq!(snapshot.entries[&9].fingerprint, "textbox\u{1f}Email");
    }

    #[test]
    fn delta_entry_keeps_the_ref_the_agent_holds() {
        // Tombstones must name the ref previously issued, so an agent can
        // recognise which handle became invalid.
        let previous = crate::browser::SnapshotDelta {
            entries: [(3i64, entry("e5_4", "button\u{1f}Submit"))]
                .into_iter()
                .collect(),
            document: Some(1),
            url: "https://example.com".into(),
        };
        assert_eq!(previous.entries[&3].reference, "e5_4");
    }
}
