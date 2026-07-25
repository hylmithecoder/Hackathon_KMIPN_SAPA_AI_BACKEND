//! In-process WhatsApp Web session for the CRM (foundation-wide sender).
//!
//! Backed by `whatsapp-rust`. The registry owns a single foundation session that
//! can be paired via QR code and used to send text messages to leads/contacts.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use mysql::params;
use mysql::prelude::*;

use whatsapp_rust::wacore_binary::JidExt;
use whatsapp_rust::Client;
use whatsapp_rust::Jid;
use whatsapp_rust::NodeFilter;
use whatsapp_rust::TokioRuntime;
use whatsapp_rust::bot::Bot;
use whatsapp_rust::media::{self, DocumentOptions, ImageOptions, VideoOptions};
use whatsapp_rust::send::SendOptions;
use whatsapp_rust::store::SqliteStore;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

use wacore::types::events::Event;
use wacore::types::presence::ReceiptType;
use wacore::download::MediaType;

use crate::ws::event::ChangeAction;
use crate::ws::Broadcaster;
use crate::{log_err, log_info, log_warn};

pub mod formatmessage;

pub use formatmessage::normalize_phone;

const WA_STORE_DIR: &str = "storage/whatsapp";
const UPLOAD_DIR: &str = "storage/uploads";
const MAX_OUTBOUND_MEDIA_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutboundMediaKind {
    Image,
    Video,
    Document,
}

fn uploaded_media_path(media_url: &str) -> Result<PathBuf, String> {
    let relative = media_url
        .strip_prefix("/uploads/")
        .ok_or_else(|| "media_url must reference /uploads/<file>".to_string())?;
    let mut components = Path::new(relative).components();
    let file_name = match (components.next(), components.next()) {
        (Some(Component::Normal(file_name)), None) => file_name,
        _ => return Err("invalid media_url path".to_string()),
    };
    Ok(Path::new(UPLOAD_DIR).join(file_name))
}

fn outbound_media_kind(path: &Path) -> (OutboundMediaKind, MediaType, &'static str) {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => (OutboundMediaKind::Image, MediaType::Image, "image/jpeg"),
        "png" => (OutboundMediaKind::Image, MediaType::Image, "image/png"),
        "gif" => (OutboundMediaKind::Image, MediaType::Image, "image/gif"),
        "webp" => (OutboundMediaKind::Image, MediaType::Image, "image/webp"),
        "mp4" => (OutboundMediaKind::Video, MediaType::Video, "video/mp4"),
        "mov" => (OutboundMediaKind::Video, MediaType::Video, "video/quicktime"),
        "pdf" => (OutboundMediaKind::Document, MediaType::Document, "application/pdf"),
        "doc" => (OutboundMediaKind::Document, MediaType::Document, "application/msword"),
        "docx" => (
            OutboundMediaKind::Document,
            MediaType::Document,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        _ => (
            OutboundMediaKind::Document,
            MediaType::Document,
            "application/octet-stream",
        ),
    }
}

struct WaInner {
    status: Mutex<String>,
    qr_code: Mutex<Option<String>>,
    number: Mutex<Option<String>>,
    client: Mutex<Option<Arc<Client>>>,
    running: Mutex<bool>,
    pool: mysql::Pool,
    broadcaster: Broadcaster,
}

impl WaInner {
    fn new(pool: mysql::Pool, broadcaster: Broadcaster) -> Self {
        WaInner {
            status: Mutex::new("disconnected".to_string()),
            qr_code: Mutex::new(None),
            number: Mutex::new(None),
            client: Mutex::new(None),
            running: Mutex::new(false),
            pool,
            broadcaster,
        }
    }

    fn persist(&self, status: &str, number: Option<&str>, set_paired_at: bool) {
        if let Ok(mut conn) = self.pool.get_conn() {
            let paired_expr = if set_paired_at {
                "NOW()"
            } else {
                "wa_paired_at"
            };
            let sql = format!(
                "UPDATE whatsapp_sessions SET wa_status = :status, sender_number = :number, \
                 wa_paired_at = {paired_expr} WHERE id = (SELECT id FROM (SELECT id FROM whatsapp_sessions ORDER BY id LIMIT 1) AS s)"
            );
            if let Err(e) = conn.exec_drop(
                sql,
                params! {
                    "status" => status,
                    "number" => number,
                },
            ) {
                crate::log_err!("Failed to update WhatsApp session state: {e}");
            } else {
                self.broadcaster
                    .notify("whatsapp_session", ChangeAction::Updated, None);
            }
        }
    }

