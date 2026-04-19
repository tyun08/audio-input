# Pricing Strategy & Cost Model — Exploration

> **Status: Exploration / Alignment** — not a locked spec. See [Issue #43](https://github.com/tyun08/audio-input/issues/43) / [Issue #42](https://github.com/tyun08/audio-input/issues/42).

---

## 1. Usage & Cost Model

### Groq API Pricing (Whisper large-v3-turbo)

| Metric | Value |
|--------|-------|
| Price per hour of audio | $0.04 |
| Price per minute | ~$0.000667 |
| Price per second | ~$0.0000111 |

*Source: [Groq pricing page](https://groq.com/pricing) — cheapest high-quality Whisper inference available as of Q2 2026.*

---

### Typical Usage Profiles

| User Type | Daily Use | Monthly Minutes | Groq API Cost/mo | At 2× Markup | At 3× Markup |
|-----------|-----------|-----------------|------------------|--------------|--------------|
| **Casual** | 2–3 min/day | ~75 min | ~$0.05 | ~$0.10 | ~$0.15 |
| **Regular** | 10 min/day | ~300 min | ~$0.20 | ~$0.40 | ~$0.60 |
| **Power** | 30 min/day | ~900 min | ~$0.60 | ~$1.20 | ~$1.80 |
| **Heavy** | 60 min/day | ~1,800 min | ~$1.20 | ~$2.40 | ~$3.60 |

**Key insight:** Even power users burn less than $2/month at Groq wholesale. The margin opportunity comes from convenience (no API key required), not from obscuring the cost.

---

### Infrastructure Overhead (Managed Service)

Minimal backend (serverless + DB + auth) estimated at **$20–40/month flat** regardless of user count (up to ~1,000 users). This means:

- Break-even at $3/user/mo requires **7–14 paying subscribers** to cover infrastructure.
- Break-even at $5/user/mo requires **4–8 paying subscribers**.

---

### Profitability Estimate by Price Point

Assumptions: Regular user profile (300 min/month), 3× markup on Groq cost. LemonSqueezy fees: 5% + $0.50/transaction.

| Subscription Price | LemonSqueezy Fees | Groq Cost | Infra Share | Net Margin |
|--------------------|-------------------|-----------|-------------|------------|
| **$1/mo** | $0.55 | $0.60 | ~$0.04 | **−$0.19** ❌ (regular; only viable for casual < 150 min/mo) |
| **$3/mo** | $0.65 | $0.60 | ~$0.04 | **~$1.71** ✅ (most users) |
| **$5/mo** | $0.75 | $0.60 | ~$0.04 | **~$3.61** ✅ |

**Conclusion:** $3/month is solidly profitable for the vast majority of users once payment fees are accounted for. $1/month does not cover costs for a regular user profile — only works if the subscriber is a very casual user (< ~150 min/month). A $3/month floor is the right minimum.

---

### Usage Caps & Overage

At $3/month (3× markup):
- Included allowance: ~**1,200 min/month** (20 hrs) — more than any realistic user would consume
- Overage beyond cap: $0.003/min (3× Groq's $0.00067/min)

**Recommendation:** Set a generous included cap (20 hrs/month) and charge per-minute overage. Most users will never hit the cap, which simplifies billing and reduces support load.

---

## 2. Competitive Pricing Landscape

### Direct Competitors

| Product | Model | Price | Notes |
|---------|-------|-------|-------|
| **SuperWhisper** | Subscription | $9.99/mo or $59.99/yr | macOS only, on-device + cloud, most polished UI |
| **MacWhisper** | One-time | ~$22 perpetual | macOS, local Whisper only (no cloud), no AI polish |
| **Aqua Voice** | Subscription | ~$9/mo | Browser-based, LLM polish, target: knowledge workers |
| **Wispr Flow** | Subscription | $12/mo | macOS, AI dictation with GPT-4, heavy LLM integration |
| **Vinyasa (Whisper Memos)** | One-time + sub | $5.99/mo | Mobile-first, less desktop-focused |
| **Notta** | Freemium / Sub | $9–18/mo | Meeting transcription, not input-focused |

### Key Observations

1. **Price anchoring:** The market range for dedicated transcription tools is **$9–12/month** for subscriptions or **$10–25** one-time. We can position below this.
2. **"No-brainer cheap":** Below $5/month feels impulsive-buy territory. $3/month is near frictionless.
3. **One-time vs. subscription tension:** MacWhisper's perpetual model resonates with developers who hate subscriptions. SuperWhisper's subscription model funds ongoing cloud costs.
4. **Local vs. cloud:** Privacy-conscious users prefer local (see MacWhisper's appeal). Our BYOK free tier already addresses this crowd.

### Positioning Opportunity

We currently have an **extremely low cost base** vs. competitors (Groq is cheaper than Deepgram, AssemblyAI, or running OpenAI Whisper). This allows us to undercut on price while maintaining margin, or to position value more aggressively than competitors can afford.

---

## 3. Architecture Direction

### Recommended: Thin API Proxy + Light Account

After weighing the options, the recommendation is:

> **Our own lightweight API proxy, email magic link accounts, LemonSqueezy for billing.**

#### Why an API Proxy (not pure client-side key rotation)?

| Approach | Pros | Cons |
|----------|------|------|
| Client-side key rotation | Zero backend, simpler | Users could extract our key; no per-user metering; no rate limiting |
| **API proxy (recommended)** | Per-user metering, key never exposed to client, failover/provider swap transparent | Requires backend infra (but minimal) |

An API proxy is essential for:
- Protecting our Groq key from extraction
- Per-user usage tracking for billing
- Seamless provider failover (Groq → Deepgram fallback)
- Rate limiting to prevent abuse

**Minimal backend stack:** Cloudflare Workers (proxy) + Cloudflare D1 (usage DB) or Supabase — stays well within free tiers for early scale.

#### Account System: Email Magic Link

| Option | Privacy | Friction | Complexity |
|--------|---------|----------|------------|
| License key only | Best | Low | Low but no recovery |
| **Email magic link (recommended)** | Good | Low | Low |
| OAuth (GitHub/Google) | Moderate | Very low | Higher |

Email magic link is recommended because:
- No password to manage or forget
- Email is needed for receipts and subscription management regardless
- Minimal data collected (just email + usage stats)
- Works well with LemonSqueezy's customer system

#### Payment Processor: LemonSqueezy

| Processor | Developer UX | Tax Handling | Best For |
|-----------|--------------|--------------|----------|
| **LemonSqueezy** | Excellent | Automatic global (VAT/GST) | Indie devs, digital products |
| Stripe | Very good | Manual (requires Tax add-on) | Larger teams, custom flows |
| Paddle | Good | Automatic | B2B SaaS |

LemonSqueezy is the clear choice for an indie product:
- Merchant of Record → they handle all global tax compliance automatically
- Simple flat fee model (5% + $0.50/transaction)
- No need to register a legal entity in each tax jurisdiction
- Built-in subscription management and webhooks
- Easy license key issuance if we ever want a one-time model

#### Privacy: Minimal Data Storage

Data we **must** store:
- Email address (auth + receipts)
- Subscription status + plan
- Monthly usage minutes (for billing/overage)
- Credit balance (if credit packs offered)

Data we **do not** store:
- Audio recordings — never hit our servers
- Screenshots — captured locally; forwarded to Groq's vision LLM only when AI polish is enabled (not stored by our service)
- Transcription text — never logged
- IP addresses beyond rate-limiting window

This keeps us compliant with GDPR/CCPA by design. Update PRIVACY.md when managed service launches.

---

### Architecture Diagram (Managed Service Flow)

```
User Device                    Our Proxy                Groq API
─────────────────────────────────────────────────────────────────
app sends audio
  + auth token    ──────────►  validate token
                               check usage cap
                               forward to Groq  ──────► Whisper
                               log usage (mins)  ◄────── transcript
                 ◄────────────  return transcript
```

---

## 4. Draft Pricing Page Concept

### Tier Structure

**Two tiers is sufficient at launch. Three tiers adds complexity without clear benefit yet.**

---

```
┌─────────────────────────────────────┬─────────────────────────────────────┐
│           FREE                      │           PRO                        │
│           $0 / month                │           $3 / month                 │
│─────────────────────────────────────│─────────────────────────────────────│
│ ✓ Bring Your Own API Key            │ ✓ No API key needed                  │
│ ✓ Whisper large-v3-turbo            │ ✓ Whisper large-v3-turbo             │
│ ✓ AI polish (your key's quota)      │ ✓ AI polish included                 │
│ ✓ 50+ languages                     │ ✓ 50+ languages                      │
│ ✓ Unlimited (your quota)            │ ✓ 20 hrs/month included              │
│                                     │ ✓ Custom vocabulary (coming soon)    │
│                                     │ ✓ Priority support                   │
│ No account required                 │ Overage: $0.003/min beyond 20 hrs   │
└─────────────────────────────────────┴─────────────────────────────────────┘
```

### Copy Concepts

**Headline options:**
- "Voice input that just works. No API keys."
- "Pro plan — $3/month. Cheaper than one coffee."
- "We handle the API. You handle the talking."

**Value prop for Pro:**
> For most users, $3/month costs less than a single dollar of API credits would ever amount to — but without the account setup, key management, or worrying about hitting rate limits.

### Annual Discount (optional at launch)
- $3/month → $27/year (save 25%) — standard SaaS practice, improves cash flow

### Credit Packs (optional, defer to P2)
- $2 one-time → 500 min (~8 hrs) of extra minutes
- Rolls over for 6 months
- Good for seasonal heavy users (e.g., conference season)

---

## 5. Open Decisions & Risks

| Decision | Options | Recommendation |
|----------|---------|----------------|
| Launch price | $2 / $3 / $5 | **$3/mo** — low friction, sustainable margin |
| Annual plan | Yes / No | **Yes** at $27/year (~$2.25/mo) |
| Credit packs | Launch / Later | **Later (P2)** — complexity not needed at launch |
| Free trial | 7 days / usage-capped / None | **7-day trial** for Pro (1-month trial is trivially farmable with throwaway emails under magic-link auth) |
| One-time option | Yes / No | **Revisit after local inference ships** (zero marginal cost changes the calculus) |

### Key Risk: Groq Price Changes

Groq is currently the cheapest option. If pricing increases, our margins compress. **Mitigation:** API proxy architecture lets us swap providers transparently; build in a 4–5× markup buffer to absorb moderate price changes without repricing.

---

## Summary & Recommended Direction

| Area | Decision |
|------|----------|
| **Pricing** | $0 free (BYOK) + $3/month Pro ($27/year) |
| **Included quota** | 20 hrs/month for Pro (~630× typical casual usage) |
| **Backend** | Lightweight API proxy (Cloudflare Workers + D1) |
| **Auth** | Email magic link via LemonSqueezy customer portal |
| **Payments** | LemonSqueezy (MoR, automatic tax, indie-friendly) |
| **Data** | Email + usage minutes only; no audio/transcript storage |
| **Overage** | $0.003/min (~3× Groq cost) beyond 20 hrs |
| **Next step** | P1: account system + API proxy + LemonSqueezy integration |
