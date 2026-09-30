use super::*;
use crate::agents::Backend;

impl StateStore {
    pub(crate) fn load_expand_transcript_folders(&self) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT value FROM meta WHERE key='expand_transcript_folders'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.as_deref() == Some("true"))
            .map_err(|error| format!("load transcript folder setting: {error}"))
    }

    pub(crate) fn save_expand_transcript_folders(&self, expanded: bool) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO meta(key, value) VALUES('expand_transcript_folders', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [if expanded { "true" } else { "false" }],
            )
            .map(|_| ())
            .map_err(|error| format!("save transcript folder setting: {error}"))
    }

    pub(crate) fn load_stage_changes_like_vscode(&self) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT value FROM meta WHERE key='stage_changes_like_vscode'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.as_deref() != Some("false"))
            .map_err(|error| format!("load source control setting: {error}"))
    }

    pub(crate) fn save_stage_changes_like_vscode(&self, enabled: bool) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO meta(key, value) VALUES('stage_changes_like_vscode', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [if enabled { "true" } else { "false" }],
            )
            .map(|_| ())
            .map_err(|error| format!("save source control setting: {error}"))
    }

    pub(crate) fn load_hide_unchanged_lines(&self) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT value FROM meta WHERE key='hide_unchanged_lines'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.as_deref() == Some("true"))
            .map_err(|error| format!("load diff setting: {error}"))
    }

    pub(crate) fn save_hide_unchanged_lines(&self, hidden: bool) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO meta(key, value) VALUES('hide_unchanged_lines', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [if hidden { "true" } else { "false" }],
            )
            .map(|_| ())
            .map_err(|error| format!("save diff setting: {error}"))
    }

    pub(crate) fn load_hide_split_borders(&self) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT value FROM meta WHERE key='hide_split_borders'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.as_deref() == Some("true"))
            .map_err(|error| format!("load terminal setting: {error}"))
    }

    pub(crate) fn save_hide_split_borders(&self, hidden: bool) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO meta(key, value) VALUES('hide_split_borders', ?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [if hidden { "true" } else { "false" }],
            )
            .map(|_| ())
            .map_err(|error| format!("save terminal setting: {error}"))
    }

    pub(crate) fn load_source_control_view(&self) -> Result<Option<String>, String> {
        self.optional_setting("source_control_view", "load source control view")
    }

    pub(crate) fn save_source_control_view(&self, view: &str) -> Result<(), String> {
        self.save_setting("source_control_view", view, "save source control view")
    }

    pub(crate) fn load_source_control_sort(&self) -> Result<Option<String>, String> {
        self.optional_setting("source_control_sort", "load source control sort")
    }

    pub(crate) fn save_source_control_sort(&self, sort: &str) -> Result<(), String> {
        self.save_setting("source_control_sort", sort, "save source control sort")
    }

    fn optional_setting(&self, key: &str, context: &str) -> Result<Option<String>, String> {
        self.connection
            .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| format!("{context}: {error}"))
    }

    fn save_setting(&self, key: &str, value: &str, context: &str) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [key, value],
            )
            .map(|_| ())
            .map_err(|error| format!("{context}: {error}"))
    }

    pub(crate) fn load_text_editor(&self) -> Result<Option<String>, String> {
        self.connection
            .query_row(
                "SELECT value FROM meta WHERE key='text_editor'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|value| value.filter(|value| !value.trim().is_empty()))
            .map_err(|error| format!("load text editor setting: {error}"))
    }

    pub(crate) fn save_text_editor(&self, command: Option<&str>) -> Result<(), String> {
        match command {
            Some(command) => self.connection.execute(
                "INSERT INTO meta(key, value) VALUES('text_editor', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [command],
            ),
            None => self
                .connection
                .execute("DELETE FROM meta WHERE key='text_editor'", []),
        }
        .map(|_| ())
        .map_err(|error| format!("save text editor setting: {error}"))
    }

    pub(crate) fn load_preferred_harness(&self, project: &Path) -> Result<Option<Backend>, String> {
        // Before the first saved choice, infer it from this project's main sessions.
        let normalized_project = crate::sessions::normalize_session_path(project);
        let legacy_project = project.to_string_lossy();
        self.connection
            .query_row(
                "SELECT COALESCE(
                    (SELECT value FROM meta WHERE key='preferred_harness'),
                    (SELECT harness FROM sessions
                     WHERE submitted=1 AND client_key IS NOT NULL
                       AND project_id IN (SELECT id FROM projects WHERE path IN (?1, ?2))
                       AND parent_id IS NULL AND parent_backend_id IS NULL
                     ORDER BY created_ms DESC, id DESC LIMIT 1))",
                [
                    normalized_project.to_string_lossy().as_ref(),
                    legacy_project.as_ref(),
                ],
                |row| row.get(0),
            )
            .map_err(|error| format!("load preferred harness: {error}"))
            .and_then(|harness: Option<String>| match harness {
                Some(harness) if harness.trim().is_empty() => {
                    Err("The saved backend is empty. Choose a backend.".into())
                }
                Some(harness) => harness.parse().map(Some),
                None => Ok(None),
            })
    }

    pub(crate) fn save_preferred_harness(&self, harness: Backend) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO meta(key, value) VALUES('preferred_harness', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [harness],
            )
            .map(|_| ())
            .map_err(|error| format!("save preferred harness: {error}"))
    }

    pub(crate) fn load_window_placement(&self) -> Result<Option<WindowPlacement>, String> {
        self.load_json_setting("window_placement_json", "window placement")
    }

    pub(crate) fn save_window_placement(&self, placement: &WindowPlacement) -> Result<(), String> {
        self.save_json_setting("window_placement_json", "window placement", placement)
    }

    pub(crate) fn load_app_session_order(&self) -> Result<Vec<i64>, String> {
        Ok(self
            .load_json_setting("app_session_order_json", "application session order")?
            .unwrap_or_default())
    }

    pub(crate) fn save_app_session_order(&self, order: &[i64]) -> Result<(), String> {
        self.save_json_setting("app_session_order_json", "application session order", order)
    }

    pub(crate) fn load_network_proxy(&self) -> Result<Option<String>, String> {
        self.load_text_setting("network_proxy", "network proxy")
    }

    pub(crate) fn save_network_proxy(&self, proxy: Option<&str>) -> Result<(), String> {
        if let Some(proxy) = proxy {
            crate::access::validate_app_proxy(proxy)?;
        }
        self.ensure_ui_state()?;
        self.connection
            .execute("UPDATE ui_state SET network_proxy=?1 WHERE id=1", [proxy])
            .map(|_| ())
            .map_err(|error| format!("save network proxy: {error}"))
    }

    pub(crate) fn load_configuration_catalogs(
        &self,
    ) -> Result<Vec<CachedConfigurationCatalog>, String> {
        self.load_json_setting("configuration_catalogs_json", "configuration catalogs")
            .map(Option::unwrap_or_default)
            .map(normalize_configuration_catalogs)
    }

    pub(crate) fn save_configuration_catalogs(
        &self,
        catalogs: &[CachedConfigurationCatalog],
    ) -> Result<(), String> {
        self.save_json_setting(
            "configuration_catalogs_json",
            "configuration catalogs",
            &normalize_configuration_catalogs(catalogs.to_vec()),
        )
    }

    pub(crate) fn load_session_control_defaults(
        &self,
    ) -> Result<Vec<CachedSessionControlDefaults>, String> {
        self.load_json_setting("session_control_defaults_json", "session control defaults")
            .map(Option::unwrap_or_default)
    }

    pub(crate) fn save_session_control_defaults(
        &self,
        defaults: &[CachedSessionControlDefaults],
    ) -> Result<(), String> {
        self.save_json_setting(
            "session_control_defaults_json",
            "session control defaults",
            defaults,
        )
    }

    fn load_json_setting<T: DeserializeOwned>(
        &self,
        column: &str,
        subject: &str,
    ) -> Result<Option<T>, String> {
        let stored = self.load_text_setting(column, subject)?;
        stored
            .map(|value| {
                serde_json::from_str(&value).map_err(|error| format!("decode {subject}: {error}"))
            })
            .transpose()
    }

    fn save_json_setting<T: Serialize + ?Sized>(
        &self,
        column: &str,
        subject: &str,
        value: &T,
    ) -> Result<(), String> {
        let value =
            serde_json::to_string(value).map_err(|error| format!("encode {subject}: {error}"))?;
        self.ensure_ui_state()?;
        self.connection
            .execute(
                &format!("UPDATE ui_state SET {column}=?1 WHERE id=1"),
                [value],
            )
            .map(|_| ())
            .map_err(|error| format!("save {subject}: {error}"))
    }

    fn load_text_setting(&self, column: &str, subject: &str) -> Result<Option<String>, String> {
        self.connection
            .query_row(
                &format!("SELECT {column} FROM ui_state WHERE id=1"),
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| format!("load {subject}: {error}"))
            .map(Option::flatten)
    }

    fn ensure_ui_state(&self) -> Result<(), String> {
        self.connection
            .execute("INSERT OR IGNORE INTO ui_state(id) VALUES(1)", [])
            .map(|_| ())
            .map_err(|error| format!("ensure ui_state: {error}"))
    }
}

