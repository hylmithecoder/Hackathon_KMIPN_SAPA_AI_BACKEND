# SAPA AI CRM API Documentation

Contract snapshot: **2026-07-25**

Source of truth: the current Rust working tree at
`/home/hylmi/Hylmi/Pemrograman_Berorientasi_Objek/Rust/api_sapaai`.

Base URL: `http://localhost:5790` by default, or the configured
`SERVER_BASE_URL`. Port `3000` is normally the Next.js frontend and must not be
used as the backend origin unless a reverse proxy is explicitly configured.

## Breaking-change migration notes

- Configure browser requests with
  `NEXT_PUBLIC_API_URL=http://localhost:5790`. `NEXT_PUBLIC_WS_URL` is optional;
  clients can derive `ws://localhost:5790/api/v1/ws` from the API origin.
- Uploads are now multipart requests. Do not set `Content-Type` manually; the
  browser must add the multipart boundary.
- Upload responses contain backend-relative paths such as
  `/uploads/<uuid>.png`. Resolve them against the API origin before rendering.
- WhatsApp messages add nullable `media_url`; send requests accept it, and
  deal-scoped sends also accept an optional phone override.
- Quotes add nullable `template_id` plus server-computed `subtotal`,
  `tax_amount`, and `total_amount`.
- Price books, quote templates, and read-only OpenCode drafting are new
  resources. Template instantiation creates a quote snapshot.
- Rust response DTOs use JSON `null` for many optional CRM fields. Consumers
  must not assume that phone, description, company, owner, dates, or media
  fields are always strings or IDs.
- Realtime events now include `price_book` and `quote_template`. Event `id`,
  `payload`, and `timestamp` may be omitted.
- WhatsApp lifecycle transitions (`pairing`, `connected`, `disconnected`,
  `failed`, and logout) emit `whatsapp_session` updates. Persisted inbound,
  outbound, and failed outbound records emit `whatsapp_message` events.

Most JSON endpoints follow a unified response envelope:

### Success Envelope
```json
{
  "success": true,
  "data": { ... }
}
```

Message-only success:
```json
{
  "success": true,
  "message": "Operation completed successfully"
}
```

Successful delete/logout operations may instead return `204 No Content`.

### Error Envelope
```json
{
  "success": false,
  "message": "Detailed error message"
}
```

---