    fn persist_message(
        &self,
        phone: &str,
        wa_message_id: &str,
        text: &str,
        sender_name: &str,
        direction: &str,
    ) {
        if text.trim().is_empty() {
            return;
        }

        let Ok(mut conn) = self.pool.get_conn() else {
            crate::log_err!("WA message persist: failed to get DB connection");
            return;
        };

        let session_id: Option<u64> = match conn
            .query_first("SELECT id FROM whatsapp_sessions ORDER BY id LIMIT 1")
        {
            Ok(id) => id,
            Err(e) => {
                crate::log_err!("WA message persist: failed to resolve session: {e}");
                return;
            }
        };
        let Some(session_id) = session_id else {
            crate::log_err!("WA message persist: no WhatsApp session record");
            return;
        };

        // Find contact by phone; strip non-digit characters for matching.
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let contact: Option<(u64, Option<u64>)> = match conn.exec_first(
            "SELECT id, company_id FROM contacts WHERE REPLACE(REPLACE(REPLACE(phone, '+', ''), '-', ''), ' ', '') = :digits OR phone = :phone ORDER BY id LIMIT 1",
            params! { "digits" => &digits, "phone" => phone },
        ) {
            Ok(c) => c,
            Err(e) => {
                crate::log_err!("WA message persist: failed to lookup contact: {e}");
                None
            }
        };

        let (contact_id, deal_id) = match contact {
            Some((cid, _company_id)) => {
                // Contact exists; pick the most recent deal or auto-create a deal if none exists
                let deal: Option<u64> = match conn.exec_first(
                    "SELECT id FROM deals WHERE contact_id = :cid ORDER BY id DESC LIMIT 1",
                    params! { "cid" => cid },
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        crate::log_err!("WA message persist: failed to lookup deal: {e}");
                        None
                    }
                };

                let deal_id = match deal {
                    Some(did) => did,
                    None => {
                        let deal_title = if !sender_name.trim().is_empty() {
                            format!("Deal - {sender_name}")
                        } else {
                            format!("Deal - {phone}")
                        };
                        if let Err(e) = conn.exec_drop(
                            "INSERT INTO deals (title, contact_id, stage_id, owner_id, value, currency, expected_close_date, status, description) \
                             VALUES (:title, :cid, 1, 1, 0.0, 'IDR', CURRENT_DATE, 'Open', 'Auto-created from WhatsApp chat')",
                            params! {
                                "title" => &deal_title,
                                "cid" => cid,
                            },
                        ) {
                            crate::log_err!("WA auto-resolve: failed to auto-create deal: {e}");
                            0
                        } else {
                            let new_did = conn.last_insert_id();
                            self.broadcaster.notify("deal", ChangeAction::Created, Some(new_did));
                            new_did
                        }
                    }
                };
                (cid, if deal_id > 0 { Some(deal_id) } else { None })
            }
            None => {
                // Contact does not exist; auto-create Contact AND Deal!
                let contact_name = if !sender_name.trim().is_empty() {
                    sender_name.trim()
                } else {
                    phone
                };
                let new_cid = if let Err(e) = conn.exec_drop(
                    "INSERT INTO contacts (first_name, last_name, phone, status, source) \
                     VALUES (:name, '', :phone, 'Lead', 'whatsapp')",
                    params! {
                        "name" => contact_name,
                        "phone" => phone,
                    },
                ) {
                    crate::log_err!("WA auto-resolve: failed to auto-create contact: {e}");
                    0
                } else {
                    let cid = conn.last_insert_id();
                    self.broadcaster.notify("contact", ChangeAction::Created, Some(cid));
                    cid
                };

                let deal_id = if new_cid > 0 {
                    let deal_title = format!("Deal - {contact_name}");
                    if let Err(e) = conn.exec_drop(
                        "INSERT INTO deals (title, contact_id, stage_id, owner_id, value, currency, expected_close_date, status, description) \
                         VALUES (:title, :cid, 1, 1, 0.0, 'IDR', CURRENT_DATE, 'Open', 'Auto-created from WhatsApp chat')",
                        params! {
                            "title" => &deal_title,
                            "cid" => new_cid,
                        },
                    ) {
                        crate::log_err!("WA auto-resolve: failed to auto-create deal: {e}");
                        None
                    } else {
                        let did = conn.last_insert_id();
                        self.broadcaster.notify("deal", ChangeAction::Created, Some(did));
                        Some(did)
                    }
                } else {
                    None
                };

                (new_cid, deal_id)
            }
        };