fn normalize_configuration_catalogs(
    catalogs: Vec<CachedConfigurationCatalog>,
) -> Vec<CachedConfigurationCatalog> {
    let mut indexes = BTreeMap::new();
    let mut normalized = Vec::with_capacity(catalogs.len());
    for mut catalog in catalogs {
        catalog.project = crate::sessions::normalize_session_path(&catalog.project);
        let key = (catalog.harness, catalog.project.clone());
        if let Some(index) = indexes.get(&key) {
            normalized[*index] = catalog;
        } else {
            indexes.insert(key, normalized.len());
            normalized.push(catalog);
        }
    }
    normalized
}

impl StateStore {
    pub(crate) fn load_panel_layout(&self) -> Result<Option<PanelLayout>, String> {
        self.load_meta_value("panel_layout", "panel layout")?
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|error| format!("decode panel layout: {error}"))
            })
            .transpose()
    }

    pub(crate) fn save_panel_layout(&self, layout: &PanelLayout) -> Result<(), String> {
        let json = serde_json::to_string(layout)
            .map_err(|error| format!("encode panel layout: {error}"))?;
        self.save_meta_value("panel_layout", &json, "panel layout")
    }

    pub(crate) fn load_theme_css(&self) -> Result<Option<String>, String> {
        self.load_meta_value("theme_css", "themes")
    }

    pub(crate) fn save_theme_css(&self, css: &str) -> Result<(), String> {
        self.save_meta_value("theme_css", css, "themes")
    }

    pub(crate) fn load_active_theme(&self) -> Result<Option<String>, String> {
        self.load_meta_value("theme_selected", "active theme")
    }

    pub(crate) fn save_active_theme(&self, name: &str) -> Result<(), String> {
        self.save_meta_value("theme_selected", name, "active theme")
    }

    fn load_meta_value(&self, key: &str, subject: &str) -> Result<Option<String>, String> {
        self.connection
            .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| format!("load {subject}: {error}"))
    }

    fn save_meta_value(&self, key: &str, value: &str, subject: &str) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO meta(key,value) VALUES(?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|error| format!("save {subject}: {error}"))
    }

    pub(crate) fn load_session_folders(&self) -> Result<crate::sessions::SessionFolders, String> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM meta WHERE key='session_folders'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("load session folders: {error}"))?;
        json.map(|json| {
            serde_json::from_str(&json).map_err(|error| format!("decode session folders: {error}"))
        })
        .transpose()
        .map(Option::unwrap_or_default)
    }

    pub(crate) fn save_session_folders(
        &self,
        folders: &crate::sessions::SessionFolders,
    ) -> Result<(), String> {
        let json = serde_json::to_string(folders)
            .map_err(|error| format!("encode session folders: {error}"))?;
        self.connection.execute(
            "INSERT INTO meta(key,value) VALUES('session_folders',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json]
        ).map_err(|error| format!("save session folders: {error}"))?;
        Ok(())
    }
}
