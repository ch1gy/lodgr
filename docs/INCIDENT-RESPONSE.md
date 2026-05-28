# Incident Response Plan

> **Disclaimer:** This document reflects Lodgr's best-effort implementation of data
> privacy principles inspired by the Swiss Federal Act on Data Protection (FADP / nDSG,
> in force 1 September 2023) and the Kenya Data Protection Act 2019. It does not
> constitute formal legal compliance certification. Consult a qualified data protection
> lawyer for formal compliance assessment. Deployers are responsible for adapting this
> plan to their own jurisdiction and filling in the contacts below.

---

## Who owns this

The deployer is the **data controller** and owns all breach response obligations.
Lodgr (the software) provides detection tools; the deployer is responsible for
acting on what they find.

**Incident response contact:** [Fill in — name and email of the person responsible]
**Supervisory authority contact:** See jurisdiction guidance below.

---

## What counts as a reportable breach

A personal data breach is any event that leads to the accidental or unlawful
destruction, loss, alteration, unauthorised disclosure of, or access to personal data.

Examples that require assessment:

| Event | Likely reportable? |
|---|---|
| Unauthorised login to the desk account | Yes — assess scope of access |
| Database file copied or exfiltrated | Yes — all client data affected |
| Export file not deleted, accessed by unintended party | Yes — decrypted plaintext |
| Server compromise (OS-level) | Yes — full data exposure |
| A client's magic link URL leaked (e.g. via email provider) | Possibly — scoped 1-hour token; assess actual use |
| Refresh token replay detected (all sessions wiped by system) | Possibly — assess if data was accessed |
| Log files accessed by unauthorised party | Possibly — contains IPs and event metadata |
| Accidental deletion of data without prior export | Internal — assess impact on data subject rights |

---

## Response steps

### 1. Contain

Take immediate steps to limit ongoing exposure:

- **Revoke all client sessions** for affected accounts — use `DELETE /admin/clients/:id/sessions` per client, or shut down the server if a full breach is suspected
- **Rotate `JWT_SECRET`** in `.env` and restart the server — this immediately invalidates all issued access tokens
- **Rotate `ENCRYPTION_PASSPHRASE` and `ENCRYPTION_SALT`** only if the `.env` file itself was compromised — **note:** changing these values renders all stored message content permanently unreadable; only do this if the key is confirmed compromised and data integrity is secondary
- **Take the system offline** if the scope of the breach is unknown and ongoing access is possible

### 2. Assess

Determine the scope:

- What data was affected? (user accounts, tickets, thread content, attachments, exports, logs)
- How many data subjects are affected?
- Was the data encrypted? (thread bodies: yes, AES-256-GCM; user profiles and ticket metadata: no)
- How was the breach possible? (compromised credentials, server vulnerability, misconfiguration)
- Is the breach ongoing or contained?
- What is the risk to affected individuals? (identity theft, harassment, professional harm)

**Use Lodgr's built-in detection capabilities:**
- **Structured audit logs** in `logs/` — check for unusual login activity, unexpected export generation, or high volumes of failed attempts
- **Refresh token reuse detection** — if a compromised refresh token was replayed, Lodgr automatically wiped all sessions for that user and logged `"refresh token reuse detected — all sessions revoked"` at WARN level
- **Account lockout events** — repeated failed logins trigger exponential backoff and are logged at WARN with IP and email
- **Export events** — every export is logged with `desk_user_id`, `client_id`, and filename; unexpected exports are a signal of exfiltration

### 3. Document

Record the following while the investigation is ongoing:

- Date and time the breach was discovered
- Date and time it is estimated to have begun
- How it was discovered
- What data was affected (categories, estimated number of records)
- Number of data subjects affected
- Containment actions taken and when
- Whether data was encrypted at the time of exposure

Keep this record regardless of whether formal notification is required.

### 4. Notify affected clients

If the breach is likely to result in a **high risk to the rights and freedoms** of
the affected individuals (e.g. identity theft, physical harm, significant financial
impact), notify each affected client directly. Include:

- What happened
- What data was involved
- Steps they should take (e.g. be alert to phishing, change passwords on other services)
- What you are doing about it
- Who to contact with questions

### 5. Report to supervisory authority

---

## Jurisdiction guidance

### Switzerland

**Supervisory authority:** Federal Data Protection and Information Commissioner (FDPIC / EDÖB)
**Website:** https://www.edoeb.admin.ch
**Notification requirement:** Report within **72 hours** of discovering a breach that
is likely to result in a high risk to the personality or fundamental rights of the
affected persons (revised FADP Art. 24).

The 72-hour clock starts when you have **reasonable certainty** that a breach has
occurred — not when the investigation is complete.

### Kenya

**Supervisory authority:** Office of the Data Protection Commissioner (ODPC)
**Website:** https://odpc.go.ke
**Notification requirement:** Under the Kenya Data Protection Act 2019, notify the
Commissioner and affected data subjects as soon as reasonably practicable after
becoming aware of a breach.

### European Union / EEA

**Supervisory authority:** The relevant national data protection authority in your
member state (e.g. CNIL for France, ICO for the UK, BfDI for Germany).
**Notification requirement:** 72 hours from awareness under GDPR Art. 33.

### Other jurisdictions

Consult local data protection law. When in doubt, notification is almost always the
safer choice.

---

## Post-incident

After containment and notification:

- Conduct a root cause analysis
- Update this plan with any lessons learned
- Implement technical or process changes to prevent recurrence
- Consider whether open security findings (see `notes.md` — Security Posture) should be prioritised
