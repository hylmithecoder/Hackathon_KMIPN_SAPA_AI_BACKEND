## API Endpoint Reference

All endpoints return a JSON envelope:

```json
{
  "success": true,
  "data": { ... }
}
```

Or on error:

```json
{
  "success": false,
  "message": "Detailed error context"
}
```

### Health

| Method | Path | Description |
|---|---|---|
| GET | `/health` | Liveness probe |
| GET | `/health/ready` | Readiness probe |

### Authentication & Users

| Method | Path | Description |
|---|---|---|
| POST | `/api/v1/auth/login` | Login and receive bearer token |
| POST | `/api/v1/auth/register` | Register a new CRM user |
| POST | `/api/v1/auth/logout` | Invalidate current session |
| GET | `/api/v1/users` | List all users |
| PUT | `/api/v1/users/{id}` | Update a user |
| DELETE | `/api/v1/users/{id}` | Delete a user |

### Companies

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/companies` | List companies |
| POST | `/api/v1/companies` | Create company |
| GET | `/api/v1/companies/{id}` | Get company |
| PUT | `/api/v1/companies/{id}` | Update company |
| DELETE | `/api/v1/companies/{id}` | Delete company |

### Contacts

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/contacts` | List contacts |
| POST | `/api/v1/contacts` | Create contact |
| GET | `/api/v1/contacts/{id}` | Get contact |
| PUT | `/api/v1/contacts/{id}` | Update contact |
| DELETE | `/api/v1/contacts/{id}` | Delete contact |
| GET | `/api/v1/contacts/{id}/tags` | List contact tags |
| POST | `/api/v1/contacts/{id}/tags` | Add tag to contact |
| DELETE | `/api/v1/contacts/{id}/tags/{tag_id}` | Remove tag from contact |

### Deal Stages

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/deal-stages` | List pipeline stages |
| POST | `/api/v1/deal-stages` | Create stage |
| GET | `/api/v1/deal-stages/{id}` | Get stage |
| PUT | `/api/v1/deal-stages/{id}` | Update stage |
| DELETE | `/api/v1/deal-stages/{id}` | Delete stage |
| PUT | `/api/v1/deal-stages/reorder` | Reorder stages |

### Deals

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/deals` | List deals |
| POST | `/api/v1/deals` | Create deal |
| GET | `/api/v1/deals/{id}` | Get deal |
| PUT | `/api/v1/deals/{id}` | Update deal |
| DELETE | `/api/v1/deals/{id}` | Delete deal |
| PUT | `/api/v1/deals/{id}/move-stage` | Move deal to another stage |

### Activities

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/activities` | List activities |
| POST | `/api/v1/activities` | Create activity |
| GET | `/api/v1/activities/{id}` | Get activity |
| PUT | `/api/v1/activities/{id}` | Update activity |
| DELETE | `/api/v1/activities/{id}` | Delete activity |
| PUT | `/api/v1/activities/{id}/done` | Mark activity completed |

### Notes

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/notes` | List notes |
| POST | `/api/v1/notes` | Create note |
| GET | `/api/v1/notes/{id}` | Get note |
| PUT | `/api/v1/notes/{id}` | Update note |
| DELETE | `/api/v1/notes/{id}` | Delete note |

### Products

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/products` | List products |
| POST | `/api/v1/products` | Create product |
| GET | `/api/v1/products/{id}` | Get product |
| PUT | `/api/v1/products/{id}` | Update product |
| DELETE | `/api/v1/products/{id}` | Delete product |

### Quotes

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/quotes` | List quotes |
| POST | `/api/v1/quotes` | Create quote with items |
| GET | `/api/v1/quotes/{id}` | Get quote |
| PUT | `/api/v1/quotes/{id}` | Update quote |
| DELETE | `/api/v1/quotes/{id}` | Delete quote |
| PUT | `/api/v1/quotes/{id}/status` | Update quote status |
| GET | `/api/v1/quotes/{id}/items` | List quote items |

### Tickets

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/tickets` | List support tickets |
| POST | `/api/v1/tickets` | Create ticket |
| GET | `/api/v1/tickets/{id}` | Get ticket |
| PUT | `/api/v1/tickets/{id}` | Update ticket |
| DELETE | `/api/v1/tickets/{id}` | Delete ticket |
| PUT | `/api/v1/tickets/{id}/status` | Change ticket status |

### Campaigns

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/campaigns` | List campaigns |
| POST | `/api/v1/campaigns` | Create campaign |
| GET | `/api/v1/campaigns/{id}` | Get campaign |
| PUT | `/api/v1/campaigns/{id}` | Update campaign |
| DELETE | `/api/v1/campaigns/{id}` | Delete campaign |
| PUT | `/api/v1/campaigns/{id}/status` | Update campaign status |

### Tags

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/tags` | List tags |
| POST | `/api/v1/tags` | Create tag |
| GET | `/api/v1/tags/{id}` | Get tag |
| PUT | `/api/v1/tags/{id}` | Update tag |
| DELETE | `/api/v1/tags/{id}` | Delete tag |

### Notifications

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/notifications` | List notifications |
| POST | `/api/v1/notifications` | Create notification |
| GET | `/api/v1/notifications/unread-count` | Unread count |
| PUT | `/api/v1/notifications/read-all` | Mark all read |
| PUT | `/api/v1/notifications/{id}/read` | Mark notification read |
| DELETE | `/api/v1/notifications/{id}` | Delete notification |

### WhatsApp

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/whatsapp/status` | Get WhatsApp session status |
| GET | `/api/v1/whatsapp/qr` | Get current pairing QR string |
| POST | `/api/v1/whatsapp/connect` | Start WhatsApp pairing |
| POST | `/api/v1/whatsapp/send` | Send a text message |
| POST | `/api/v1/whatsapp/logout` | Disconnect and clear session |
| GET | `/api/v1/whatsapp/messages` | List recent outgoing messages |

### Authentication

Most endpoints require a `Authorization: Bearer <token>` header. Obtain a token via `POST /api/v1/auth/login`.

A default admin account is seeded on first startup:
- username: `admin`
- password: `admin123`
