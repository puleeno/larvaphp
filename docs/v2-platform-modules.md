# LarvaPHP v2 — Platform Modules Roadmap

## Feature Groups (v2)

### 1. Email
- Mail server integration (Postfix/Dovecot or Stalwart)
- Mailbox/domain management, aliases, autoresponders
- Webmail (Roundcube/SnappyMail) integration
- SPF/DKIM/DMARC automation

### 2. Backup
- Scheduled & on-demand backups (files + DB)
- Retention policies, incremental backups
- Off-site targets: S3, B2, SFTP, Google Drive
- One-click restore, backup verification

### 3. Open-Source Installer
- One-click installers for WordPress, Laravel, Joomla, Drupal, etc.
- Auto-create DB + user + vhost
- Post-install wizard (admin user, site URL)

### 4. Server
- Service control (start/stop/restart nginx/php/mysql/etc.)
- Resource monitoring (CPU/RAM/disk/network)
- Process manager, cron jobs

### 5. Security
- Firewall (UFW/firewalld) management
- ModSecurity WAF rules
- Fail2ban / brute-force protection
- SSL/TLS hardening, HSTS, CSP
- Malware scanner integration

### 6. Global System Settings
- PHP version management (install/switch)
- Timezone, locale, mail relay
- Default vhost template, global php.ini

### 7. Build Services
- CI/CD pipelines (GitHub Actions, GitLab CI)
- Build & deploy from Git repos
- Environment variables, secrets management

### 8. User & Plan
- Multi-user (admin/reseller/end-user)
- Plan/quotas (disk, bandwidth, DB count, email count)
- Billing integration (Stripe/PayPal)
- Usage reports

### 9. Connect
- DNS management (Cloudflare, Route53)
- CDN integration
- Remote server management (SSH to other nodes)

### 10. Community
- Plugin/extension marketplace
- Template sharing
- Forums/docs integration

## Enterprise License Features (proposed)
- Multi-tenant isolation & RBAC (admin/reseller/user roles)
- SSO/SAML/OIDC
- Audit log & compliance export (GDPR, SOC2)
- Advanced monitoring/alerting (Prometheus, Slack/PagerDuty)
- Automated backups with retention & off-site (S3/B2)
- WAF/ModSecurity integration
- Per-tenant rate limiting & quotas
- SLA/uptime monitoring
- White-label branding
- API tokens & webhooks
- Cluster/HA (multi-node)
- License key validation & feature gating
- Priority support & SLA
- Advanced analytics & reporting
- Backup encryption at rest
- IP allowlisting / 2FA enforcement
- Custom domain for panel

## Implementation Order (suggested)
1. Global system settings + PHP version management (foundation)
2. User & Plan (auth/quotas) — gates enterprise features
3. Backup (high value)
4. Security (firewall/fail2ban/WAF)
5. Email
6. Open-source installer
7. Build services / Connect / Community (later)