        let status_val = if direction == "inbound" { "delivered" } else { "sent" };

        if let Err(e) = conn.exec_drop(
            "INSERT INTO whatsapp_messages (session_id, deal_id, contact_id, phone, direction, message, wa_message_id, sender_name, status, sent_at) \
             VALUES (:session_id, :deal_id, :contact_id, :phone, :direction, :message, :wa_message_id, :sender_name, :status, NOW())",
            params! {
                "session_id" => session_id,
                "deal_id" => deal_id,
                "contact_id" => if contact_id > 0 { Some(contact_id) } else { None },
                "phone" => phone,
                "direction" => direction,
                "message" => text,
                "wa_message_id" => wa_message_id,
                "sender_name" => sender_name,
                "status" => status_val,
            },
        ) {
            crate::log_err!("WA message persist: failed to insert: {e}");
        } else {
            let msg_id = conn.last_insert_id();
            self.broadcaster.notify("whatsapp_message", ChangeAction::Created, Some(msg_id));
            crate::log_info!("WA message persisted: {direction} from/to {phone} (deal_id={deal_id:?}, msg_id={msg_id})");
        }
    }
}

#[derive(Clone)]
pub struct WaSession {
    inner: Arc<WaInner>,
}

impl WaSession {
    pub fn new(pool: mysql::Pool, broadcaster: Broadcaster) -> Self {
        Self {
            inner: Arc::new(WaInner::new(pool, broadcaster)),
        }
    }

    pub fn has_store(&self) -> bool {
        std::path::Path::new(&format!("{WA_STORE_DIR}/whatsapp.db")).exists()
    }

    pub async fn status(&self) -> String {
        self.inner.status.lock().await.clone()
    }

    pub async fn qr_code(&self) -> Option<String> {
        self.inner.qr_code.lock().await.clone()
    }

    pub async fn number(&self) -> Option<String> {
        self.inner.number.lock().await.clone()
    }

