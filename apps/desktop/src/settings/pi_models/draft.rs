use super::*;
use std::{collections::BTreeMap, fs, io, path::Path};
use tempfile::NamedTempFile;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct SavedForm {
    target: Target,
    initial: Values,
    value: Values,
    revision: String,
}
#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    drafts: BTreeMap<String, SavedForm>,
}
impl File {
    fn load(root: &Path) -> io::Result<Self> {
        match fs::File::open(root.join("desktop-model-drafts.json")) {
            Ok(file) => {
                let value: Self = serde_json::from_reader(file)?;
                if value.version != 1 {
                    return Err(io::Error::other("模型草稿版本不受支持"));
                }
                Ok(value)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self {
                version: 1,
                drafts: BTreeMap::new(),
            }),
            Err(error) => Err(error),
        }
    }
    fn update(root: &Path, path: &str, form: Option<SavedForm>) -> io::Result<()> {
        let mut file = Self::load(root)?;
        if file.drafts.get(path) == form.as_ref() {
            return Ok(());
        }
        if let Some(form) = form {
            file.drafts.insert(path.into(), form);
        } else {
            file.drafts.remove(path);
        }
        let mut temp = NamedTempFile::new_in(root)?;
        serde_json::to_writer(temp.as_file_mut(), &file)?;
        temp.persist(root.join("desktop-model-drafts.json"))?;
        Ok(())
    }
}
impl PiModels {
    pub(crate) fn set_draft_root(&mut self, root: &Path) {
        self.draft_root = Some(root.to_owned());
    }
    pub(crate) fn has_draft_root(&self) -> bool {
        self.draft_root.is_some()
    }
    pub(crate) fn flush_draft(&self, cx: &App) -> io::Result<()> {
        if self.target.is_none() {
            return Ok(());
        }
        let root = self
            .draft_root
            .as_deref()
            .ok_or_else(|| io::Error::other("模型草稿存储尚未可用，请保留窗口或丢弃改动"))?;
        let form = self.dirty(cx).then(|| SavedForm {
            target: self.target.clone().expect("active form"),
            initial: self.initial.clone(),
            value: self.values(cx),
            revision: self.revision.clone(),
        });
        File::update(root, &self.form_path, form)
    }
    pub(super) fn clear_draft(&self) -> io::Result<()> {
        if self.target.is_some()
            && let Some(root) = &self.draft_root
        {
            File::update(root, &self.form_path, None)?;
        }
        Ok(())
    }
}
impl HiveApp {
    pub(super) fn restore_pi_model_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pi_models.target.is_some() {
            return;
        }
        let Some(snapshot) = &self.pi_models.snapshot else {
            return;
        };
        let path = snapshot.path.clone();
        let revision = snapshot.revision.clone();
        if self.pi_models.draft_checked.as_ref() == Some(&path) {
            return;
        }
        let Some(root) = &self.pi_models.draft_root else {
            return;
        };
        let saved = match File::load(root) {
            Ok(mut file) => {
                self.pi_models.draft_checked = Some(path.clone());
                file.drafts.remove(&path)
            }
            Err(error) => {
                self.pi_models.message = Some(format!("无法恢复模型草稿，原文件已保留：{error}"));
                return;
            }
        };
        let Some(saved) = saved else {
            return;
        };
        let changed = revision != saved.revision;
        self.pi_models.form_path = path;
        self.pi_models.revision = saved.revision;
        self.pi_models.initial = saved.initial;
        self.pi_models.target = Some(saved.target);
        self.apply_pi_model_values(saved.value, window, cx);
        self.pi_models.message = Some(
            if changed {
                "已恢复未保存的模型草稿；配置文件已有外部修改，保存前请核对，冲突检查仍会生效。"
            } else {
                "已恢复上次未保存的模型草稿。"
            }
            .into(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{File, SavedForm, Target, Values};
    use std::fs;
    fn sample() -> SavedForm {
        SavedForm {
            target: Target {
                provider: "custom".into(),
                model: Some("terra".into()),
                is_model: true,
                creating: false,
            },
            initial: Values {
                name: "before".into(),
                ..Values::default()
            },
            value: Values {
                context: "unfinished-number".into(),
                reasoning: true,
                vision: true,
                ..Values::default()
            },
            revision: "original-revision".into(),
        }
    }
    #[test]
    fn restores_original_conflict_base_and_keeps_other_config_drafts() {
        let root = tempfile::tempdir().unwrap();
        let saved = sample();
        File::update(root.path(), "/first/models.json", Some(saved.clone())).unwrap();
        let mut other = saved.clone();
        other.value.api = "openai-responses".into();
        File::update(root.path(), "/other/models.json", Some(other.clone())).unwrap();
        let reopened = File::load(root.path()).unwrap();
        assert!(reopened.drafts["/first/models.json"] == saved);
        assert!(reopened.drafts["/other/models.json"] == other);
        File::update(root.path(), "/first/models.json", None).unwrap();
        let reopened = File::load(root.path()).unwrap();
        assert!(!reopened.drafts.contains_key("/first/models.json"));
        assert!(reopened.drafts["/other/models.json"] == other);
    }
    #[test]
    fn unreadable_drafts_are_not_overwritten_or_discarded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("desktop-model-drafts.json");
        fs::write(&path, b"incomplete json").unwrap();
        assert!(File::update(root.path(), "/first/models.json", Some(sample())).is_err());
        assert!(File::update(root.path(), "/first/models.json", None).is_err());
        assert_eq!(fs::read(path).unwrap(), b"incomplete json");
    }
    #[cfg(unix)]
    #[test]
    fn failed_atomic_write_preserves_the_previous_form() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        File::update(root.path(), "/models.json", Some(sample())).unwrap();
        let path = root.path().join("desktop-model-drafts.json");
        let before = fs::read(&path).unwrap();
        let permissions = fs::metadata(root.path()).unwrap().permissions();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o500)).unwrap();
        let mut changed = sample();
        changed.value.name = "new unsaved value".into();
        let failed = File::update(root.path(), "/models.json", Some(changed));
        let discard = File::update(root.path(), "/models.json", None);
        fs::set_permissions(root.path(), permissions).unwrap();
        assert!(failed.is_err());
        assert!(discard.is_err());
        assert_eq!(fs::read(path).unwrap(), before);
        assert!(File::load(root.path()).unwrap().drafts["/models.json"] == sample());
    }
}
