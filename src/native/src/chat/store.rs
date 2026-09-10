//! Chat + actions persistence, scoped per user.
//!
//! Owns a second SQLite connection to the same file as auth: local traffic
//! is tiny and SQLite serializes writers, so domains stay decoupled without
//! a shared pool. Revisit only if contention ever shows up (it won't locally).
use rusqlite::{params, Connection};

use super::model::{Action, ChatMessageRow, Selection, Session, SessionSummary};
use crate::platform::db;

pub struct ChatStore {
    conn: Connection,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn random_id() -> String {
    use rand::{rngs::OsRng, RngCore};
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

impl ChatStore {
    pub fn open(db_path: &str) -> rusqlite::Result<Self> {
        let conn = db::connect(db_path)?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self { conn })
    }

    pub fn create_session(
        &self,
        user_id: &str,
        title: &str,
        provider: &str,
        model: Option<&str>,
    ) -> rusqlite::Result<Session> {
        let id = random_id();
        let ts = now();
        self.conn.execute(
            "INSERT INTO chat_sessions(id, user_id, title, provider, model, created_at, updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?6)",
            params![id, user_id, title, provider, model, ts],
        )?;
        Ok(Session {
            id,
            title: title.to_string(),
            provider: provider.to_string(),
            model: model.map(str::to_string),
            created_at: ts.clone(),
            updated_at: ts,
        })
    }

    pub fn get_session(&self, id: &str, user_id: &str) -> rusqlite::Result<Option<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, provider, model, created_at, updated_at
             FROM chat_sessions WHERE id = ?1 AND user_id = ?2",
        )?;
        let mut rows = stmt.query([id, user_id])?;
        match rows.next()? {
            Some(r) => Ok(Some(Session {
                id: r.get(0)?,
                title: r.get(1)?,
                provider: r.get(2)?,
                model: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })),
            None => Ok(None),
        }
    }

    pub fn list_sessions(&self, user_id: &str) -> rusqlite::Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.title, s.provider, s.model, s.updated_at,
               (SELECT m.content FROM chat_messages m
                WHERE m.session_id = s.id ORDER BY m.created_at DESC, m.rowid DESC LIMIT 1),
               (SELECT COUNT(*) FROM chat_messages m WHERE m.session_id = s.id)
             FROM chat_sessions s WHERE s.user_id = ?1
             ORDER BY s.updated_at DESC, s.rowid DESC LIMIT 100",
        )?;
        let rows = stmt.query_map([user_id], |r| {
            Ok(SessionSummary {
                id: r.get(0)?,
                title: r.get(1)?,
                provider: r.get(2)?,
                model: r.get(3)?,
                updated_at: r.get(4)?,
                preview: r.get(5)?,
                message_count: r.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn touch_session(
        &self,
        id: &str,
        provider: &str,
        model: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE chat_sessions SET provider = ?1, model = ?2, updated_at = ?3 WHERE id = ?4",
            params![provider, model, now(), id],
        )?;
        Ok(())
    }

    /// Deletes only what the user owns; messages + actions go via CASCADE.
    pub fn delete_session(&self, id: &str, user_id: &str) -> rusqlite::Result<bool> {
        let gone: Result<String, _> = self.conn.query_row(
            "DELETE FROM chat_sessions WHERE id = ?1 AND user_id = ?2 RETURNING id",
            [id, user_id],
            |r| r.get(0),
        );
        match gone {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn add_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
    ) -> rusqlite::Result<ChatMessageRow> {
        let id = random_id();
        let ts = now();
        self.conn.execute(
            "INSERT INTO chat_messages(id, session_id, role, content, created_at)
             VALUES(?1,?2,?3,?4,?5)",
            params![id, session_id, role, content, ts],
        )?;
        Ok(ChatMessageRow {
            id,
            role: role.to_string(),
            content: content.to_string(),
            created_at: ts,
        })
    }

    pub fn list_messages(&self, session_id: &str) -> rusqlite::Result<Vec<ChatMessageRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, role, content, created_at FROM chat_messages
             WHERE session_id = ?1 ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], |r| {
            Ok(ChatMessageRow {
                id: r.get(0)?,
                role: r.get(1)?,
                content: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn create_action(
        &self,
        session_id: Option<&str>,
        user_id: &str,
        kind: &str,
        title: &str,
    ) -> rusqlite::Result<Action> {
        let id = random_id();
        let ts = now();
        self.conn.execute(
            "INSERT INTO actions(id, session_id, user_id, kind, title, status, created_at, updated_at)
             VALUES(?1,?2,?3,?4,?5,'running',?6,?6)",
            params![id, session_id, user_id, kind, title, ts],
        )?;
        Ok(Action {
            id,
            session_id: session_id.map(str::to_string),
            kind: kind.to_string(),
            title: title.to_string(),
            status: "running".into(),
            created_at: ts.clone(),
            updated_at: ts,
        })
    }

    /// Scoped update for the API: only the owner's rows move. Returns false
    /// when the action doesn't exist or belongs to someone else.
    pub fn set_action_status_owned(
        &self,
        id: &str,
        user_id: &str,
        status: &str,
    ) -> rusqlite::Result<bool> {
        let gone: Result<String, _> = self.conn.query_row(
            "UPDATE actions SET status = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4 RETURNING id",
            params![status, now(), id, user_id],
            |r| r.get(0),
        );
        match gone {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn get_action(&self, id: &str, user_id: &str) -> rusqlite::Result<Option<Action>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, kind, title, status, created_at, updated_at
             FROM actions WHERE id = ?1 AND user_id = ?2",
        )?;
        let mut rows = stmt.query([id, user_id])?;
        match rows.next()? {
            Some(r) => Ok(Some(row_to_action(r)?)),
            None => Ok(None),
        }
    }

    pub fn list_actions_by_session(&self, session_id: &str) -> rusqlite::Result<Vec<Action>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, kind, title, status, created_at, updated_at
             FROM actions WHERE session_id = ?1 ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], row_to_action)?;
        rows.collect()
    }

    pub fn list_recent_actions(&self, user_id: &str, limit: i64) -> rusqlite::Result<Vec<Action>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, kind, title, status, created_at, updated_at
             FROM actions WHERE user_id = ?1
             ORDER BY created_at DESC, rowid DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![user_id, limit], row_to_action)?;
        rows.collect()
    }

    pub fn get_selection(&self, user_id: &str) -> rusqlite::Result<Option<Selection>> {
        let mut stmt = self
            .conn
            .prepare("SELECT provider, model FROM ai_selection WHERE user_id = ?1")?;
        let mut rows = stmt.query([user_id])?;
        match rows.next()? {
            Some(r) => Ok(Some(Selection {
                provider: r.get(0)?,
                model: r.get(1)?,
            })),
            None => Ok(None),
        }
    }

    pub fn upsert_selection(
        &self,
        user_id: &str,
        provider: &str,
        model: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO ai_selection(user_id, provider, model, updated_at)
             VALUES(?1,?2,?3,?4)
             ON CONFLICT(user_id) DO UPDATE SET provider = ?2, model = ?3, updated_at = ?4",
            params![user_id, provider, model, now()],
        )?;
        Ok(())
    }
}

