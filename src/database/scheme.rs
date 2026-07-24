use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub full_name: String,
    pub role: String,
    pub email: String,
    pub phone: Option<String>,
    pub photo_url: Option<String>,
    pub is_active: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Company {
    pub id: u64,
    pub name: String,
    pub industry: Option<String>,
    pub website: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub description: Option<String>,
    pub assigned_to: Option<u64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Contact {
    pub id: u64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub company_id: Option<u64>,
    pub company_name: Option<String>,
    pub source: Option<String>,
    pub status: String,
    pub assigned_to: Option<u64>,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DealStage {
    pub id: u64,
    pub name: String,
    pub position: i32,
    pub probability: f64,
    pub color: Option<String>,
    pub is_active: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Deal {
    pub id: u64,
    pub title: String,
    pub contact_id: Option<u64>,
    pub contact_name: Option<String>,
    pub company_id: Option<u64>,
    pub company_name: Option<String>,
    pub stage_id: u64,
    pub stage_name: Option<String>,
    pub owner_id: Option<u64>,
    pub owner_name: Option<String>,
    pub value: f64,
    pub currency: String,
    pub expected_close_date: Option<String>,
    pub actual_close_date: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DealDetail {
    pub id: u64,
    pub title: String,
    pub contact: Option<Contact>,
    pub company: Option<Company>,
    pub stage_id: u64,
    pub stage_name: Option<String>,
    pub owner_id: Option<u64>,
    pub owner_name: Option<String>,
    pub value: f64,
    pub currency: String,
    pub expected_close_date: Option<String>,
    pub actual_close_date: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscussionFile {
    pub id: u64,
    pub discussion_id: u64,
    pub file_name: String,
    pub file_url: String,
    pub mime_type: Option<String>,
    pub file_size: u64,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DealDiscussion {
    pub id: u64,
    pub deal_id: u64,
    pub user_id: Option<u64>,
    pub author_name: Option<String>,
    pub content: String,
    pub files: Vec<DiscussionFile>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Activity {
    pub id: u64,
    pub activity_type: String,
    pub subject: String,
    pub description: Option<String>,
    pub contact_id: Option<u64>,
    pub contact_name: Option<String>,
    pub deal_id: Option<u64>,
    pub deal_title: Option<String>,
    pub company_id: Option<u64>,
    pub company_name: Option<String>,
    pub assigned_to: Option<u64>,
    pub assigned_name: Option<String>,
    pub due_date: Option<String>,
    pub completed_at: Option<String>,
    pub status: String,
    pub created_by: Option<u64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Note {
    pub id: u64,
    pub content: String,
    pub contact_id: Option<u64>,
    pub deal_id: Option<u64>,
    pub company_id: Option<u64>,
    pub created_by: Option<u64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    pub id: u64,
    pub name: String,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub unit_price: f64,
    pub currency: String,
    pub is_active: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Quote {
    pub id: u64,
    pub deal_id: u64,
    pub quote_number: String,
    pub issue_date: String,
    pub expiry_date: Option<String>,
    pub subtotal: f64,
    pub tax_rate: f64,
    pub tax_amount: f64,
    pub total_amount: f64,
    pub currency: String,
    pub status: String,
    pub notes: Option<String>,
    pub created_by: Option<u64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuoteItem {
    pub id: u64,
    pub quote_id: u64,
    pub product_id: Option<u64>,
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub discount: f64,
    pub total: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ticket {
    pub id: u64,
    pub ticket_number: String,
    pub subject: String,
    pub description: String,
    pub contact_id: Option<u64>,
    pub contact_name: Option<String>,
    pub company_id: Option<u64>,
    pub company_name: Option<String>,
    pub assigned_to: Option<u64>,
    pub assigned_name: Option<String>,
    pub priority: String,
    pub status: String,
    pub source: Option<String>,
    pub resolved_at: Option<String>,
    pub closed_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Campaign {
    pub id: u64,
    pub name: String,
    pub campaign_type: String,
    pub status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub budget: Option<f64>,
    pub currency: String,
    pub target_audience: Option<String>,
    pub message_template: Option<String>,
    pub sent_count: u64,
    pub delivered_count: u64,
    pub responded_count: u64,
    pub created_by: Option<u64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tag {
    pub id: u64,
    pub name: String,
    pub color: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: u64,
    pub user_id: u64,
    pub title: String,
    pub body: String,
    pub category: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<u64>,
    pub is_read: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhatsappSession {
    pub id: u64,
    pub name: String,
    pub sender_number: Option<String>,
    pub wa_status: String,
    pub wa_qr: Option<String>,
    pub wa_paired_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhatsappMessage {
    pub id: u64,
    pub session_id: u64,
    pub deal_id: Option<u64>,
    pub contact_id: Option<u64>,
    pub phone: String,
    pub direction: String,
    pub message: String,
    pub wa_message_id: Option<String>,
    pub sender_name: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: Option<String>,
}