    pub async fn connect(&self) -> Result<(), String> {
        {
            let mut running = self.inner.running.lock().await;
            if *running {
                return Ok(());
            }
            *running = true;
        }

        *self.inner.status.lock().await = "pairing".to_string();
        *self.inner.qr_code.lock().await = None;
        self.inner.persist("pairing", None, false);

        if let Err(e) = std::fs::create_dir_all(WA_STORE_DIR) {
            self.reset_after_failure().await;
            return Err(format!("failed to create storage dir: {e}"));
        }

        let store_path = format!("{WA_STORE_DIR}/whatsapp.db");
        let backend = match SqliteStore::new(&store_path).await {
            Ok(b) => b,
            Err(e) => {
                self.reset_after_failure().await;
                return Err(format!("failed to open WhatsApp store: {e}"));
            }
        };

        let inner_events = self.inner.clone();
        let build = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(UreqHttpClient::new())
            .with_runtime(TokioRuntime)
            .on_event(move |event, client| {
                let inner = inner_events.clone();
                async move {
                    match &*event {
                        Event::PairingQrCode(qr) => {
                            *inner.qr_code.lock().await = Some(qr.code.clone());
                            *inner.status.lock().await = "pairing".to_string();
                        }
                        Event::PairSuccess(pair) => {
                            let num = pair.id.user.to_string();
                            *inner.number.lock().await = Some(num.clone());
                            inner.persist("pairing", Some(&num), false);
                        }
                        Event::Connected(_) => {
                            *inner.status.lock().await = "connected".to_string();
                            *inner.qr_code.lock().await = None;

                            let mut num = inner.number.lock().await.clone();
                            if num.is_none()
                                && let Some(jid) = client.get_pn()
                            {
                                let pn = jid.user.to_string();
                                *inner.number.lock().await = Some(pn.clone());
                                num = Some(pn);
                            }
                            inner.persist("connected", num.as_deref(), true);
                        }
                        Event::LoggedOut(reason) => {
                            log_warn!("WA event: LoggedOut — {reason:?}");
                            *inner.status.lock().await = "disconnected".to_string();
                            *inner.number.lock().await = None;
                            inner.persist("disconnected", None, false);
                        }
                        Event::Receipt(receipt) => {
                            let ids = receipt.message_ids.join(", ");
                            match receipt.r#type {
                                ReceiptType::ServerError => {
                                    log_err!(
                                        "WA receipt: SERVER ERROR from {} for [{ids}]",
                                        receipt.source.chat
                                    );
                                }
                                _ => {
                                    log_info!(
                                        "WA receipt: {:?} from {} for [{ids}]",
                                        receipt.r#type,
                                        receipt.source.chat
                                    );
                                }
                            }
                        }
                        Event::Disconnected(d) => {
                            log_warn!("WA event: Disconnected — {d:?}");
                        }
                        Event::ClientOutdated(_) => {
                            log_err!("WA event: ClientOutdated");
                        }
                        Event::PairError(e) => {
                            log_err!("WA event: PairError — {e:?}");
                        }
                        Event::Messages(batch) => {
                            for msg in batch.iter() {
                                let direction = if msg.info.source.is_from_me { "outbound" } else { "inbound" };
                                let chat = &msg.info.source.chat;
                                // Only process 1-on-1 personal user chats (@s.whatsapp.net or @lid)
                                if chat.is_group()
                                    || chat.is_broadcast_list()
                                    || chat.is_status_broadcast()
                                    || chat.is_bot()
                                    || chat.server == whatsapp_rust::Server::Newsletter
                                {
                                    continue;
                                }
                                let phone = chat.user.clone();
                                let text = if let Some(c) = &msg.message.conversation {
                                    c.clone()
                                } else if let Some(e) = msg.message.extended_text_message.as_option() {
                                    e.text.clone().unwrap_or_default()
                                } else if let Some(i) = msg.message.image_message.as_option() {
                                    i.caption.clone().unwrap_or_else(|| "[Gambar]".to_string())
                                } else if let Some(v) = msg.message.video_message.as_option() {
                                    v.caption.clone().unwrap_or_else(|| "[Video]".to_string())
                                } else if msg.message.sticker_message.is_set() {
                                    "[Stiker]".to_string()
                                } else if let Some(d) = msg.message.document_message.as_option() {
                                    d.caption.clone().unwrap_or_else(|| "[Dokumen]".to_string())
                                } else {
                                    String::new()
                                };

                                if text.is_empty() {
                                    continue;
                                }
                                let wa_message_id = msg.info.id.to_string();
                                let sender_name = msg.info.push_name.as_str();
                                inner.persist_message(&phone, &wa_message_id, &text, sender_name, direction);
                            }
                        }
                        other => {
                            log_info!("WA event: {:?}", other.kind());
                        }
                    }
                }
            })
            .build()
            .await;

        let bot = match build {
            Ok(b) => b,
            Err(e) => {
                self.reset_after_failure().await;
                return Err(format!("failed to build WhatsApp bot: {e}"));
            }
        };

        *self.inner.client.lock().await = Some(bot.client());

        let inner_run = self.inner.clone();
        tokio::spawn(async move {
            bot.run().await;
            *inner_run.status.lock().await = "disconnected".to_string();
            *inner_run.qr_code.lock().await = None;
            *inner_run.client.lock().await = None;
            *inner_run.running.lock().await = false;
            inner_run.persist("disconnected", None, false);
        });

