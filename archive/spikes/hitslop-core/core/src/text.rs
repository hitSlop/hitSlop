//! Draft ancestry is an authored branch, never a renderer-owned CRDT replica.
use super::*;
pub(super) struct Draft {
    path: String,
    base: String,
    sequence: usize,
    request: String,
    reply: String,
    /// The owner version right after this draft's last accepted edit. While the
    /// owner is still exactly there, the authored text equals the owner's text.
    accepted: VersionVector,
    /// Materialized only once other edits intervene after this draft started.
    authored: Option<LoroDoc>,
}
fn text_at(
    doc: &LoroDoc,
    schema: &Node,
    path: &[Segment],
    lists: &HashMap<ContainerID, ListState>,
) -> Result<LoroText> {
    let loc = resolve(doc, schema, path, &Rows::new(lists))?;
    match (loc.node, loc.value) {
        (Node::Text, ValueOrContainer::Container(Container::Text(text))) => Ok(text),
        _ => Err(err("type_mismatch", "Expected text")),
    }
}
fn unicode_offset(text: &str, utf16: usize) -> Result<usize> {
    let mut n = 0;
    for (i, c) in text.chars().enumerate() {
        if n == utf16 {
            return Ok(i);
        }
        n += c.len_utf16();
    }
    if n == utf16 {
        Ok(text.chars().count())
    } else {
        Err(err(
            "out_of_range",
            "Selection splits a surrogate or exceeds text",
        ))
    }
}
impl Document {
    pub fn detach_renderer(&mut self) {
        self.retired
            .extend(std::mem::take(&mut self.drafts).into_keys());
    }
    pub fn release_draft(&mut self, draft: &str) -> Result<()> {
        if self.drafts.remove(draft).is_some() {
            self.retired.insert(draft.to_owned());
        }
        Ok(())
    }
    pub fn text(&mut self, request: &str) -> Result<String> {
        let r: wire::TextRequest = parse(request)?;
        let canonical = encode(&r)?;
        if r.session != self.session {
            return Err(err("wrong_session", "Reconcile with a fresh snapshot"));
        }
        if r.draft.is_empty() || r.draft.len() > 128 || r.sequence == 0 {
            return Err(err("invalid_request", "Invalid draft identity/sequence"));
        }
        if self.retired.contains(&r.draft) {
            return Err(err(
                "unknown_outcome",
                "Draft already released; reconcile before editing",
            ));
        }
        let path = encode(&r.path)?;
        let owner = self.doc.oplog_vv();
        // `Some(branch)`: other edits intervened, merge through the authored branch.
        // `None`: the authored text is the owner's text; edit the owner directly.
        let branch = if let Some(draft) = self.drafts.get(&r.draft) {
            if draft.sequence == r.sequence {
                return if draft.request == canonical {
                    Ok(draft.reply.clone())
                } else {
                    Err(err(
                        "request_conflict",
                        "Sequence reused with different content",
                    ))
                };
            }
            if r.parent != Some(draft.sequence)
                || r.sequence != draft.sequence + 1
                || draft.path != path
                || draft.base != r.base
            {
                return Err(err("draft_parent", "Wrong field, base or parent"));
            }
            match &draft.authored {
                Some(authored) => {
                    let doc = authored.fork();
                    doc.set_peer_id(authored.peer_id()).map_err(engine)?;
                    Some(doc)
                }
                None if draft.accepted == owner => None,
                None => Some(
                    self.doc
                        .fork_at(&self.doc.vv_to_frontiers(&draft.accepted))
                        .map_err(engine)?,
                ),
            }
        } else {
            if self.drafts.len() >= 64 || self.retired.len() >= 4096 {
                return Err(err(
                    "too_large",
                    "Draft session capacity; reopen after flushing",
                ));
            }
            if r.sequence != 1 || r.parent.is_some() {
                return Err(err("draft_parent", "Unknown draft"));
            }
            let vv = decode_version(&r.base)?;
            if vv == owner {
                None
            } else {
                let fronts = self.doc.vv_to_frontiers(&vv);
                let doc = self.doc.fork_at(&fronts).map_err(engine)?;
                if doc.oplog_vv() != vv {
                    return Err(err("stale_base", "Unknown or non-causal draft base"));
                }
                Some(doc)
            }
        };
        // Resolve the current target too: an old branch cannot resurrect a removed row.
        let current_text = text_at(&self.doc, &self.schema, &r.path, &self.lists)?;
        let (value, selection, branch) = match branch {
            None => {
                // Validate the complete result before the owner's first mutation.
                let value = spliced(&current_text.to_string(), r.index, r.delete, &r.insert)?;
                unicode_offset(&value, r.selectionStart)?;
                unicode_offset(&value, r.selectionEnd)?;
                let intent = Intent::Splice {
                    path: r.path.clone(),
                    base: self.version(),
                    index: r.index,
                    delete: r.delete,
                    insert: r.insert.clone(),
                };
                let before = self.doc.state_frontiers();
                let version = self.version();
                let result = execute(
                    &self.doc,
                    &self.schema,
                    &intent,
                    &version,
                    &mut vec![],
                    &mut Rows::new(&self.lists),
                );
                if let Err(e) = result {
                    self.abort(&before)?;
                    return Err(e);
                }
                self.doc.commit();
                (value, [r.selectionStart, r.selectionEnd], None)
            }
            Some(authored) => {
                let branch_lists = HashMap::new();
                let authored_text = text_at(&authored, &self.schema, &r.path, &branch_lists)?;
                if current_text.id() != authored_text.id() {
                    return Err(err("path_not_found", "Text identity changed"));
                }
                let from = authored.oplog_vv();
                let base = version_token(&from);
                let intent = Intent::Splice {
                    path: r.path.clone(),
                    base: base.clone(),
                    index: r.index,
                    delete: r.delete,
                    insert: r.insert.clone(),
                };
                execute(
                    &authored,
                    &self.schema,
                    &intent,
                    &base,
                    &mut vec![],
                    &mut Rows::new(&branch_lists),
                )?;
                authored.commit();
                let value = authored_text.to_string();
                let selection = [r.selectionStart, r.selectionEnd].map(|v| {
                    let offset = unicode_offset(&value, v)?;
                    authored_text
                        .get_cursor(offset, loro::cursor::Side::Middle)
                        .ok_or_else(|| err("out_of_range", "Cannot anchor selection"))
                });
                let [start, end] = selection;
                let cursors = [start?, end?];
                // Our own authored delta: its dependencies are owner history.
                self.doc
                    .import(&authored.export(ExportMode::updates(&from)).map_err(engine)?)
                    .map_err(|e| err("invalid_bytes", e))?;
                let merged = text_at(&self.doc, &self.schema, &r.path, &self.lists)?.to_string();
                let mut positions = [0usize; 2];
                for (slot, cursor) in positions.iter_mut().zip(cursors) {
                    let pos = self.doc.get_cursor_pos(&cursor).map_err(engine)?.current.pos;
                    *slot = merged.chars().take(pos).map(char::len_utf16).sum::<usize>();
                }
                (value, positions, Some(authored))
            }
        };
        let previous = self.sequence;
        let response = self.publish(vec![])?;
        let mut response: Value = serde_json::from_str(&response).expect("own encoded publication");
        response["text"] = json!({"draft":r.draft,"sequence":r.sequence,"authored":value,"selectionStart":selection[0],"selectionEnd":selection[1]});
        let reply = response.to_string();
        debug_assert_eq!(self.sequence, previous + 1);
        self.drafts.insert(
            r.draft,
            Draft {
                path,
                base: r.base,
                sequence: r.sequence,
                request: canonical,
                reply: reply.clone(),
                accepted: self.doc.oplog_vv(),
                authored: branch,
            },
        );
        Ok(reply)
    }
}

/// The text after a UTF-16 splice, refusing ranges that split a surrogate pair.
fn spliced(text: &str, index: usize, delete: usize, insert: &str) -> Result<String> {
    let end = index
        .checked_add(delete)
        .ok_or_else(|| err("out_of_range", "Text range overflow"))?;
    let byte = |target: usize| -> Result<usize> {
        let mut units = 0;
        for (i, c) in text.char_indices() {
            if units == target {
                return Ok(i);
            }
            units += c.len_utf16();
        }
        if units == target {
            Ok(text.len())
        } else {
            Err(err("out_of_range", "Invalid UTF-16 boundary"))
        }
    };
    let (from, to) = (byte(index)?, byte(end)?);
    Ok(format!("{}{}{}", &text[..from], insert, &text[to..]))
}
