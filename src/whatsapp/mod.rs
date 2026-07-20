//! In-process WhatsApp Web session for the CRM (foundation-wide sender).
//!
//! Backed by `whatsapp-rust`. The registry owns a single foundation session that
//! can be paired via QR code and used to send text messages to leads/contacts.

use std::sync::Arc;
use tokio::sync::Mutex;

use mysql::params;
use mysql::prelude::*;

use whatsapp_rust::Client;
use whatsapp_rust::Jid;
use whatsapp_rust::NodeFilter;
use whatsapp_rust::TokioRuntime;
use whatsapp_rust::bot::Bot;
use whatsapp_rust::send::SendOptions;
use whatsapp_rust::store::SqliteStore;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

use wacore::types::events::Event;
use wacore::types::presence::ReceiptType;

use crate::{log_err, log_info, log_warn};

pub mod formatmessage;

pub use formatmessage::normalize_phone;

const WA_STORE_DIR: &str = "storage/whatsapp";

struct WaInner {
    status: Mutex<String>,
    qr_code: Mutex<Option<String>>,
    number: Mutex<Option<String>>,
    client: Mutex<Option<Arc<Client>>>,
    running: Mutex<bool>,
    pool: mysql::Pool,
}

impl WaInner {
    fn new(pool: mysql::Pool) -> Self {
        WaInner {
            status: Mutex::new("disconnected".to_string()),
            qr_code: Mutex::new(None),
            number: Mutex::new(None),
            client: Mutex::new(None),
            running: Mutex::new(false),
            pool,
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
            }
        }
    }
}

#[derive(Clone)]
pub struct WaSession {
    inner: Arc<WaInner>,
}

impl WaSession {
    pub fn new(pool: mysql::Pool) -> Self {
        Self {
            inner: Arc::new(WaInner::new(pool)),
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
                            if num.is_none() && let Some(jid) = client.get_pn() {
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
        let pn = Jid::pn(digits);
        match client.get_lid_pn_entry(&pn).await {
            Ok(Some(entry)) => Ok(Jid::lid(entry.lid.as_ref())),
            Ok(None) => Ok(pn),
            Err(e) => {
                log_warn!("WA resolve: LID lookup failed ({e}); using PN");
                Ok(pn)
            }
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

            client
                .send_message_with_options(target.clone(), message.clone(), opts)
                .await
                .map_err(|e| {
                    log_err!("WA send: -> {target} id={message_id} write failed: {e}");
                    format!("failed to send WhatsApp message: {e}")
                })?;

            let ack_error: Option<String> =
                match tokio::time::timeout(std::time::Duration::from_secs(15), ack_rx).await {
                    Ok(Ok(node)) => node
                        .get()
                        .get_attr("error")
                        .map(|v| v.as_str().into_owned()),
                    Ok(Err(_)) | Err(_) => {
                        log_warn!("WA send: no ack for {message_id} within 15s");
                        None
                    }
                };

            match ack_error {
                None => {
                    log_info!("WA send: {message_id} accepted");
                    return Ok(message_id);
                }
                Some(code) => {
                    log_err!("WA send: {message_id} rejected (error={code})");
                    if code == "400"
                        && attempt == 0
                        && target.server == whatsapp_rust::Server::Pn
                        && let Ok(Some(entry)) = client.get_lid_pn_entry(&target).await
                    {
                        log_info!("WA send: retrying via LID {}@lid", entry.lid);
                        target = Jid::lid(entry.lid.as_ref());
                        continue;
                    }
                    return Err(format!(
                        "WhatsApp rejected the message (code {code}). Ensure the number is registered."
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

#[derive(Clone)]
pub struct WaRegistry {
    foundation: WaSession,
}

impl WaRegistry {
    pub fn new(pool: mysql::Pool) -> Self {
        Self {
            foundation: WaSession::new(pool),
        }
    }

    pub fn foundation(&self) -> WaSession {
        self.foundation.clone()
    }

    pub async fn restore_all(&self) {
        self.foundation.restore().await;
    }
}
