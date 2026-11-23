# TODO: from beta to production

> What's left before DiBillSion can go live in the stores. [STATUS.md](STATUS.md) tracks
> day-to-day progress and the detailed backlog; this list is the bigger picture and its order.
> Items marked **(owner)** need a decision, money, an account or a physical action from the
> owner (CLAUDE.md, "Autonomy"). Items marked *added* weren't on the owner's first list.
> Tick items when done and link the PR or issue.

Order: **A → B → C** can go in parallel with **D**; **E (legal) must be done before F (stores)**.

## A. Accounts (owner's item 3; needed by B)

Today a device is anonymous: the server only knows a random device token (ADR 0006) and "who am
I" is a per-device, unsynced choice. To know *who* edited something across devices, people need
accounts.

- [ ] **(owner)** Decide the sign-in method. The owner's request is email + username +
      password. SPEC §1.1 says accounts are optional and plans email link, Sign in with Apple and
      Google. Choose between, or combine:
      - email + password: needs password reset and email verification, so a transactional email
        provider **(owner: account, maybe cost)**;
      - email link or one-time code (no password to leak or forget; same email provider);
      - passkeys;
      - Sign in with Apple / Google. If Google sign-in is offered, Apple requires Sign in with
        Apple (or an equivalent privacy-preserving option) too.
- [ ] Decide whether accounts stay optional (offline-only use keeps working) or become required
      for shared groups. Update SPEC.
- [ ] ADR: account model, how an account links to a group member (replaces the device-local
      "who am I" for signed-in users), sessions and tokens, and how existing anonymous devices
      and their groups move into an account without losing data.
- [ ] Server: `users` table, password hashing with Argon2id if passwords are used, login rate
      limits and lockout, session or refresh tokens stored hashed (as device tokens already are),
      log out everywhere.
- [ ] Apps: sign up, sign in, sign out, forgot password, change email/password, in Android and
      iOS, en + es.
- [ ] **Account and data deletion inside the app.** Both stores require it once accounts exist.
      Decide what happens to the shared group history of a deleted user: anonymise the name and
      keep the amounts, so the other members' balances still add up.
- [ ] Data export for the user (GDPR access and portability): CSV or JSON of their groups.

## B. Editing and history

