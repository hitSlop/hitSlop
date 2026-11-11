//! One typed SQL row owns document lifecycle and publication state.
use crate::{
    policy,
    room::{CloudDocument, fail},
};
use pop_sharing_protocol::http::RoomPhase;
use serde::Deserialize;
use worker::{Result, SqlStorage, SqlStorageValue};
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct DocumentState {
    pub(crate) owner_uid: String,
    pub(crate) document: String,
    pub(crate) phase: RoomPhase,
    pub(crate) capability: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) template_id: Option<String>,
    pub(crate) seed_hash: Option<String>,
    pub(crate) seed_size: Option<u32>,
    pub(crate) app: Option<String>,
    pub(crate) root: Option<String>,
    pub(crate) update_bytes: Option<usize>,
    pub(crate) update_rows: Option<usize>,
    pub(crate) base_version: Option<String>,
    pub(crate) access_generation: Option<u64>,
    pub(crate) library_revision: Option<u64>,
    pub(crate) library_pending: Option<u64>,
    pub(crate) library_retry_ms: Option<u64>,
    pub(crate) cleanup_pending: Option<u64>,
}
impl DocumentState {
    pub(crate) fn load(sql: &SqlStorage) -> Result<Option<Self>> {
        Ok(sql.exec("SELECT * FROM document_state", None)?.to_array::<Self>()?.into_iter().next())
    }
    pub(crate) fn save(&self, sql: &SqlStorage) -> Result<()> {
        let phase = match self.phase {
            RoomPhase::Uploading => "uploading",
            RoomPhase::Live => "live",
            RoomPhase::Ended => "ended",
        };
        sql.exec("INSERT OR REPLACE INTO document_state(id,owner_uid,document,phase,capability,title,template_id,seed_hash,seed_size,app,root,update_bytes,update_rows,base_version,access_generation,library_revision,library_pending,library_retry_ms,cleanup_pending) VALUES(1,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", vec![
 self.owner_uid.clone().into(), self.document.clone().into(), phase.into(),
 self.capability.clone().map(|v| v.into()).unwrap_or(SqlStorageValue::Null),
 self.title.clone().map(|v| v.into()).unwrap_or(SqlStorageValue::Null),
 self.template_id.clone().map(|v| v.into()).unwrap_or(SqlStorageValue::Null),
 self.seed_hash.clone().map(|v| v.into()).unwrap_or(SqlStorageValue::Null),
 self.seed_size.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
 self.app.clone().map(|v| v.into()).unwrap_or(SqlStorageValue::Null),
 self.root.clone().map(|v| v.into()).unwrap_or(SqlStorageValue::Null),
 self.update_bytes.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
 self.update_rows.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
 self.base_version.clone().map(|v| v.into()).unwrap_or(SqlStorageValue::Null),
 self.access_generation.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
 self.library_revision.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
 self.library_pending.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
 self.library_retry_ms.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
 self.cleanup_pending.map(|v| (v as i64).into()).unwrap_or(SqlStorageValue::Null),
        ])?;
        Ok(())
    }
}
impl CloudDocument {
    pub(crate) fn document_state(&self) -> Result<Option<DocumentState>> {
        DocumentState::load(&self.sql)
    }
    pub(crate) fn stored(&self) -> Result<DocumentState> {
        self.document_state()?.ok_or_else(|| fail("Missing document state"))
    }
    pub(crate) fn update_state(&self, edit: impl FnOnce(&mut DocumentState)) -> Result<()> {
        let mut state = self.stored()?;
        edit(&mut state);
        state.save(&self.sql)
    }
    pub(crate) fn phase(&self) -> Result<Option<RoomPhase>> {
        Ok(self.document_state()?.map(|s| s.phase))
    }
    pub(crate) fn set_phase(&self, to: RoomPhase) -> Result<()> {
        let mut state = self.stored()?;
        state.phase = policy::transition(Some(state.phase), to).map_err(|_| fail("Invalid phase transition"))?;
        state.save(&self.sql)
    }
}
pub(crate) fn initialize(sql: &SqlStorage) {
    sql.exec("CREATE TABLE IF NOT EXISTS document_state(id INTEGER PRIMARY KEY CHECK(id=1),owner_uid TEXT NOT NULL,document TEXT NOT NULL,phase TEXT NOT NULL CHECK(phase IN ('uploading','live','ended')),capability TEXT,title TEXT,template_id TEXT,seed_hash TEXT,seed_size INTEGER CHECK(seed_size>=0),app TEXT,root TEXT,update_bytes INTEGER CHECK(update_bytes>=0),update_rows INTEGER CHECK(update_rows>=0),base_version TEXT,access_generation INTEGER CHECK(access_generation>=0),library_revision INTEGER CHECK(library_revision>=0),library_pending INTEGER CHECK(library_pending>=0),library_retry_ms INTEGER CHECK(library_retry_ms>=0),cleanup_pending INTEGER CHECK(cleanup_pending>=0));",None).expect("document state schema");
}
