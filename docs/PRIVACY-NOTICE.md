# Privacy Notice — Lodgr Support System

> **Disclaimer:** This document reflects Lodgr's best-effort implementation of data
> privacy principles inspired by the Swiss Federal Act on Data Protection (FADP / nDSG,
> in force 1 September 2023) and the Kenya Data Protection Act 2019. It does not
> constitute formal legal compliance certification. Consult a qualified data protection
> lawyer for formal compliance assessment. Deployers are responsible for adapting this
> notice to their own jurisdiction and filling in the contact details below.

---

## Who runs this system

This support desk is operated by **[Deployer name — fill in before use]**. They are
the data controller for all personal data processed by Lodgr.

For any questions or requests about your data, contact:
**[Contact email or address — fill in before use]**

---

## What data we hold about you

When your account is created and while you use the system, Lodgr stores:

| Data | Why it is stored |
|---|---|
| **Your name** | To address you in communications and identify your account |
| **Your email address** | To log you in, send you ticket notifications, and deliver access links |
| **Your account creation date** | Administrative record |
| **Support ticket content** (title, description, status, priority, category) | To manage your support requests |
| **Message thread content** | Your conversations with the support desk. Stored encrypted at rest using AES-256-GCM — the desk operator cannot read message content without the system's encryption key |
| **File attachments** | Documents you upload as part of a support request |
| **Session information** | A token stored in your browser to keep you logged in. Only a SHA-256 hash is stored on the server — the raw token is never written to any persistent storage |
| **Login event data** | Timestamps and source IP addresses for successful and failed login attempts, retained in server log files for security monitoring |
| **Billing profile** (contact person, postal address, tax/VAT/PIN number, phone number) | To prepare and address invoices issued to you |
| **Invoices** (billing name, address, tax/PIN number, email, phone, line items, amounts, dates) | Statutory bookkeeping records required by applicable law |

We do **not** store: dates of birth, payment card or bank credentials, or any data
beyond what is listed above.

---

## How long your data is kept

| Data | Retention |
|---|---|
| Your account and all associated data | Retained until you request deletion. After a deletion request the account enters a 30-day recovery window, then is permanently deleted automatically |
| Message threads and attachments | Deleted together with your account |
| **Draft invoices** | Deleted together with your account |
| **Issued invoices** (status: sent or paid) | Retained for the statutory bookkeeping period — **10 years** (Swiss Code of Obligations, Art. 958f) / **5 years** (Kenya Tax Procedures Act) — even after account deletion, on the legal basis of statutory record-keeping obligations. Retained invoices are no longer linked to a login account and can no longer be accessed through this system by any party except the desk operator for bookkeeping purposes |
| Session tokens | Expired sessions cleaned automatically; all sessions revoked if suspicious activity is detected |
| Server log files | Retained for 30 days, then automatically deleted by log rotation |
| Data export files | Deleted from the server within 24 hours of generation or immediately upon download |

---

## Who can access your data

- **You** — you can view your own tickets and message threads through the system at
  any time using your login credentials or a one-time magic link
- **The desk operator** — the support agent can view and manage your tickets, create
  tickets on your behalf, and export your data when handling a data subject request
- **No one else** — Lodgr does not share your data with third parties, analytics
  services, or advertisers. If you use email notifications, your name, email address,
  and ticket title pass through the configured email provider in transit (see below)

---

## Email notifications

If the desk has configured email notifications, Lodgr will send you emails when:
- A new ticket is created for you
- Your ticket is updated or closed
- A one-time access link is generated for you

These emails contain your name, email address, and the ticket title. They are sent
via the desk operator's configured email provider (SMTP relay). If that provider
processes data outside Switzerland or the EU, this constitutes a cross-border
transfer. Ask the desk operator which provider they use if this is relevant to you.

Email bodies intentionally contain minimal information — no ticket message content
is ever included in notification emails.

---

## Your rights

You have the following rights regarding your personal data. To exercise any of them,
contact the desk operator at the address above.

**Access** — You may request a copy of all data Lodgr holds about you. The desk
operator will generate an export (a JSON file) containing your profile, all your
tickets, all message thread content in decrypted form, and all invoices issued to you.


**Rectification** — If your name or email address is incorrect, you may ask the
desk operator to correct it. The desk can update your profile directly.

**Erasure** — You may request that your account and all associated data be permanently
deleted. The desk operator will initiate deletion; a 30-day recovery window applies
before data is permanently removed. Immediate deletion can be requested explicitly.
Note: issued invoices (sent or paid) are retained for the statutory bookkeeping period
described in the retention table above even after account deletion — this is a legal
obligation, not discretion. Draft invoices are deleted with your account.

**Portability** — You may request a machine-readable export of your data. Lodgr
exports data as a structured JSON file containing your full profile, complete ticket
history, and all invoices issued to you.

**Objection** — If you have concerns about how your data is processed, contact the
desk operator. If you are not satisfied with their response, you may contact the
relevant data protection supervisory authority in your jurisdiction.

---

## Changes to this notice

If this notice is updated in a way that materially affects your rights, the desk
operator will inform you. The current version is always available from the desk
operator on request.
