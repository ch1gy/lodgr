# Data Subject Request Process

> **Disclaimer:** This document reflects Lodgr's best-effort implementation of data
> privacy principles inspired by the Swiss Federal Act on Data Protection (FADP / nDSG,
> in force 1 September 2023) and the Kenya Data Protection Act 2019. It does not
> constitute formal legal compliance certification. Consult a qualified data protection
> lawyer for formal compliance assessment. Deployers are responsible for adapting this
> process to their own jurisdiction.

---

## Your rights

As a client whose data is processed by this support system, you have the following
rights regarding your personal data:

- **Access** — you can request a copy of all data the system holds about you
- **Rectification** — you can request that inaccurate or outdated data be corrected
- **Erasure** — you can request that your data be permanently deleted
- **Portability** — you can request a machine-readable export of your data

---

## How to exercise your rights

Send an email to the desk operator at the address provided when your account was
created. A template is provided at the bottom of this document.

**Response time:** The desk operator will respond within **30 days** of receiving
your request. If the request is complex, they may inform you of an extension.

---

## What happens for each right

### Access

The desk operator will use Lodgr's built-in export function (`POST /admin/clients/:id/export`)
to generate a JSON file containing:

- Your profile: name, email, account creation date
- All your tickets: title, description, status, priority, category, type, dates
- All message thread content, decrypted and readable
- Attachment file paths for any uploaded files

The export file is provided to you directly. It is automatically deleted from the
server within 24 hours of generation.

**Note:** Internal desk notes are not included in client exports. These are the desk
operator's own working notes, equivalent to internal case notes.

---

### Rectification

The desk operator can update your name and email address directly in the system via
`PATCH /admin/clients/:id`. If you notice your name or email is incorrect, state the
correct value in your request and it will be updated.

Password corrections are handled separately — if you need to regain access to your
account, ask the desk to generate a magic link for you.

---

### Erasure

The desk operator will initiate account deletion in two stages:

1. **Soft delete** — your account and all associated tickets are marked as deleted.
   Sessions are immediately revoked. A **30-day recovery window** applies, during which
   deletion can be reversed if requested.

2. **Hard delete** — after 30 days, the system permanently deletes your account,
   tickets, message threads, attachments, and all associated data in a single
   cascading transaction. An export is generated before deletion as a pre-deletion
   record. All data is then irrecoverable.

If you need **immediate permanent deletion** (no 30-day window), state this
explicitly in your request. The desk operator can trigger hard deletion directly.

---

### Portability

The export described under Access is in structured JSON format — machine-readable and
not proprietary. The fields and structure are documented in the Lodgr codebase. The
file can be parsed by any JSON tool or imported into another system.

---

## Request template

Copy and fill in this template and send it to the desk operator's email address:

```
Subject: Data Request — [Your Name]

Hi,

I would like to request [access to / correction of / deletion of / a copy of]
my personal data held in your system.

My registered email address is: [your email]

[If correction: the correct value should be: ___]

[If deletion: I understand this will result in permanent deletion after a 30-day
recovery window. / I am requesting immediate permanent deletion without a recovery
window.]

Please respond within 30 days.

Thank you.
```

---

## Desk operator: handling a request

When a data subject request is received:

1. Verify the requester's identity matches a registered client email
2. Acknowledge receipt within 5 business days
3. Execute the appropriate action in Lodgr (export, profile update, or deletion)
4. Respond to the client with the outcome within 30 days of the original request
5. Keep a record of the request and your response (outside Lodgr — a simple log or
   email thread is sufficient)

If you need to refuse a request (for example, deletion is not possible while an
open legal obligation exists), explain the reason clearly in writing.