fn row_to_action(r: &rusqlite::Row<'_>) -> rusqlite::Result<Action> {
    Ok(Action {
        id: r.get(0)?,
        session_id: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        status: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::store::Store as AuthStore;

    fn tmp_db() -> String {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "smartpc-test-{}-{}.db",
            std::process::id(),
            random_id()
        ));
        p.to_string_lossy().into_owned()
    }

    /// Mirrors production boot order: auth store first (owns users),
    /// then chat store on the same file.
    fn setup() -> (String, ChatStore, String) {
        let path = tmp_db();
        let auth = AuthStore::open(&path).unwrap();
        let user = auth
            .create_user(
                &random_id(),
                "u1@example.com",
                "hash",
                "2024-01-01T00:00:00Z",
            )
            .unwrap();
        let s = ChatStore::open(&path).unwrap();
        (path, s, user.id)
    }

    #[test]
    fn sessions_messages_actions_selection() {
        let (path, s, uid) = setup();

        assert!(s.get_selection(&uid).unwrap().is_none());
        s.upsert_selection(&uid, "ollama", Some("llama3.1:8b"))
            .unwrap();
        let sel = s.get_selection(&uid).unwrap().unwrap();
        assert_eq!(sel.provider, "ollama");
        assert_eq!(sel.model.as_deref(), Some("llama3.1:8b"));

        let a = s
            .create_session(&uid, "Hola mundo", "ollama", None)
            .unwrap();
        assert_eq!(s.list_sessions(&uid).unwrap().len(), 1);
        assert!(s.get_session(&a.id, "stranger").unwrap().is_none());

        s.add_message(&a.id, "user", "hola").unwrap();
        s.add_message(&a.id, "assistant", "buenas").unwrap();
        let msgs = s.list_messages(&a.id).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");

        let act = s
            .create_action(Some(&a.id), &uid, "open_app", "Abrir navegador")
            .unwrap();
        assert_eq!(act.status, "running");
        assert!(s.set_action_status_owned(&act.id, &uid, "done").unwrap());
        assert!(!s
            .set_action_status_owned(&act.id, "stranger", "done")
            .unwrap());
        assert_eq!(s.get_action(&act.id, &uid).unwrap().unwrap().status, "done");

        assert!(s.delete_session(&a.id, &uid).unwrap());
        assert!(!s.delete_session(&a.id, &uid).unwrap());
        assert!(s.list_messages(&a.id).unwrap().is_empty());

        std::fs::remove_file(&path).ok();
    }
}
