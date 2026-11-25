use std::path::absolute;

use crate::history::{HistoryRecord, HistoryStore};
use crate::meeting::transcript::{history_text, vault_note};

use super::records::{ExportProjection, HistoryProjection};
use super::*;

pub(in crate::meeting) fn replay_pending(root: &Path, states: &[scan::DiskState]) {
    for state in states.iter().filter(|state| state.needs_replay()) {
        match Bundle::open(root, state.id()) {
            Ok(bundle) => {
                bundle.replay_publication();
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => eprintln!(
                "Could not open meeting {} for projection repair: {error}",
                state.id()
            ),
        }
    }
}

impl Bundle {
    pub(in crate::meeting) fn replay_publication(
        &self,
    ) -> Option<(super::super::Artifact, Projections)> {
        let state = self.read_state();
        let scan::DiskState::Complete {
            receipt,
            seal,
            projections,
            ..
        } = &state
        else {
            return None;
        };
        if state.needs_replay() {
            let path = crate::paths::history_database();
            let cutoff = seal.as_ref().map(|seal| seal.cutoff);
            match self.committed_markdown(receipt) {
                Ok(markdown) => {
                    self.project(
                        receipt,
                        &markdown,
                        cutoff,
                        *projections,
                        path.as_deref().map_err(String::as_str),
                    );
                }
                Err(error) => eprintln!(
                    "Could not read committed meeting {}: {error}",
                    self.manifest.info.id
                ),
            }
        }
        self.completed_artifact()
    }

    pub(in crate::meeting) fn finish_publication(
        &self,
        receipt: &Receipt,
        markdown: &str,
        cutoff: Offset,
    ) {
        let path = crate::paths::history_database();
        self.project(
            receipt,
            markdown,
            Some(cutoff),
            Projections::pending(&self.manifest),
            path.as_deref().map_err(String::as_str),
        );
    }

    pub(in crate::meeting) fn completed_artifact(
        &self,
    ) -> Option<(super::super::Artifact, Projections)> {
        let root = self.directory.parent()?;
        if let super::super::MeetingView::Complete(_, artifact, projections) =
            self.read_state().view(root)
        {
            Some((artifact, projections))
        } else {
            None
        }
    }

    fn committed_markdown(&self, receipt: &Receipt) -> Result<String, String> {
        let markdown =
            fs::read_to_string(self.directory.join(TRANSCRIPT)).map_err(|err| err.to_string())?;
        if sha256(markdown.as_bytes()) != receipt.transcript_sha256 {
            return Err("Committed meeting transcript does not match its receipt".into());
        }
        Ok(markdown)
    }

    pub(super) fn project(
        &self,
        receipt: &Receipt,
        markdown: &str,
        cutoff: Option<Offset>,
        mut projections: Projections,
        history_path: Result<&Path, &str>,
    ) {
        if let Err(error) = self.apply_retention() {
            eprintln!(
                "Could not apply audio retention to meeting {}: {error}",
                self.manifest.info.id
            );
        }
        #[cfg(test)]
        self.writer.checkpoint("retention");

        if projections.history == HistoryProjection::Pending {
            match history_path
                .map_err(str::to_owned)
                .and_then(|path| self.index(path, receipt, markdown))
            {
                Ok(()) => projections.history = HistoryProjection::Indexed,
                Err(error) => eprintln!(
                    "Meeting History pending for {}: {error}",
                    self.manifest.info.id
                ),
            }
        }
        #[cfg(test)]
        self.writer.checkpoint("history");

        if projections.export == ExportProjection::Pending {
            let result = cutoff
                .ok_or_else(|| "Missing seal for export".to_string())
                .and_then(|cutoff| self.export(receipt, markdown, cutoff));
            match result {
                Ok(()) => projections.export = ExportProjection::Exported,
                Err(error) => eprintln!(
                    "Meeting export pending for {}: {error}",
                    self.manifest.info.id
                ),
            }
        }
        #[cfg(test)]
        self.writer.checkpoint("export");

        if let Err(error) = self.write_projections(&projections) {
            eprintln!(
                "Could not record projections for meeting {}: {error}",
                self.manifest.info.id
            );
        }
    }

    fn index(&self, path: &Path, receipt: &Receipt, markdown: &str) -> Result<(), String> {
        let history = &receipt.publication.history;
        let text = history_text(markdown)?;
        if history.id != format!("meeting:{}", self.manifest.info.id)
            || sha256(text.as_bytes()) != history.text_sha256
        {
            return Err("Meeting History does not match its receipt".into());
        }
        let store = HistoryStore::open(path)?;
        let record = HistoryRecord {
            id: history.id.clone(),
            created_at_ms: i64::try_from(history.created_at_ms).map_err(|err| err.to_string())?,
            model: history.model.clone(),
            text,
            raw_text: None,
        };
        match store.get(&record.id)? {
            Some(existing) if existing == record => Ok(()),
            Some(_) if store.replace_meeting(&record)? => Ok(()),
            _ => store.save(&record),
        }
    }

    fn export(&self, receipt: &Receipt, markdown: &str, cutoff: Offset) -> Result<(), String> {
        let directory = self
            .manifest
            .export_dir
            .as_ref()
            .ok_or("Missing vault export directory")?;
        let bundle_path = absolute(&self.directory).map_err(|err| err.to_string())?;
        let (name, note) = vault_note(
            &self.manifest,
            cutoff,
            &receipt.publication,
            markdown,
            &bundle_path,
        )?;
        ensure_directory(directory).map_err(|err| err.to_string())?;
        self.writer
            .write_export(directory, &name, &note)
            .map_err(|err| err.to_string())
    }
}