- [ ] **Edit expenses at any moment, by any member** (owner's item 1). Today neither app edits
      a saved expense, only item shares. Every field: title, amount, payer, date, currency and
      rate, items, split. The sync model already supports this: the whole transaction is one
      `Body` field, last writer wins (ADR 0004/0005). Also editing incomes and transfers.
- [ ] **(owner)** Decide whether anyone can also delete any expense, or only its creator.
      SPEC §5 already asks for confirmation on destructive actions.
- [ ] **History of every addition, change and deletion** (owner's item 2). The op log already
      stores every change on every device and on the server (who: device; when: Lamport clock
      and time; what: field and value). So the history mostly needs:
      - [ ] attributing each op to an account or member, not just a device (needs A);
      - [ ] ADR: where history lives and how long it's kept (device op log, server op table, or
            both) and whether it can be pruned (op log compaction would lose it);
      - [ ] a readable "what changed" (before → after) per expense, and a group activity feed;
      - [ ] *added*: undoing a deletion from the history (tombstones keep the data).
- [ ] *added*: concurrent-edit behaviour. Two people editing the same expense offline: the last
      writer wins for the whole body. Decide whether the history must show the overwritten
      version (it can: the losing op is still in the log).

## C. Group screen

- [ ] **"Who am I" / "Cambiar quién soy" placement** (owner's item 5). Proposal: move it into
      the *Participantes* screen (reached from the ⋯ menu since the "Mi Resumen" work), as a
      "This is me" action on a person's row, plus a short entry in the ⋯ menu that opens it.
      With accounts (A) it becomes "link my account to this person", so design it with that
      in mind.

## D. Server security (owner's item 4)

Already in place on the Hetzner VPS (`deploy/README.md`): SSH with keys only and no root login,
`ufw` allowing only SSH/80/443, HTTPS through Caddy, unattended security upgrades, nightly
PostgreSQL dumps with an off-site copy, device tokens stored as SHA-256 hashes, PostgreSQL not
exposed to the internet.

Proposed, to research and decide in an ADR:
- [ ] Hetzner Cloud Firewall in front of the VPS (a second layer if `ufw` is misconfigured);
      restrict SSH to known IPs or put it behind Tailscale.
- [ ] `fail2ban` (or equivalent) for SSH; rate limits in the server: device registration per IP,
      pushes per device, logins per account and IP (STATUS: "Server hardening").
- [ ] Encryption at rest: encrypt backups (`age` or `gpg`) before they leave the server and on
      the Windows PC; decide on an encrypted volume for the database. Receipt photos are personal
      data stored on the server.
- [ ] Encryption in transit is done (TLS). Add HSTS and security headers in Caddy.
- [ ] Secrets: keep `.env` out of git (done), rotate the database password and the VLM API key,
      document who holds which key.
- [ ] Request logging without bodies or tokens; alerts on errors, disk space and failed
      backups; uptime check.
- [ ] Restore drill: restore a backup into a scratch database every few months.
- [ ] Data retention: delete receipt photos after N days or when the expense is deleted
      (STATUS: "receipt photo cleanup"); delete devices unused for a long time.
- [ ] Dependency and container updates: Dependabot and `cargo deny` are on; add image
      rebuilds for base-image security fixes.
- [ ] *added*: an external security review or penetration test before production, or at least
      a self-review against the OWASP API Security Top 10.

## E. Legal and professional review (owner's item 7; before F)

- [ ] **(owner)** Talk to a lawyer or data-protection adviser (Spain/EU). Questions to bring:
  - GDPR / LOPDGDD: who is the data controller, legal basis, privacy policy, records of
    processing, whether a data protection officer or an impact assessment is needed;
  - processors and international transfers: Hetzner (hosting; confirm the server's location) and the receipt AI provider
    (Anthropic, US): data processing agreements, transfer safeguards, what the receipt photos
    contain;
  - terms of use and liability (the app calculates who owes whom but moves no money); confirm
    it isn't a regulated payment service;
  - minimum age; cookie or tracking rules for the website `dibillsion.com`;
  - name and trademark: check "DiBillSion" (or the final name) before publishing;
  - if it will ever earn money: tax, invoicing and the stores' payment rules;
  - open-source licences of dependencies (`cargo deny` checks them; the stores need an
    attribution screen).
- [ ] Write the privacy policy and terms with that advice, host them on `dibillsion.com`, link
      them in the apps and the store listings.

## F. Store betas (owner's item 6)

- [ ] **(owner)** Final app name, icon and Android `applicationId`. **The `applicationId`
      can't change after the first Play upload**, so decide before F.
- [ ] Apple: TestFlight already works for internal testers. External testing (anyone with a
      link) needs Beta App Review, the privacy labels and a privacy policy URL.
- [ ] Google Play **(owner: Play Console, one-time US$25, identity verification)**: upload the
      signed bundle (AAB) to internal testing. New personal accounts must run a closed test
      with at least 12 testers for 14 days before production access.
- [ ] Store listings in en + es: description, screenshots (*added*: generate them in CI), Play
      Data safety form, Apple privacy nutrition labels (declare that receipt images go to a
      cloud AI).
- [ ] Production release once testers' feedback is in (M8).

## G. Quality before production (*added*)

- [ ] Check sync with two real devices (M5 exit criterion; needs a second phone or an
      emulator **(owner)**).
- [ ] Check iOS Universal Links for invites.
- [ ] Version compatibility: older apps silently drop sync ops for fields they don't know. Add a
      minimum supported app version on the server and an "update the app" message before adding
      the account and history fields.
- [ ] Crash reporting that respects privacy (no third-party SDK that collects personal data),
      or at least a way for testers to send logs.
- [ ] Move `ReceiptDraft` (duplicated in Kotlin and Swift) into the Rust core; add Kotlin and
      Swift linters.
- [ ] Accessibility (TalkBack/VoiceOver, font scaling, contrast) and the rest of M6 polish
      (CSV/PDF export).