        Ok(())
    }

    async fn reset_after_failure(&self) {
        *self.inner.status.lock().await = "disconnected".to_string();
        *self.inner.running.lock().await = false;
        self.inner.persist("disconnected", None, false);
    }

    async fn ready_client(&self) -> Result<Arc<Client>, String> {
        let client = self
            .inner
            .client
            .lock()
            .await
            .clone()
            .ok_or_else(|| "WhatsApp is not connected".to_string())?;

        if !client.is_connected() {
            return Err("WhatsApp session is not connected".to_string());
        }
        if !client.is_logged_in() {
            return Err("WhatsApp session is not logged in — please re-scan the QR".to_string());
        }

        Ok(client)
    }

    async fn resolve_recipient(&self, client: &Client, phone: &str) -> Result<Jid, String> {
        let digits =
            normalize_phone(phone).ok_or_else(|| format!("invalid phone number: {phone}"))?;

        // Inbound chats can be identified by a WhatsApp LID rather than a
        // public phone number. Older CRM contacts stored that bare LID in the
        // phone field, so detect the reverse mapping before treating the value
        // as a PN. Sending a LID as `@s.whatsapp.net` is rejected with 406.
        let stored_lid = Jid::lid(&digits);
        if let Ok(Some(entry)) = client.get_lid_pn_entry(&stored_lid).await {
            log_info!(
                "WA resolve: stored identifier {} is LID for phone {}",
                entry.lid,
                entry.phone_number
            );
            return Ok(Jid::lid(entry.lid.as_ref()));
        }

        let pn = Jid::pn(&digits);
        match client.get_lid_pn_entry(&pn).await {
            Ok(Some(entry)) => Ok(Jid::lid(entry.lid.as_ref())),
            Ok(None) => Ok(Jid::pn(digits)),
            Err(e) => {
                log_warn!("WA resolve: LID lookup failed ({e}); using User JID");
                Ok(Jid::pn(digits))
            }
        }
    }

    async fn alternate_recipient(client: &Client, target: &Jid) -> Option<Jid> {
        let entry = client.get_lid_pn_entry(target).await.ok().flatten()?;
        match target.server {
            whatsapp_rust::Server::Pn => Some(Jid::lid(entry.lid.as_ref())),
            whatsapp_rust::Server::Lid => Some(Jid::pn(entry.phone_number.as_ref())),
            _ => None,
        }
    }

    async fn send_and_confirm(
        &self,
        client: &Arc<Client>,
        jid: Jid,
        message: whatsapp_rust::waproto::whatsapp::Message,
    ) -> Result<String, String> {
        let mut target = jid;
        for attempt in 0..2 {
            let message_id = client.generate_message_id();
            let ack_rx =
                client.wait_for_node(NodeFilter::tag("ack").attr("id", message_id.clone()));

            let opts = SendOptions {
                message_id: Some(message_id.clone()),
                ..Default::default()
            };

            log_info!(
                "WA send: -> {target} id={message_id} (attempt {})",
                attempt + 1
            );

            if let Err(e) = client
                .send_message_with_options(target.clone(), message.clone(), opts)
                .await
            {
                log_err!("WA send: -> {target} id={message_id} write failed: {e}");
                if attempt == 0
                    && let Some(alternate) = Self::alternate_recipient(client, &target).await
                {
                    log_info!("WA send: retrying write via alternate JID {alternate}");
                    target = alternate;
                    continue;
                }
                return Err(format!("failed to send WhatsApp message: {e}"));
            }

            let ack_error: Option<String> =
                match tokio::time::timeout(std::time::Duration::from_secs(15), ack_rx).await {
                    Ok(Ok(node)) => node
                        .get()
                        .get_attr("error")
                        .map(|v| v.as_str().into_owned()),
                    Ok(Err(_)) | Err(_) => {
                        log_warn!("WA send: no ack for {message_id} within 15s");
                        Some("timeout".into())
                    }
                };

            match ack_error {
                None => {
                    log_info!("WA send: {message_id} accepted");
                    return Ok(message_id);
                }
                Some(code) => {
                    log_err!("WA send: {message_id} rejected (error={code})");
                    if attempt == 0
                        && let Some(alternate) = Self::alternate_recipient(client, &target).await
                    {
                        log_info!("WA send: retrying ack via alternate JID {alternate}");
                        target = alternate;
                        continue;
                    }
                    return Err(format!(
                        "WhatsApp delivery rejected or timed out (code {code}). Ensure recipient is active."
                    ));
                }
            }
        }
        unreachable!("send loop exits by return");
    }

    pub async fn send_text(&self, phone: &str, text: &str) -> Result<String, String> {
        let client = self.ready_client().await?;
        let jid = self.resolve_recipient(&client, phone).await?;

        let message = whatsapp_rust::waproto::whatsapp::Message {
            conversation: Some(text.to_string()),
            ..Default::default()
        };

        self.send_and_confirm(&client, jid, message).await
    }

    pub async fn send_media(
        &self,
        phone: &str,
        caption: &str,
        media_url: &str,
        original_file_name: Option<&str>,
    ) -> Result<String, String> {
        let path = uploaded_media_path(media_url)?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|e| format!("uploaded media is unavailable: {e}"))?;
        if !metadata.is_file() {
            return Err("uploaded media path is not a file".to_string());
        }
        if metadata.len() > MAX_OUTBOUND_MEDIA_BYTES {
            return Err(format!(
                "uploaded media exceeds the {} MB outbound limit",
                MAX_OUTBOUND_MEDIA_BYTES / 1024 / 1024
            ));
        }

        let data = tokio::fs::read(&path)
            .await
            .map_err(|e| format!("failed to read uploaded media: {e}"))?;
        let client = self.ready_client().await?;
        let jid = self.resolve_recipient(&client, phone).await?;
        let (kind, media_type, mimetype) = outbound_media_kind(&path);
        let upload = client
            .upload(data, media_type, Default::default())
            .await
            .map_err(|e| format!("failed to upload media to WhatsApp: {e}"))?;
        let caption = (!caption.trim().is_empty()).then(|| caption.trim().to_string());

        let message = match kind {
            OutboundMediaKind::Image => media::image_message(
                upload,
                ImageOptions {
                    caption,
                    mimetype: Some(mimetype.to_string()),
                    ..Default::default()
                },
            ),
            OutboundMediaKind::Video => media::video_message(
                upload,
                VideoOptions {
                    caption,
                    mimetype: Some(mimetype.to_string()),
                    ..Default::default()
                },
            ),
            OutboundMediaKind::Document => media::document_message(
                upload,
                DocumentOptions {
                    caption,
                    mimetype: Some(mimetype.to_string()),
                    file_name: original_file_name
                        .filter(|name| !name.trim().is_empty())
                        .map(str::to_string)
                        .or_else(|| {
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .map(str::to_string)
                        }),
                    ..Default::default()
                },
            ),
        };

        self.send_and_confirm(&client, jid, message).await
    }

    pub async fn logout(&self) {
        if let Some(client) = self.inner.client.lock().await.clone() {
            let _ = client.logout().await;
        }
        *self.inner.status.lock().await = "disconnected".to_string();
        *self.inner.qr_code.lock().await = None;
        *self.inner.number.lock().await = None;
        *self.inner.client.lock().await = None;
        *self.inner.running.lock().await = false;
        self.inner.persist("disconnected", None, false);

        let store_path = format!("{WA_STORE_DIR}/whatsapp.db");
        if std::path::Path::new(&store_path).exists() {
            let _ = std::fs::remove_file(&store_path);
        }
    }

    pub async fn restore(&self) {
        if self.has_store() {
            log_info!("Auto-connecting WhatsApp session on startup...");
            if let Err(e) = self.connect().await {
                log_err!("Failed to auto-connect WhatsApp: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploaded_media_path_accepts_one_upload_file() {
        assert_eq!(
            uploaded_media_path("/uploads/photo.jpg").unwrap(),
            PathBuf::from("storage/uploads/photo.jpg")
        );
    }

    #[test]
    fn uploaded_media_path_rejects_traversal_and_external_urls() {
        assert!(uploaded_media_path("/uploads/../secret.env").is_err());
        assert!(uploaded_media_path("https://example.com/photo.jpg").is_err());
    }

    #[test]
    fn outbound_media_kind_detects_images_and_documents() {
        assert_eq!(
            outbound_media_kind(Path::new("photo.PNG")).0,
            OutboundMediaKind::Image
        );
        assert_eq!(
            outbound_media_kind(Path::new("proposal.pdf")).0,
            OutboundMediaKind::Document
        );
    }
}

#[derive(Clone)]
pub struct WaRegistry {
    foundation: WaSession,
}

impl WaRegistry {
    pub fn new(pool: mysql::Pool, broadcaster: Broadcaster) -> Self {
        Self {
            foundation: WaSession::new(pool, broadcaster),
        }
    }

    pub fn foundation(&self) -> WaSession {
        self.foundation.clone()
    }

    pub async fn restore_all(&self) {
        self.foundation.restore().await;
    }
}
