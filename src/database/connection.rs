use mysql::Pool;
use mysql::params;
use mysql::prelude::*;

/// Idempotently add a column to an existing table. Needed because tables are
/// created with `CREATE TABLE IF NOT EXISTS`, which does not add new columns to
/// tables that already exist from an earlier version of the schema.
fn ensure_column(
    conn: &mut mysql::PooledConn,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), mysql::Error> {
    let exists: Option<u8> = conn.exec_first(
        "SELECT 1 FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = :t AND column_name = :c",
        params! { "t" => table, "c" => column },
    )?;
    if exists.is_none() {
        conn.query_drop(format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))?;
    }
    Ok(())
}

/// Idempotently make a column nullable and repoint its foreign key to
/// `ON DELETE SET NULL`. Used when relaxing a previous `NOT NULL` constraint.
fn ensure_column_nullable(
    conn: &mut mysql::PooledConn,
    table: &str,
    column: &str,
    definition: &str,
    ref_table: &str,
    ref_column: &str,
) -> Result<(), mysql::Error> {
    let nullable: Option<String> = conn.exec_first(
        "SELECT is_nullable FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = :t AND column_name = :c",
        params! { "t" => table, "c" => column },
    )?;

    if nullable.as_deref() != Some("YES") {
        // Find existing FK constraint name so we can drop it before modifying the column.
        let fk_name: Option<String> = conn.exec_first(
            "SELECT constraint_name FROM information_schema.key_column_usage \
             WHERE table_schema = DATABASE() AND table_name = :t AND column_name = :c \
             AND referenced_table_name IS NOT NULL",
            params! { "t" => table, "c" => column },
        )?;
        if let Some(name) = fk_name {
            conn.query_drop(format!(
                "ALTER TABLE {table} DROP FOREIGN KEY {name}"
            ))?;
        }
        conn.query_drop(format!(
            "ALTER TABLE {table} MODIFY COLUMN {column} {definition}"
        ))?;
        conn.query_drop(format!(
            "ALTER TABLE {table} ADD FOREIGN KEY ({column}) REFERENCES {ref_table}({ref_column}) ON DELETE SET NULL"
        ))?;
    }
    Ok(())
}