## Table of Contents
1. [Health Checks](#1-health-checks)
   - [File Uploads](#file-uploads)
2. [Authentication & Users](#2-authentication--users)
3. [Companies](#3-companies)
4. [Contacts](#4-contacts)
5. [Deal Stages](#5-deal-stages)
6. [Deals](#6-deals)
7. [Activities](#7-activities)
8. [Notes](#8-notes)
9. [Products](#9-products)
10. [Price Books](#10-price-books)
11. [Quotes](#11-quotes)
12. [Quote Templates](#12-quote-templates)
13. [OpenCode AI Quote Drafting](#13-opencode-ai-quote-drafting)
14. [Tickets](#14-tickets)
15. [Campaigns](#15-campaigns)
16. [Tags](#16-tags)
17. [Notifications](#17-notifications)
18. [WhatsApp Integration](#18-whatsapp-integration)
19. [Real-Time WebSocket](#19-real-time-websocket)

---

## 1. Health Checks

### GET `/api/v1/health`
- **Description:** Server liveness probe.
- **Request Body:** None
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "version": "0.1.0"
  }
}
```

### GET `/api/v1/health/ready`
- **Description:** Readiness probe (verifies DB connection).
- **Request Body:** None
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "status": "ready",
    "database": "connected"
  }
}
```

---

## File Uploads

### POST `/api/v1/upload`

- **Content-Type:** `multipart/form-data`
- **Form field:** `file`
- **Description:** Store the first attached image, document, or sticker under
  `storage/uploads`. The backend serves stored files from `/uploads/*`.
- **Current constraints:** The handler does not currently enforce a maximum
  size, MIME allowlist, or extension allowlist. Deployments should enforce
  limits at the reverse proxy until backend validation is added.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "url": "/uploads/550e8400-e29b-41d4-a716-446655440000.png",
    "filename": "proposal.png"
  }
}
```

The returned `url` is relative to the backend origin. For example, a frontend
running on port `3000` renders the sample above from
`http://localhost:5790/uploads/550e8400-e29b-41d4-a716-446655440000.png`.

---

## 2. Authentication & Users

Default admin account seeded on startup:
- **Username:** `admin`
- **Password:** `admin123`

### POST `/api/v1/auth/login`
- **Description:** Authenticate user and receive bearer token.
- **Request Body:**
```json
{
  "username": "admin",
  "password": "admin123"
}
```
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "user": {
      "id": 1,
      "username": "admin",
      "full_name": "System Admin",
      "role": "admin",
      "email": "admin@sapaai.com",
      "phone": "+628123456789",
      "photo_url": null,
      "is_active": true
    },
    "token": "439e6a0d-8622-4a0b-a25e-3ecf12467d34"
  }
}
```

### POST `/api/v1/auth/register`
- **Description:** Register a new CRM user.
- **Request Body:**
```json
{
  "username": "johndoe",
  "password": "SecretPassword123",
  "full_name": "John Doe",
  "role": "sales",
  "email": "john@example.com",
  "phone": "+628129876543"
}
```
- **Response (201 Created):**
```json
{
  "success": true,
  "data": {
    "id": 2,
    "username": "johndoe",
    "full_name": "John Doe",
    "role": "sales",
    "email": "john@example.com",
    "phone": "+628129876543",
    "photo_url": null,
    "is_active": true
  }
}
```

### POST `/api/v1/auth/logout`
- **Description:** Invalidate current user session token.
- **Headers:** `Authorization: Bearer <token>`
- **Response:** `204 No Content`

### GET `/api/v1/users`
- **Description:** List all users.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "username": "admin",
      "full_name": "System Admin",
      "role": "admin",
      "email": "admin@sapaai.com",
      "phone": "+628123456789",
      "photo_url": null,
      "is_active": true
    }
  ]
}
```

### PUT `/api/v1/users/{id}`
- **Description:** Update existing user profile or status.
- **Request Body:**
```json
{
  "username": "johndoe_updated",
  "full_name": "John Doe Updated",
  "email": "john.updated@example.com",
  "phone": "+628129876543",
  "role": "manager",
  "is_active": true
}
```
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": 2,
    "username": "johndoe_updated",
    "full_name": "John Doe Updated",
    "role": "manager",
    "email": "john.updated@example.com",
    "phone": "+628129876543",
    "photo_url": null,
    "is_active": true
  }
}
```

### DELETE `/api/v1/users/{id}`
- **Description:** Delete user by ID.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "message": "User deleted successfully"
  }
}
```

---

## 3. Companies

### GET `/api/v1/companies`
- **Description:** List companies with optional filtering.
- **Query Params:** `search` (string), `assigned_to` (u64)
- **Response (200 OK):**
```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "name": "Acme Corp",
      "industry": "Technology",
      "website": "https://acme.com",
      "phone": "+62215551234",
      "email": "info@acme.com",
      "address": "Jl. Sudirman No. 12",
      "city": "Jakarta",
      "country": "Indonesia",
      "description": "Enterprise client",
      "assigned_to": 1
    }
  ]
}
```

### POST `/api/v1/companies`
- **Description:** Create a new company record.
- **Request Body:**
```json
{
  "name": "Acme Corp",
  "industry": "Technology",
  "website": "https://acme.com",
  "phone": "+62215551234",
  "email": "info@acme.com",
  "address": "Jl. Sudirman No. 12",
  "city": "Jakarta",
  "country": "Indonesia",
  "description": "Enterprise client",
  "assigned_to": 1
}
```
- **Response (201 Created):**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "name": "Acme Corp",
    "industry": "Technology",
    "website": "https://acme.com",
    "phone": "+62215551234",
    "email": "info@acme.com",
    "address": "Jl. Sudirman No. 12",
    "city": "Jakarta",
    "country": "Indonesia",
    "description": "Enterprise client",
    "assigned_to": 1
  }
}
```

### GET `/api/v1/companies/{id}`
- **Description:** Get company by ID.

### PUT `/api/v1/companies/{id}`
- **Description:** Update company details.
- **Request Body:**
```json
{
  "name": "Acme Global Corp",
  "industry": "Software",
  "website": "https://acmeglobal.com",
  "phone": "+62215559999",
  "email": "contact@acmeglobal.com",
  "address": "Jl. MH Thamrin No. 5",
  "city": "Jakarta",
  "country": "Indonesia",
  "description": "Updated enterprise client details",
  "assigned_to": 1
}
```

### DELETE `/api/v1/companies/{id}`
- **Description:** Delete company by ID.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "message": "Company deleted successfully"
  }
}
```

---

## 4. Contacts

### GET `/api/v1/contacts`
- **Description:** List contacts with optional filters.
- **Query Params:** `search` (string), `status` (string), `company_id` (u64), `assigned_to` (u64), `tag_id` (u64)
- **Response (200 OK):**
```json
{
  "success": true,
  "data": [
    {
      "id": 10,
      "first_name": "Budi",
      "last_name": "Santoso",
      "email": "budi@acme.com",
      "phone": "+62812345678",
      "job_title": "CTO",
      "company_id": 1,
      "source": "Website",
      "status": "Lead",
      "assigned_to": 1,
      "description": "Key decision maker",
      "tags": [
        { "id": 1, "name": "VIP", "color": "#FF0000" }
      ]
    }
  ]
}
```

### POST `/api/v1/contacts`
- **Description:** Create contact record and attach optional tags.
- **Request Body:**
```json
{
  "first_name": "Budi",
  "last_name": "Santoso",
  "email": "budi@acme.com",
  "phone": "+62812345678",
  "job_title": "CTO",
  "company_id": 1,
  "source": "Website",
  "status": "Lead",
  "assigned_to": 1,
  "description": "Key decision maker",
  "tag_ids": [1, 2]
}
```
- **Response (201 Created):**
```json
{
  "success": true,
  "data": {
    "id": 10,
    "first_name": "Budi",
    "last_name": "Santoso",
    "email": "budi@acme.com",
    "phone": "+62812345678",
    "job_title": "CTO",
    "company_id": 1,
    "source": "Website",
    "status": "Lead",
    "assigned_to": 1,
    "description": "Key decision maker"
  }
}
```

### GET `/api/v1/contacts/{id}`
- **Description:** Get detailed contact info including attached tags.

### PUT `/api/v1/contacts/{id}`
- **Description:** Update contact details and tag assignments.
- **Request Body:**
```json
{
  "first_name": "Budi",
  "last_name": "Santoso",
  "email": "budi.santoso@acme.com",
  "phone": "+62812345678",
  "job_title": "VP of Engineering",
  "company_id": 1,
  "source": "Referral",
  "status": "Customer",
  "assigned_to": 1,
  "description": "Upgraded to customer",
  "tag_ids": [1, 3]
}
```

### DELETE `/api/v1/contacts/{id}`
- **Description:** Delete contact by ID.

### GET `/api/v1/contacts/{id}/tags`
- **Description:** List tags linked to specific contact.

### POST `/api/v1/contacts/{id}/tags`
- **Description:** Add single tag to contact.
- **Request Body:**
```json
{
  "tag_id": 3
}
```

### DELETE `/api/v1/contacts/{id}/tags/{tag_id}`
- **Description:** Remove tag association from contact.

---

## 5. Deal Stages

### GET `/api/v1/deal-stages`
- **Description:** List pipeline stages ordered by position.

### POST `/api/v1/deal-stages`
- **Description:** Create new deal stage.
- **Request Body:**
```json
{
  "name": "Qualification",
  "position": 1,
  "probability": 20.0,
  "color": "#3498db"
}
```

### GET `/api/v1/deal-stages/{id}`
### PUT `/api/v1/deal-stages/{id}`
- **Request Body:**
```json
{
  "name": "Qualification & Discovery",
  "position": 1,
  "probability": 25.0,
  "color": "#2980b9",
  "is_active": true
}
```

### DELETE `/api/v1/deal-stages/{id}`

### PUT `/api/v1/deal-stages/reorder`
- **Description:** Bulk update display order of deal stages.
- **Request Body:**
```json
{
  "ordered_ids": [3, 1, 2, 4]
}
```

---

## 6. Deals

### GET `/api/v1/deals`
- **Description:** List all sales deals.

### POST `/api/v1/deals`
- **Description:** Create a new deal.
- **Request Body:**
```json
{
  "title": "SAPA AI Enterprise License",
  "contact_id": 10,
  "company_id": 1,
  "stage_id": 1,
  "owner_id": 1,
  "value": 150000000.0,
  "currency": "IDR",
  "expected_close_date": "2026-08-30",
  "status": "Open",
  "description": "500 seats license deal"
}
```
- **Notes:** `contact_id` is optional. If the provided contact does not exist, it is silently dropped to `null` so the deal can still be created.
- **Response (201 Created):**
```json
{
  "success": true,
  "data": {
    "id": 5,
    "title": "SAPA AI Enterprise License",
    "contact_id": 10,
    "company_id": 1,
    "stage_id": 1,
    "owner_id": 1,
    "value": 150000000.0,
    "currency": "IDR",
    "expected_close_date": "2026-08-30",
    "status": "Open",
    "description": "500 seats license deal"
  }
}
```

### GET `/api/v1/deals/{id}`

### PUT `/api/v1/deals/{id}`
- **Request Body:**
```json
{
  "title": "SAPA AI Enterprise License (Renegotiated)",
  "contact_id": 10,
  "company_id": 1,
  "stage_id": 2,
  "owner_id": 1,
  "value": 180000000.0,
  "currency": "IDR",
  "expected_close_date": "2026-09-15",
  "actual_close_date": null,
  "status": "In Progress",
  "description": "Expanded scope to include WhatsApp Bot add-on"
}
```

### DELETE `/api/v1/deals/{id}`

### PUT `/api/v1/deals/{id}/move-stage`
- **Description:** Quickly shift deal to another pipeline stage.
- **Request Body:**
```json
{
  "stage_id": 3
}
```

### GET `/api/v1/deals/{id}/detail`
- **Description:** Get deal with full contact and company details.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": 5,
    "title": "SAPA AI Enterprise License",
    "contact": {
      "id": 10,
      "first_name": "Budi",
      "last_name": "Santoso",
      "email": "budi@acme.com",
      "phone": "+62812345678",
      "job_title": "CTO",
      "company_id": 1,
      "company_name": "Acme Corp",
      "source": "Website",
      "status": "Lead",
      "assigned_to": 1,
      "description": "Key decision maker"
    },
    "company": {
      "id": 1,
      "name": "Acme Corp",
      "industry": "Technology",
      "website": "https://acme.com",
      "phone": "+62215551234",
      "email": "info@acme.com",
      "address": "Jl. Sudirman No. 12",
      "city": "Jakarta",
      "country": "Indonesia",
      "description": "Enterprise client",
      "assigned_to": 1
    },
    "stage_id": 1,
    "stage_name": "New",
    "owner_id": 1,
    "owner_name": "System Admin",
    "value": 150000000.0,
    "currency": "IDR",
    "expected_close_date": "2026-08-30",
    "actual_close_date": null,
    "status": "Open",
    "description": "500 seats license deal",
    "created_at": "2026-07-20T10:00:00Z",
    "updated_at": "2026-07-20T10:00:00Z"
  }
}
```

### GET `/api/v1/deals/{id}/discussions`
- **Description:** List timeline comments for a deal.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "deal_id": 5,
      "user_id": 1,
      "author_name": "System Admin",
      "content": "Follow-up call scheduled for tomorrow.",
      "created_at": "2026-07-20T10:00:00Z"
    }
  ]
}
```

### POST `/api/v1/deals/{id}/discussions`
- **Description:** Add a comment to the deal timeline. Supports text and file uploads.
- **Request Body:** `multipart/form-data`
  - `content` (string, optional) — discussion text.
  - `files` (file, optional, multiple) — attachments up to 10 MB each.
- **Response (201 Created):**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "deal_id": 5,
    "user_id": null,
    "author_name": null,
    "content": "Follow-up call scheduled for tomorrow.",
    "files": [
      {
        "id": 1,
        "discussion_id": 1,
        "file_name": "proposal.pdf",
        "file_url": "/uploads/discussions/1/...",
        "mime_type": "application/pdf",
        "file_size": 24576,
        "created_at": "2026-07-20T10:00:00Z"
      }
    ],
    "created_at": "2026-07-20T10:00:00Z"
  }
}
```

---

## 7. Activities

### GET `/api/v1/activities`
- **Description:** List scheduled activities (calls, meetings, tasks).

### POST `/api/v1/activities`
- **Request Body:**
```json
{
  "activity_type": "Meeting",
  "subject": "Product Demo Walkthrough",
  "description": "Demonstrate CRM features to technical team",
  "contact_id": 10,
  "deal_id": 5,
  "company_id": 1,
  "assigned_to": 1,
  "due_date": "2026-07-25T14:00:00Z"
}
```

### GET `/api/v1/activities/{id}`

### PUT `/api/v1/activities/{id}`
- **Request Body:**
```json
{
  "activity_type": "Meeting",
  "subject": "Product Demo & Pricing Walkthrough",
  "description": "Updated agenda with finance team",
  "contact_id": 10,
  "deal_id": 5,
  "company_id": 1,
  "assigned_to": 1,
  "due_date": "2026-07-26T10:00:00Z",
  "status": "Pending",
  "completed_at": null
}
```

### DELETE `/api/v1/activities/{id}`

### PUT `/api/v1/activities/{id}/done`
- **Description:** Mark activity status as completed.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "message": "Activity marked as done"
  }
}
```

---

## 8. Notes

### GET `/api/v1/notes`
- **Description:** List internal CRM notes.

### POST `/api/v1/notes`
- **Request Body:**
```json
{
  "content": "Client requested custom SLA addendum for WhatsApp gateway.",
  "contact_id": 10,
  "deal_id": 5,
  "company_id": 1
}
```

### GET `/api/v1/notes/{id}`

### PUT `/api/v1/notes/{id}`
- **Request Body:**
```json
{
  "content": "Updated note: Client requested custom SLA addendum (99.9% uptime)."
}
```

### DELETE `/api/v1/notes/{id}`

---

## 9. Products

### GET `/api/v1/products`
- **Description:** List catalog products and services.

### POST `/api/v1/products`
- **Request Body:**
```json
{
  "name": "SAPA AI Pro Monthly",
  "sku": "SKU-PRO-001",
  "description": "Monthly subscription per user",
  "category": "Software",
  "unit_price": 250000.0,
  "currency": "IDR"
}
```

### GET `/api/v1/products/{id}`

### PUT `/api/v1/products/{id}`
- **Request Body:**
```json
{
  "name": "SAPA AI Pro Monthly (v2)",
  "sku": "SKU-PRO-001",
  "description": "Monthly subscription per user with unlimited WhatsApp bots",
  "category": "Software",
  "unit_price": 300000.0,
  "currency": "IDR",
  "is_active": true
}
```

### DELETE `/api/v1/products/{id}`

---

## 10. Price Books

Price books add deliberate, quantity-tiered catalogue pricing. Resolving a
price does not modify a product or an existing quote; applications must copy
the resolved result into the quote item they create.

### GET `/api/v1/price-books`
- **Description:** List all price books, with the default book first.

### POST `/api/v1/price-books`
```json
{
  "name": "Indonesia Enterprise 2026",
  "currency": "IDR",
  "description": "Commercial pricing for Indonesian enterprise accounts",
  "is_default": true
}
```

### GET, PUT, DELETE `/api/v1/price-books/{id}`
- **Description:** Read, modify, or remove a price book. Setting `is_default`
  to `true` clears the previous default book.

### GET, POST `/api/v1/price-books/{id}/items`
- **Description:** List tiers or add one product price tier.
```json
{
  "product_id": 1,
  "min_quantity": 10,
  "unit_price": 225000.0
}
```

### PUT, DELETE `/api/v1/price-books/{price_book_id}/items/{item_id}`
- **Description:** Change or remove a price tier.

### GET `/api/v1/price-books/{price_book_id}/resolve/{product_id}?quantity=10`
- **Description:** Resolves the highest eligible `min_quantity` tier.
- **Response data:** `price_book_id`, `product_id`, requested `quantity`,
  matched `min_quantity`, and `unit_price`.

---

## 11. Quotes

### GET `/api/v1/quotes`
- **Description:** List sales quotes.

### POST `/api/v1/quotes`
- **Description:** Create quote with line items.
- **Request Body:**
```json
{
  "deal_id": 5,
  "quote_number": "QUO-2026-001",
  "issue_date": "2026-07-20",
  "expiry_date": "2026-08-20",
  "tax_rate": 11.0,
  "currency": "IDR",
  "notes": "Valid for 30 days.",
  "items": [
    {
      "product_id": 1,
      "description": "SAPA AI Pro Monthly (10 seats)",
      "quantity": 10.0,
      "unit_price": 250000.0,
      "discount": 0.0
    }
  ]
}
```

- **Response:** The created quote has `template_id: null`, status `draft`, and
  backend-computed `subtotal`, `tax_amount`, and `total_amount`. Line items are
  retrieved separately from `GET /api/v1/quotes/{id}/items`.
- **Calculation:** Each line total is
  `max(quantity * unit_price - discount, 0)`. Tax is calculated from the
  resulting subtotal.

### GET `/api/v1/quotes/{id}`

### PUT `/api/v1/quotes/{id}`
- **Request Body:**
```json
{
  "quote_number": "QUO-2026-001-REV",
  "issue_date": "2026-07-20",
  "expiry_date": "2026-08-31",
  "tax_rate": 11.0,
  "currency": "IDR",
  "status": "Sent",
  "notes": "Revised expiry date."
}
```

### DELETE `/api/v1/quotes/{id}`

### PUT `/api/v1/quotes/{id}/status`
- **Request Body:**
```json
{
  "status": "Accepted"
}
```

### GET `/api/v1/quotes/{id}/items`
- **Description:** List line items belonging to a specific quote.

---

## 12. Quote Templates

Templates are reusable quote blueprints. Instantiation copies every template
item, price, discount, note, and term into a normal editable quote so later
template changes never alter a sent quote.

### GET, POST `/api/v1/quote-templates`
```json
{
  "name": "SAPA AI Enterprise Starter",
  "description": "Baseline enterprise proposal",
  "currency": "IDR",
  "tax_rate": 11,
  "notes": "Pricing valid for 30 days.",
  "terms": "Annual commitment; implementation starts after acceptance.",
  "items": [
    {
      "product_id": 1,
      "description": "SAPA AI Pro Monthly (10 seats)",
      "quantity": 10,
      "unit_price": 250000,
      "discount": 0
    }
  ]
}
```

### GET, PUT, DELETE `/api/v1/quote-templates/{id}`
- **Description:** Read, update, or delete a template. `PUT` may include a
  full replacement `items` array.

### GET `/api/v1/quote-templates/{id}/items`
- **Description:** Retrieve ordered template line items.

### POST `/api/v1/quote-templates/{id}/instantiate`
- **Description:** Create a draft quote snapshot from an active template.
  `quote_number` and `issue_date` are optional; the backend generates them
  when omitted.
```json
{
  "deal_id": 5,
  "quote_number": "QUO-2026-ENTERPRISE-001",
  "issue_date": "2026-07-25",
  "expiry_date": "2026-08-24",
  "notes": "Prepared for the procurement team."
}
```

---

## 13. OpenCode AI Quote Drafting

The AI integration is backend-only and intentionally read-only. It executes
the server's configured OpenCode CLI in an empty working directory, provides a
minimal deal/quote/template context, and returns a draft for human review. It
does not create, edit, send, accept, or reject a quote.

Set these variables through the custom `.env` parser before using it:

```dotenv
AI_ENABLED=true
AI_OPENCODE_COMMAND=opencode
# Optional; must be a configured OpenCode provider/model value.
AI_OPENCODE_MODEL=provider/model
AI_TIMEOUT_SECS=45
```

### POST `/api/v1/ai/quotes/draft`
```json
{
  "deal_id": 5,
  "quote_id": 12,
  "template_id": 3,
  "language": "Indonesian",
  "instruction": "Write a concise introduction for the procurement lead."
}
```

- **Response data:** `provider`, optional `model`, `review_required: true`,
  and `draft`. The draft normally contains `subject`, `intro`, `notes`,
  `recommended_next_step`, and `warnings`.
- **Safety boundary:** Verify prices, legal terms, delivery dates, and customer
  claims before inserting the returned text into a quote.

---

## 14. Tickets

### GET `/api/v1/tickets`
- **Description:** List customer support tickets.

### POST `/api/v1/tickets`
- **Request Body:**
```json
{
  "ticket_number": "TCK-001",
  "subject": "Integration Error with Webhook",
  "description": "Receiving HTTP 500 when syncing WhatsApp contacts.",
  "contact_id": 10,
  "company_id": 1,
  "assigned_to": 1,
  "priority": "High",
  "source": "Email"
}
```

### GET `/api/v1/tickets/{id}`

### PUT `/api/v1/tickets/{id}`
- **Request Body:**
```json
{
  "subject": "Integration Error with Webhook (Resolved in v1.2)",
  "description": "Resolved webhook configuration issue.",
  "contact_id": 10,
  "company_id": 1,
  "assigned_to": 1,
  "priority": "Medium",
  "source": "Email"
}
```

### DELETE `/api/v1/tickets/{id}`

### PUT `/api/v1/tickets/{id}/status`
- **Request Body:**
```json
{
  "status": "Closed"
}
```

---

## 15. Campaigns

### GET `/api/v1/campaigns`
- **Description:** List marketing campaigns.

### POST `/api/v1/campaigns`
- **Request Body:**
```json
{
  "name": "Q3 WhatsApp Broadcast",
  "campaign_type": "WhatsApp",
  "start_date": "2026-08-01",
  "end_date": "2026-08-07",
  "budget": 5000000.0,
  "currency": "IDR",
  "target_audience": "Leads in Jakarta",
  "message_template": "Halo {{name}}, dapatkan diskon 20% SAPA AI minggu ini!"
}
```

### GET `/api/v1/campaigns/{id}`

### PUT `/api/v1/campaigns/{id}`
- **Request Body:**
```json
{
  "name": "Q3 WhatsApp Broadcast (Extended)",
  "campaign_type": "WhatsApp",
  "status": "Active",
  "start_date": "2026-08-01",
  "end_date": "2026-08-15",
  "budget": 7500000.0,
  "currency": "IDR",
  "target_audience": "All Leads",
  "message_template": "Halo {{name}}, diskon diperpanjang hingga 15 Agustus!"
}
```

### DELETE `/api/v1/campaigns/{id}`

### PUT `/api/v1/campaigns/{id}/status`
- **Request Body:**
```json
{
  "status": "Completed"
}
```

---

## 16. Tags

### GET `/api/v1/tags`
- **Description:** List available tags for labeling entities.

### POST `/api/v1/tags`
- **Request Body:**
```json
{
  "name": "High Priority",
  "color": "#E74C3C"
}
```

### GET `/api/v1/tags/{id}`

### PUT `/api/v1/tags/{id}`
- **Request Body:**
```json
{
  "name": "Critical Priority",
  "color": "#900C3F"
}
```

### DELETE `/api/v1/tags/{id}`

---

## 17. Notifications

### GET `/api/v1/notifications`
- **Description:** List user notifications.

### POST `/api/v1/notifications`
- **Request Body:**
```json
{
  "user_id": 1,
  "title": "New Deal Assigned",
  "body": "You have been assigned deal SAPA AI Enterprise License",
  "category": "Deal",
  "entity_type": "Deal",
  "entity_id": 5
}
```

### GET `/api/v1/notifications/unread-count`
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "unread_count": 3
  }
}
```

### PUT `/api/v1/notifications/read-all`
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "message": "All notifications marked as read"
  }
}
```

### PUT `/api/v1/notifications/{id}/read`
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "message": "Notification marked as read"
  }
}
```

### DELETE `/api/v1/notifications/{id}`

---

## 18. WhatsApp Integration

### GET `/api/v1/whatsapp/status`
- **Description:** Check in-process WhatsApp Web session status.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": {
    "id": 1,
    "name": "default",
    "sender_number": "6281234567890",
    "wa_status": "connected",
    "wa_qr": null,
    "wa_paired_at": "2026-07-25 08:30:00",
    "updated_at": "2026-07-25 08:30:00"
  }
}
```

### GET `/api/v1/whatsapp/qr`
- **Description:** Retrieve current pairing QR string/SVG.

### POST `/api/v1/whatsapp/connect`
- **Description:** Trigger background WhatsApp session connect/re-connect.

### POST `/api/v1/whatsapp/send`
- **Description:** Send one text message and record an optional media URL.
- **Request Body:**
```json
{
  "phone": "6281234567890",
  "message": "Halo, ini pesan otomatis dari SAPA AI!",
  "media_url": "/uploads/550e8400-e29b-41d4-a716-446655440000.png"
}
```
- **Important:** In the current backend implementation, `media_url` is stored
  with the message log but the WhatsApp engine still calls its text-send
  operation. The referenced file is not transmitted as WhatsApp media yet.

### POST `/api/v1/whatsapp/logout`
- **Description:** Disconnect and delete local SQLite session data.

### GET `/api/v1/whatsapp/messages`
- **Description:** List recently logged outgoing WhatsApp messages.

### GET `/api/v1/deals/{id}/whatsapp-messages`
- **Description:** List WhatsApp messages linked to a specific deal. Returns messages where `deal_id` matches or `contact_id` matches the deal's contact.
- **Response (200 OK):**
```json
{
  "success": true,
  "data": [
    {
      "id": 1,
      "session_id": 1,
      "deal_id": 42,
      "contact_id": 15,
      "phone": "6281234567890",
      "direction": "outbound",
      "message": "Halo, ini pesan dari deal #42",
      "media_url": "/uploads/550e8400-e29b-41d4-a716-446655440000.png",
      "wa_message_id": "3EB0...",
      "sender_name": null,
      "status": "sent",
      "error_message": null,
      "sent_at": "2026-07-24 10:30:00",
      "created_at": "2026-07-24 10:30:00"
    }
  ]
}
```

### POST `/api/v1/deals/{id}/whatsapp-messages`
- **Description:** Send a text message linked to a deal. `phone` is optional
  and overrides the deal contact's phone when supplied. `media_url` is
  optional and is stored in the message log.
- **Recipient resolution:** Deal contacts discovered from inbound WhatsApp
  traffic may contain either a public phone number (PN) or a WhatsApp linked
  identifier (LID). The backend resolves both through the persisted PN/LID
  mapping and may retry through the mapped alternate address.
- **Request Body:**
```json
{
  "phone": "6281234567890",
  "message": "Halo, ini pesan dari deal #42",
  "media_url": "/uploads/550e8400-e29b-41d4-a716-446655440000.png"
}
```
- **Response (200 OK):**
```json
{
  "success": true,
  "message": "Message sent"
}
```
- **Errors:**
  - `404` — Deal not found
  - `400` — Missing message/contact phone or WhatsApp send/rejection failure.
    The failed attempt remains available from the message list with
    `status: "failed"` and an `error_message`.
  - `500` — Internal database/service error

---

## 19. Real-Time WebSocket

### `WS /api/v1/ws`

- **Description:** Open a WebSocket connection to receive real-time change events. Whenever a CRM entity is created, updated, or deleted, the server pushes a JSON event to all connected clients. This removes the need to manually refresh GET endpoints.
- **Authentication:** Required. Because browser WebSocket clients cannot send custom headers, pass the bearer token via the `token` query parameter. Non-browser clients may alternatively send `Authorization: Bearer <token>`.
- **Query Params:**
  - `token` (required) — active bearer session token obtained from `POST /api/v1/auth/login`.
  - `entities` (optional, comma-separated) — filter events by entity type. Example: `?entities=company,contact,deal`
- **Error (401 Unauthorized):**
```json
{
  "success": false,
  "message": "Invalid or expired token"
}
```
- **Message Format (server → client):**
```json
{
  "event": "change",
  "entity": "company",
  "action": "updated",
  "id": 5,
  "timestamp": "2026-07-20T12:34:56Z"
}
```
- **Actions:** `created`, `updated`, `deleted`
- **Entities:** `user`, `company`, `contact`, `deal_stage`, `deal`, `deal_discussion`, `activity`, `note`, `product`, `price_book`, `quote`, `quote_template`, `ticket`, `campaign`, `tag`, `notification`, `whatsapp_session`, `whatsapp_message`
- **Optional fields:** `id`, `payload`, and `timestamp` are omitted when the
  broadcaster does not supply them (for example, some session-wide changes).
- **WhatsApp semantics:** `whatsapp_session` is emitted for asynchronous
  pairing, connection, disconnection, failure, and logout state transitions.
  `whatsapp_message` is emitted after an inbound, outbound, or failed outbound
  message record has been persisted. Consumers should then refetch the relevant
  list/detail endpoint; an event is an invalidation signal, not a full record.
- **Recovery:** WebSocket delivery is transient and has no replay cursor. After
  reconnecting or resuming a suspended browser tab, refetch every actively
  subscribed resource once before continuing with live events.

### Example JavaScript client
```javascript
const token = 'your-bearer-token-from-login';
const ws = new WebSocket(
  `ws://localhost:5790/api/v1/ws?token=${token}&entities=company,contact`
);

ws.onmessage = (event) => {
  const change = JSON.parse(event.data);
  console.log('Real-time change:', change);
  // Refresh relevant list or detail view
};
```