pub fn init_db() -> Result<Pool, mysql::Error> {
    let url = crate::config::database_url();
    let pool = Pool::new(url)?;

    // Validate connection
    let mut conn = pool.get_conn()?;
    let _: Option<u8> = conn.query_first("SELECT 1")?;

    // 1. Users (CRM login accounts)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            username VARCHAR(100) UNIQUE NOT NULL,
            password VARCHAR(255) NOT NULL,
            full_name VARCHAR(150) NOT NULL,
            role VARCHAR(20) NOT NULL, -- 'admin', 'sales', 'support', 'manager'
            email VARCHAR(100) NOT NULL,
            phone VARCHAR(30) NULL,
            photo_url VARCHAR(255) NULL,
            is_active TINYINT(1) NOT NULL DEFAULT 1,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
        )",
    )?;

    // 2. User tokens (session tokens)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS user_tokens (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            user_id BIGINT NOT NULL,
            token VARCHAR(255) UNIQUE NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )",
    )?;

    // 3. Companies (accounts/organizations)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS companies (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(150) NOT NULL,
            industry VARCHAR(100) NULL,
            website VARCHAR(255) NULL,
            phone VARCHAR(30) NULL,
            email VARCHAR(100) NULL,
            address TEXT NULL,
            city VARCHAR(100) NULL,
            country VARCHAR(100) NULL,
            description TEXT NULL,
            assigned_to BIGINT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (assigned_to) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 4. Contacts (people/leads)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS contacts (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            first_name VARCHAR(100) NOT NULL,
            last_name VARCHAR(100) NULL,
            email VARCHAR(100) NULL,
            phone VARCHAR(30) NULL,
            job_title VARCHAR(100) NULL,
            company_id BIGINT NULL,
            source VARCHAR(50) NULL DEFAULT 'manual', -- 'manual', 'website', 'campaign', 'referral'
            status VARCHAR(20) NOT NULL DEFAULT 'lead', -- 'lead', 'prospect', 'customer', 'churned'
            assigned_to BIGINT NULL,
            description TEXT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE SET NULL,
            FOREIGN KEY (assigned_to) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 5. Deal stages (customizable pipeline)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS deal_stages (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            position INT NOT NULL DEFAULT 0,
            probability DECIMAL(5,2) NOT NULL DEFAULT 0.00,
            color VARCHAR(20) NULL,
            is_active TINYINT(1) NOT NULL DEFAULT 1,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )?;

    // 6. Deals (opportunities)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS deals (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            title VARCHAR(255) NOT NULL,
            contact_id BIGINT NULL,
            company_id BIGINT NULL,
            stage_id BIGINT NOT NULL,
            owner_id BIGINT NULL,
            value DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            currency VARCHAR(3) NOT NULL DEFAULT 'IDR',
            expected_close_date DATE NULL,
            actual_close_date DATE NULL,
            status VARCHAR(20) NOT NULL DEFAULT 'open', -- 'open', 'won', 'lost'
            description TEXT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE SET NULL,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE SET NULL,
            FOREIGN KEY (stage_id) REFERENCES deal_stages(id) ON DELETE RESTRICT,
            FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // Relax contact_id on existing deals created before it became optional.
    ensure_column_nullable(
        &mut conn,
        "deals",
        "contact_id",
        "BIGINT NULL",
        "contacts",
        "id",
    )?;

    // 7. Deal discussions (timeline/chat for each deal)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS deal_discussions (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            deal_id BIGINT NOT NULL,
            user_id BIGINT NULL,
            content TEXT NOT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 7a. Discussion file attachments
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS discussion_files (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            discussion_id BIGINT NOT NULL,
            file_name VARCHAR(255) NOT NULL,
            file_path VARCHAR(500) NOT NULL,
            mime_type VARCHAR(100) NULL,
            file_size BIGINT NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (discussion_id) REFERENCES deal_discussions(id) ON DELETE CASCADE
        )",
    )?;

    // 8. Activities (calls, meetings, tasks, emails)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS activities (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            activity_type VARCHAR(30) NOT NULL, -- 'call', 'meeting', 'task', 'email', 'note'
            subject VARCHAR(255) NOT NULL,
            description TEXT NULL,
            contact_id BIGINT NULL,
            deal_id BIGINT NULL,
            company_id BIGINT NULL,
            assigned_to BIGINT NULL,
            due_date DATETIME NULL,
            completed_at DATETIME NULL,
            status VARCHAR(20) NOT NULL DEFAULT 'pending', -- 'pending', 'completed', 'overdue', 'cancelled'
            created_by BIGINT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE SET NULL,
            FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE SET NULL,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE SET NULL,
            FOREIGN KEY (assigned_to) REFERENCES users(id) ON DELETE SET NULL,
            FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 8. Notes (attached to contacts/deals/companies)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS notes (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            content TEXT NOT NULL,
            contact_id BIGINT NULL,
            deal_id BIGINT NULL,
            company_id BIGINT NULL,
            created_by BIGINT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE CASCADE,
            FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 9. Products
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS products (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            sku VARCHAR(100) UNIQUE NULL,
            description TEXT NULL,
            category VARCHAR(100) NULL,
            unit_price DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            currency VARCHAR(3) NOT NULL DEFAULT 'IDR',
            is_active TINYINT(1) NOT NULL DEFAULT 1,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
        )",
    )?;

    // 10. Quotes (proposals)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS quotes (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            deal_id BIGINT NOT NULL,
            quote_number VARCHAR(100) UNIQUE NOT NULL,
            issue_date DATE NOT NULL,
            expiry_date DATE NULL,
            subtotal DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            tax_rate DECIMAL(5,2) NOT NULL DEFAULT 0.00,
            tax_amount DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            total_amount DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            currency VARCHAR(3) NOT NULL DEFAULT 'IDR',
            status VARCHAR(20) NOT NULL DEFAULT 'draft', -- 'draft', 'sent', 'accepted', 'rejected', 'expired'
            notes TEXT NULL,
            created_by BIGINT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE CASCADE,
            FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 11. Quote items
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS quote_items (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            quote_id BIGINT NOT NULL,
            product_id BIGINT NULL,
            description VARCHAR(255) NOT NULL,
            quantity DECIMAL(18,4) NOT NULL DEFAULT 1.0000,
            unit_price DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            discount DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            total DECIMAL(18,2) NOT NULL DEFAULT 0.00,
            FOREIGN KEY (quote_id) REFERENCES quotes(id) ON DELETE CASCADE,
            FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE SET NULL
        )",
    )?;

    // 12. Tickets (support cases)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS tickets (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            ticket_number VARCHAR(100) UNIQUE NOT NULL,
            subject VARCHAR(255) NOT NULL,
            description TEXT NOT NULL,
            contact_id BIGINT NULL,
            company_id BIGINT NULL,
            assigned_to BIGINT NULL,
            priority VARCHAR(20) NOT NULL DEFAULT 'medium', -- 'low', 'medium', 'high', 'urgent'
            status VARCHAR(20) NOT NULL DEFAULT 'open', -- 'open', 'in_progress', 'resolved', 'closed', 'reopened'
            source VARCHAR(30) NULL DEFAULT 'email', -- 'email', 'phone', 'whatsapp', 'portal'
            resolved_at DATETIME NULL,
            closed_at DATETIME NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE SET NULL,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE SET NULL,
            FOREIGN KEY (assigned_to) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 13. Campaigns
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS campaigns (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            campaign_type VARCHAR(50) NOT NULL, -- 'email', 'whatsapp', 'sms', 'social'
            status VARCHAR(20) NOT NULL DEFAULT 'draft', -- 'draft', 'running', 'paused', 'completed'
            start_date DATE NULL,
            end_date DATE NULL,
            budget DECIMAL(18,2) NULL,
            currency VARCHAR(3) NOT NULL DEFAULT 'IDR',
            target_audience TEXT NULL,
            message_template TEXT NULL,
            sent_count INT UNSIGNED NOT NULL DEFAULT 0,
            delivered_count INT UNSIGNED NOT NULL DEFAULT 0,
            responded_count INT UNSIGNED NOT NULL DEFAULT 0,
            created_by BIGINT NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
        )",
    )?;

    // 14. Tags
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS tags (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(50) UNIQUE NOT NULL,
            color VARCHAR(20) NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )?;

    // 15. Contact tags
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS contact_tags (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            contact_id BIGINT NOT NULL,
            tag_id BIGINT NOT NULL,
            FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE,
            UNIQUE KEY unique_contact_tag (contact_id, tag_id)
        )",
    )?;

    // 16. WhatsApp sessions (foundation/CRM-wide sender)
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS whatsapp_sessions (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL DEFAULT 'Foundation',
            sender_number VARCHAR(30) NULL,
            wa_status VARCHAR(20) NOT NULL DEFAULT 'disconnected', -- 'disconnected','pairing','connected'
            wa_qr TEXT NULL,
            wa_paired_at DATETIME NULL,
            updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
        )",
    )?;

    // 17. WhatsApp messages log
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS whatsapp_messages (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            session_id BIGINT NOT NULL,
            deal_id BIGINT NULL,
            contact_id BIGINT NULL,
            phone VARCHAR(30) NOT NULL,
            direction VARCHAR(10) NOT NULL DEFAULT 'outbound', -- 'inbound','outbound'
            message TEXT NOT NULL,
            wa_message_id VARCHAR(255) NULL,
            sender_name VARCHAR(150) NULL,
            status VARCHAR(20) NOT NULL DEFAULT 'pending', -- 'pending','sent','delivered','read','failed'
            error_message TEXT NULL,
            sent_at DATETIME NULL,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (session_id) REFERENCES whatsapp_sessions(id) ON DELETE CASCADE,
            FOREIGN KEY (deal_id) REFERENCES deals(id) ON DELETE SET NULL,
            FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE SET NULL
        )",
    )?;

    // Migrate existing whatsapp_messages table created before the deal integration.
    ensure_column(&mut conn, "whatsapp_messages", "deal_id", "BIGINT NULL")?;
    ensure_column(&mut conn, "whatsapp_messages", "contact_id", "BIGINT NULL")?;
    ensure_column(
        &mut conn,
        "whatsapp_messages",
        "direction",
        "VARCHAR(10) NOT NULL DEFAULT 'outbound'",
    )?;
    ensure_column(&mut conn, "whatsapp_messages", "sender_name", "VARCHAR(150) NULL")?;
    // Widen status enum to include 'read'.
    conn.query_drop(
        "ALTER TABLE whatsapp_messages MODIFY COLUMN status VARCHAR(20) NOT NULL DEFAULT 'pending'",
    )?;

    // 18. Notifications
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS notifications (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            user_id BIGINT NOT NULL,
            title VARCHAR(255) NOT NULL,
            body TEXT NOT NULL,
            category VARCHAR(50) NOT NULL DEFAULT 'general', -- 'general','deal','activity','ticket','whatsapp'
            entity_type VARCHAR(30) NULL, -- 'deal','contact','ticket','activity'
            entity_id BIGINT NULL,
            is_read TINYINT(1) NOT NULL DEFAULT 0,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )",
    )?;

    // Seed default deal stages if none exist
    let stages_exist: Option<u8> = conn.query_first("SELECT 1 FROM deal_stages LIMIT 1")?;
    if stages_exist.is_none() {
        let stages = vec![
            ("New", 0, 10.00, "#3498db"),
            ("Qualified", 1, 30.00, "#9b59b6"),
            ("Proposal", 2, 60.00, "#f1c40f"),
            ("Negotiation", 3, 80.00, "#e67e22"),
            ("Closed Won", 4, 100.00, "#2ecc71"),
            ("Closed Lost", 5, 0.00, "#e74c3c"),
        ];
        for (name, pos, prob, color) in stages {
            conn.exec_drop(
                "INSERT INTO deal_stages (name, position, probability, color) VALUES (:name, :pos, :prob, :color)",
                params! {
                    "name" => name,
                    "pos" => pos,
                    "prob" => prob,
                    "color" => color,
                },
            )?;
        }
    }

    // Seed singleton WhatsApp session row if none exists
    let wa_exists: Option<u8> = conn.query_first("SELECT 1 FROM whatsapp_sessions LIMIT 1")?;
    if wa_exists.is_none() {
        conn.query_drop("INSERT INTO whatsapp_sessions (name) VALUES ('Foundation')")?;
    }

    // Seed default admin if table is empty
    let admin_exists: Option<u8> = conn.query_first("SELECT 1 FROM users WHERE role = 'admin'")?;
    if admin_exists.is_none() {
        let hashed_pass = bcrypt::hash("admin123", 10).unwrap();
        conn.exec_drop(
            "INSERT INTO users (username, password, full_name, role, email) VALUES (:username, :password, :full_name, :role, :email)",
            params! {
                "username" => "admin",
                "password" => hashed_pass,
                "full_name" => "Administrator",
                "role" => "admin",
                "email" => "admin@example.com"
            },
        )?;
    }

    // Idempotent migrations for columns that may be added later.
    ensure_column(&mut conn, "users", "phone", "VARCHAR(30) NULL")?;
    ensure_column(&mut conn, "users", "photo_url", "VARCHAR(255) NULL")?;
    ensure_column(&mut conn, "companies", "assigned_to", "BIGINT NULL")?;
    ensure_column(
        &mut conn,
        "contacts",
        "source",
        "VARCHAR(50) NULL DEFAULT 'manual'",
    )?;
    ensure_column(
        &mut conn,
        "deals",
        "currency",
        "VARCHAR(3) NOT NULL DEFAULT 'IDR'",
    )?;
    ensure_column(
        &mut conn,
        "activities",
        "status",
        "VARCHAR(20) NOT NULL DEFAULT 'pending'",
    )?;
    ensure_column(
        &mut conn,
        "tickets",
        "source",
        "VARCHAR(30) NULL DEFAULT 'email'",
    )?;
    ensure_column(&mut conn, "campaigns", "message_template", "TEXT NULL")?;

    Ok(pool)
}
