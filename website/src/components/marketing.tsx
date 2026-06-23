import React from "react";
import clsx from "clsx";

/* ============================================================
   Icons — inline, stroke style, matching the app's Heroicons feel
   ============================================================ */
type IconProps = { className?: string };

export const IconArrow = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M5 12h14M13 6l6 6-6 6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);
export const IconChevron = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M9 5l7 7-7 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);
export const IconSparkle = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M12 3l1.6 5.2L19 10l-5.4 1.8L12 17l-1.6-5.2L5 10l5.4-1.8L12 3z" fill="currentColor" />
  </svg>
);
export const IconPaperclip = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M21 11.5l-8.6 8.6a5 5 0 01-7-7l8.5-8.5a3.3 3.3 0 014.7 4.7l-8.5 8.5a1.7 1.7 0 01-2.4-2.4l7.8-7.8" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);
export const IconSearch = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="2" />
    <path d="M20 20l-3.2-3.2" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
  </svg>
);
export const IconCheck = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M5 13l4 4L19 7" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);
export const IconInbox = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M4 13l2.5-7h11L20 13M4 13v5h16v-5M4 13h4l1.5 2.5h5L16 13h4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);
export const IconSend = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
    <path d="M12 19V5M5 12l7-7 7 7" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);
export const IconGitHub = ({ className }: IconProps) => (
  <svg className={className} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <path d="M12 2C6.48 2 2 6.58 2 12.26c0 4.5 2.87 8.32 6.84 9.67.5.1.68-.22.68-.49l-.01-1.9c-2.78.62-3.37-1.2-3.37-1.2-.46-1.18-1.11-1.5-1.11-1.5-.9-.63.07-.62.07-.62 1 .07 1.53 1.05 1.53 1.05.9 1.56 2.34 1.11 2.91.85.09-.66.35-1.11.63-1.36-2.22-.26-4.55-1.14-4.55-5.07 0-1.12.39-2.03 1.03-2.75-.1-.26-.45-1.3.1-2.72 0 0 .84-.27 2.75 1.05a9.3 9.3 0 015 0c1.91-1.32 2.75-1.05 2.75-1.05.55 1.42.2 2.46.1 2.72.64.72 1.03 1.63 1.03 2.75 0 3.94-2.34 4.81-4.57 5.06.36.32.68.94.68 1.9l-.01 2.82c0 .27.18.6.69.49A10.03 10.03 0 0022 12.26C22 6.58 17.52 2 12 2z" />
  </svg>
);

/* Primary-nav icons — ported verbatim from the app's NavigationHelper so the
   rail mock matches the shipped icon rail exactly. */
const Stroke = ({ className, d }: IconProps & { d: React.ReactNode }) => (
  <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{d}</svg>
);
export const IconHome = ({ className }: IconProps) => (
  <Stroke className={className} d={<><path d="M3 11l9-8 9 8" /><path d="M5 10v10h5v-6h4v6h5V10" /></>} />
);
export const IconMail = ({ className }: IconProps) => (
  <Stroke className={className} d={<><rect x="3" y="5" width="18" height="14" rx="2" /><path d="m3 7 9 6 9-6" /></>} />
);
export const IconDocs = ({ className }: IconProps) => (
  <Stroke className={className} d={<><rect x="5" y="3" width="14" height="18" rx="2" /><path d="M9 8h6M9 12h6M9 16h3" /></>} />
);
export const IconWorkflows = ({ className }: IconProps) => (
  <Stroke className={className} d={<><circle cx="6" cy="6" r="2.4" /><circle cx="6" cy="18" r="2.4" /><circle cx="18" cy="12" r="2.4" /><path d="M8 7.4 16 11M8 16.6 16 13" /></>} />
);
export const IconBell = ({ className }: IconProps) => (
  <Stroke className={className} d={<><path d="M6 8a6 6 0 0 1 12 0c0 7 3 7 3 9H3c0-2 3-2 3-9" /><path d="M10 21a2 2 0 0 0 4 0" /></>} />
);
export const IconPeople = ({ className }: IconProps) => (
  <Stroke className={className} d={<><circle cx="9" cy="8" r="3" /><path d="M3 20a6 6 0 0 1 12 0" /><path d="M16 5.5a3 3 0 0 1 0 5M21 20a6 6 0 0 0-4-5.6" /></>} />
);
export const IconReceipt = ({ className }: IconProps) => (
  <Stroke className={className} d={<><path d="M5 3v18l2-1 2 1 2-1 2 1 2-1 2 1V3l-2 1-2-1-2 1-2-1-2 1z" /><path d="M9 8h6M9 12h6" /></>} />
);
export const IconTruck = ({ className }: IconProps) => (
  <Stroke className={className} d={<><rect x="2" y="7" width="12" height="9" rx="1" /><path d="M14 10h4l3 3v3h-7z" /><circle cx="6" cy="18" r="1.6" /><circle cx="17" cy="18" r="1.6" /></>} />
);
export const IconClock = ({ className }: IconProps) => (
  <Stroke className={className} d={<><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>} />
);
export const IconMessage = ({ className }: IconProps) => (
  <Stroke className={className} d={<path d="M21 15a2 2 0 0 1-2 2H8l-4 3V6a2 2 0 0 1 2-2h13a2 2 0 0 1 2 2z" />} />
);
export const IconCalendar = ({ className }: IconProps) => (
  <Stroke className={className} d={<><rect x="3" y="4.5" width="18" height="16" rx="2" /><path d="M3 9h18M8 2.5v4M16 2.5v4" /></>} />
);
export const IconColumns = ({ className }: IconProps) => (
  <Stroke className={className} d={<><rect x="3" y="4.5" width="5" height="15" rx="1.2" /><rect x="9.5" y="4.5" width="5" height="15" rx="1.2" /><rect x="16" y="4.5" width="5" height="15" rx="1.2" /></>} />
);

/* ============================================================
   Logo — the "Layered C", reproduced from Campbooks::Logo
   ============================================================ */
export function CbLogo({ size = 26, word = true, className }: { size?: number; word?: boolean; className?: string }) {
  return (
    <span className={clsx("cb-logo", className)}>
      <span className="cb-logo__tile" style={{ width: size, height: size, borderRadius: Math.round(size * 0.28) }}>
        <svg viewBox="0 0 28 28" fill="none" aria-hidden="true" style={{ width: size * 0.66, height: size * 0.66 }}>
          <path d="M20.65 17.46 A7.5 7.5 0 1 1 20.65 10.54" stroke="currentColor" strokeWidth="2.7" strokeLinecap="round" />
          <path d="M18.1 16.1 A4.6 4.6 0 1 1 18.1 11.9" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" opacity="0.55" />
        </svg>
      </span>
      {word && <span className="cb-logo__word">Campbooks</span>}
    </span>
  );
}

/* ScoutAvatar — gradient circle with a sparkle */
function ScoutAvatar({ size = 28 }: { size?: number }) {
  return (
    <span className="cb-scout-avatar" style={{ width: size, height: size }}>
      <IconSparkle className="cb-scout-avatar__spark" />
    </span>
  );
}

/* A small contact avatar (initial in a tinted circle) */
function Avatar({ initial, tone = "neutral" }: { initial: string; tone?: "neutral" | "accent" | "blue" | "green" | "amber" }) {
  return <span className={clsx("cb-avatar", `cb-avatar--${tone}`)}>{initial}</span>;
}

/* ============================================================
   App frame — a faithful mini Campbooks topbar + body
   ============================================================ */
type AppActive = "Home" | "Mail" | "Scout" | "Documents" | "Calendar" | "Workflows";

/* Instagram-shaped left icon rail — mirrors Campbooks::NavRail. Order and
   icons match the app; the active item is a near-black ink pill and Scout is
   the one Ember tile (the Meaning Rule). */
const RAIL: { key: AppActive; label: string; Icon: (p: IconProps) => React.ReactElement; ember?: boolean }[] = [
  { key: "Home", label: "Home", Icon: IconHome },
  { key: "Mail", label: "Mail", Icon: IconMail },
  { key: "Scout", label: "Scout", Icon: IconSparkle, ember: true },
  { key: "Documents", label: "Docs", Icon: IconDocs },
  { key: "Calendar", label: "Cal", Icon: IconCalendar },
  { key: "Workflows", label: "Flows", Icon: IconWorkflows },
];

export function AppFrame({
  children,
  active = "Home",
  dark = false,
  className,
}: {
  children: React.ReactNode;
  active?: AppActive;
  dark?: boolean;
  className?: string;
}) {
  return (
    <div className={clsx("cb-app", dark && "cb-mock--dark", className)}>
      <nav className="cb-navrail" aria-hidden="true">
        <CbLogo size={28} word={false} className="cb-navrail__logo" />
        {RAIL.map(({ key, label, Icon, ember }) =>
          ember ? (
            <span key={key} className="cb-navitem--scout" aria-current={active === key ? "page" : undefined}>
              <Icon />
            </span>
          ) : (
            <span key={key} className={clsx("cb-navitem", active === key && "is-active")} aria-current={active === key ? "page" : undefined}>
              <Icon />
              <span>{label}</span>
            </span>
          )
        )}
        <span className="cb-navrail__sp" />
        <span className="cb-navrail__foot">
          <span className="cb-navrail__btn"><IconSearch /></span>
          <span className="cb-navrail__btn"><IconBell /></span>
          <Avatar initial="A" tone="neutral" />
        </span>
      </nav>
      <div className="cb-app__body">{children}</div>
    </div>
  );
}

/* ============================================================
   Mock 0 — Instagram-shaped Home (story-rings + content feed)
   The marquee surface; mirrors the shipped home (tmp/design/home.html).
   ============================================================ */
const RINGS: { label: string; badge: string; live?: boolean; done?: boolean; Icon: (p: IconProps) => React.ReactElement }[] = [
  { label: "Needs you", badge: "5", live: true, Icon: IconBell },
  { label: "Invoices", badge: "3", live: true, Icon: IconDocs },
  { label: "Vendors", badge: "2", live: true, Icon: IconTruck },
  { label: "Receipts", badge: "✓", done: true, Icon: IconReceipt },
  { label: "Promos", badge: "✓", done: true, Icon: IconMail },
];

export function HomeMock() {
  return (
    <div className="cb-home">
      <div className="cb-home__scroll">
        <div className="cb-home__hello">Good morning, Alex.</div>
        <div className="cb-home__sub">47 sorted overnight. Tap a stack to skim, or work your feed.</div>

        <div className="cb-rings">
          {RINGS.map(({ label, badge, live, done, Icon }) => (
            <span key={label} className={clsx("cb-ring", live && "cb-ring--live", done && "cb-ring--done")}>
              <span className="cb-ring__o">
                <span className="cb-ring__i"><Icon /></span>
                <span className="cb-ring__badge">{badge}</span>
              </span>
              <span className="cb-ring__lab">{label}</span>
            </span>
          ))}
        </div>

        <div className="cb-feedlab">Your feed</div>

        <article className="cb-post">
          <div className="cb-post__head">
            <span className="cb-post__av">A</span>
            <div className="cb-post__id">
              <div className="cb-post__name">Acme Studio</div>
              <div className="cb-post__time">9:24 AM · billing@acmestudio.com</div>
            </div>
          </div>
          <div className="cb-post__subject">Invoice #1042 needs your sign-off</div>
          <div className="cb-post__content">
            Hi Alex, attaching invoice #1042 for the June retainer — €2,480, net 30. Same rate as
            the contract you approved in March.
          </div>
          <div className="cb-post__meta">
            <span className="cb-mchip">Invoice</span>
            <span className="cb-mchip"><IconPaperclip /><span className="cb-mchip__fn">invoice-1042.pdf</span></span>
            <span className="cb-mchip"><IconMessage />2 messages</span>
            <span className="cb-mchip cb-mchip--prio"><span className="cb-mchip__dot" />Priority</span>
          </div>
          <div className="cb-snote">
            <div className="cb-snote__head">
              <span className="cb-snote__av"><IconSparkle /></span>
              <span className="cb-snote__who">Scout</span>
              <span className="cb-snote__tag">AI</span>
              <span className="cb-snote__time">read it just now</span>
            </div>
            <div className="cb-snote__msg">
              Matches your approved March contract — nothing unusual. I drafted an approval reply,
              ready to send.
            </div>
          </div>
          <div className="cb-pacts">
            <span className="cb-btn cb-btn--ghost cb-btn--mini">Archive</span>
            <span className="cb-btn cb-btn--ghost cb-btn--mini">Reply</span>
            <span className="cb-btn cb-btn--primary cb-btn--mini"><IconCheck className="cb-btn__check" /> Approve &amp; send</span>
          </div>
        </article>

        <div className="cb-feedlab">Handled by Scout · 42</div>
        <div className="cb-hrow">
          <span className="cb-hrow__ck"><IconCheck /></span>
          <div className="cb-hrow__main">
            <span className="cb-hrow__name">Stripe</span> <span className="cb-hrow__did">receipt €420, filed to documents</span>
          </div>
          <span className="cb-hrow__time">8:01</span>
        </div>
      </div>
    </div>
  );
}

/* ============================================================
   Mock 1 — Grouped triage inbox (rail · list · reading pane)
   ============================================================ */
const GROUPS = [
  { name: "Finance", senders: "stripe, paypal, and more", count: "12", tone: "accent", initials: ["S", "P", "A"] },
  { name: "Notifications", senders: "github, loom, asana", count: "217", tone: "blue", initials: ["G", "L", "A"] },
  { name: "Personal", senders: "jamie, alex, and more", count: "8", tone: "green", initials: ["J", "A", "J"] },
  { name: "Promos", senders: "figma, notion, and more", count: "1.4k", tone: "neutral", initials: ["F", "N", "C"] },
] as const;

const CLASSIFIED = [
  { subject: "Invoice #1042", sender: "billing@acmestudio.com", initial: "A", tone: "accent", type: "Invoice", dot: "var(--cb-violet)", time: "9:24", unread: true, active: true },
  { subject: "Your receipt from Stripe", sender: "receipts@stripe.com", initial: "S", tone: "blue", type: "Receipt", dot: "var(--cb-green)", time: "8:01", unread: true, active: false },
  { subject: "Q3 vendor contract", sender: "legal@northwind.co", initial: "N", tone: "amber", type: "Contract", dot: "var(--cb-amber)", time: "Tue", unread: false, active: false },
] as const;

export function InboxMock() {
  return (
    <div className="cb-inbox">
      {/* folder rail */}
      <div className="cb-inbox__rail">
        <span className="cb-rail__item is-active"><IconInbox className="cb-rail__icon" /><b>99+</b></span>
        {["Sent", "Drafts", "Spam"].map((f, i) => (
          <span key={f} className="cb-rail__item"><span className="cb-rail__glyph" />{["", "13", "24"][i] && <b>{["", "13", "24"][i]}</b>}</span>
        ))}
        <span className="cb-rail__spacer" />
        <span className="cb-rail__item"><span className="cb-rail__glyph" /></span>
      </div>

      {/* grouped list */}
      <div className="cb-inbox__list">
        <div className="cb-list__head">
          <span className="cb-list__title">Inbox</span>
          <span className="cb-list__skim">Skim</span>
        </div>
        <div className="cb-sec">This Week</div>
        {GROUPS.map((g) => (
          <div className="cb-grow" key={g.name} style={{ "--tint": `var(--cb-tone-${g.tone})` } as React.CSSProperties}>
            <span className="cb-stack">
              {g.initials.map((ini, i) => (
                <span key={i} className={clsx("cb-stack__a", `cb-avatar--${g.tone}`)} style={{ left: i * 9, zIndex: 3 - i }}>{ini}</span>
              ))}
            </span>
            <span className="cb-grow__main">
              <span className="cb-grow__name">{g.name}</span>
              <span className="cb-grow__senders">{g.senders}</span>
            </span>
            <span className="cb-grow__count">{g.count}</span>
            <IconChevron className="cb-grow__chev" />
          </div>
        ))}
        <div className="cb-sec">Today</div>
        {CLASSIFIED.map((m) => (
          <div className={clsx("cb-row", m.active && "is-active", m.unread && "is-unread")} key={m.subject}>
            <span className="cb-row__avatar">
              <Avatar initial={m.initial} tone={m.tone as any} />
              {m.unread && <span className="cb-row__dot" />}
            </span>
            <span className="cb-row__main">
              <span className="cb-row__top">
                <span className="cb-row__subject">{m.subject}</span>
                <span className="cb-row__time">{m.time}</span>
              </span>
              <span className="cb-row__bottom">
                <span className="cb-row__sender">{m.sender}</span>
                <span className="cb-type" style={{ color: m.dot }}>
                  <span className="cb-type__dot" style={{ background: m.dot }} />
                  {m.type}
                </span>
              </span>
            </span>
          </div>
        ))}
      </div>

      {/* reading pane */}
      <div className="cb-inbox__pane">
        <div className="cb-pane__head">
          <div>
            <div className="cb-pane__subject">Invoice #1042</div>
            <div className="cb-pane__from">Acme Studio &middot; billing@acmestudio.com</div>
          </div>
          <span className="cb-badge cb-badge--accent">Invoice</span>
        </div>
        <div className="cb-pane__ai">
          <span className="cb-pane__ai-head"><IconSparkle className="cb-pane__ai-spark" /> Scout summary</span>
          <p>Invoice for <strong>€2,480</strong>, due <strong>Jul 3</strong>. Matches PO #88. No action needed until payment run.</p>
        </div>
        <div className="cb-pane__body">
          <p>Hi Alex, please find attached invoice 1042 for the June retainer. Payment terms net 30.</p>
          <span className="cb-chip">
            <IconPaperclip className="cb-chip__icon" />
            invoice-1042.pdf
            <span className="cb-chip__meta">142 KB</span>
          </span>
        </div>
        <div className="cb-pane__foot">
          <span className="cb-btn cb-btn--primary cb-btn--mini"><IconCheck className="cb-btn__check" /> Approve</span>
          <span className="cb-btn cb-btn--ghost cb-btn--mini">Reply</span>
        </div>
      </div>
    </div>
  );
}

/* ============================================================
   Mock 2 — Documents table
   ============================================================ */
const DOCS = [
  { entity: "Acme Studio", sub: "billing@acmestudio.com", type: "Invoice", dot: "var(--cb-violet)", date: "18/06/2026", ref: "#1042", status: "Approved", s: "approved", source: "Email" },
  { entity: "Stripe", sub: "receipts@stripe.com", type: "Receipt", dot: "var(--cb-green)", date: "18/06/2026", ref: "ch_3Q…", status: "Processed", s: "processed", source: "Email" },
  { entity: "Northwind Ltd", sub: "legal@northwind.co", type: "Contract", dot: "var(--cb-amber)", date: "17/06/2026", ref: "NW-2231", status: "Review", s: "review", source: "Upload" },
  { entity: "City Utilities", sub: "no-reply@cityutil.com", type: "Statement", dot: "oklch(58% 0.13 232)", date: "15/06/2026", ref: "Jun 2026", status: "Pending", s: "pending", source: "Email" },
  { entity: "Maria Santos", sub: "maria@freelance.io", type: "Invoice", dot: "var(--cb-violet)", date: "14/06/2026", ref: "#0087", status: "Approved", s: "approved", source: "Email" },
] as const;

export function DocsMock() {
  return (
    <div className="cb-docs">
      <div className="cb-docs__filters">
        <span className="cb-docs__search"><IconSearch className="cb-docs__search-icon" /> Search documents</span>
        <span className="cb-pill is-on">All types</span>
        <span className="cb-pill">This month</span>
        <span className="cb-docs__spacer" />
        <span className="cb-count">{DOCS.length} of 248</span>
      </div>
      <table className="cb-table">
        <thead>
          <tr>
            <th>Entity</th><th>Type</th><th>Date</th><th>Reference</th><th>Status</th><th className="cb-hide-sm">Source</th>
          </tr>
        </thead>
        <tbody>
          {DOCS.map((d) => (
            <tr key={d.entity + d.ref}>
              <td className="cb-td-entity">
                <span className="cb-td-entity__name">{d.entity}</span>
                <span className="cb-td-entity__sub">{d.sub}</span>
              </td>
              <td><span className="cb-type" style={{ color: d.dot }}><span className="cb-type__dot" style={{ background: d.dot }} />{d.type}</span></td>
              <td className="cb-td-muted">{d.date}</td>
              <td className="cb-td-muted">{d.ref}</td>
              <td><span className={clsx("cb-status", `cb-status--${d.s}`)}>{d.status}</span></td>
              <td className="cb-td-muted cb-hide-sm">{d.source}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/* ============================================================
   Mock 3 — Scout briefing
   ============================================================ */
const STATS = [
  { value: "3", label: "Need review", tone: "amber" },
  { value: "2", label: "Awaiting approval", tone: "accent" },
  { value: "€4.9k", label: "Due this week", tone: "green" },
];
const CHIPS = ["What do I owe this week?", "Summarize the Northwind contract", "Any invoices overdue?"];

export function ScoutMock() {
  return (
    <div className="cb-scout">
      <div className="cb-scout__briefing">
        <ScoutAvatar size={46} />
        <div className="cb-scout__greet">Good morning, Alex.</div>
        <div className="cb-scout__sub">Your desk is calm. Two things want a quick look, nothing is on fire.</div>
        <div className="cb-scout__stats">
          {STATS.map((s) => (
            <div className={clsx("cb-stat", `cb-stat--${s.tone}`)} key={s.label}>
              <span className="cb-stat__chip"><IconSparkle className="cb-stat__icon" /></span>
              <span className="cb-stat__value">{s.value}</span>
              <span className="cb-stat__label">{s.label}</span>
            </div>
          ))}
        </div>
        <div className="cb-scout__chips">
          {CHIPS.map((c) => (
            <span className="cb-suggest" key={c}>{c}</span>
          ))}
        </div>
      </div>
      <div className="cb-scout__compose">
        <span className="cb-scout__placeholder">Ask Scout anything…</span>
        <span className="cb-scout__send"><IconSend className="cb-scout__send-icon" /></span>
      </div>
    </div>
  );
}

/* ============================================================
   Mock 4 — Calendar (month view; two-way sync, email to event)
   ============================================================ */
type CalCell = { n: number; out?: boolean; today?: boolean; ev?: { t: string; k: "accent" | "blue" | "green" | "amber" } };
const CAL_DOW = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const CAL_CELLS: CalCell[] = [
  { n: 26, out: true }, { n: 27, out: true }, { n: 28, out: true }, { n: 29, out: true }, { n: 30, out: true }, { n: 31, out: true }, { n: 1 },
  { n: 2 }, { n: 3, ev: { t: "Rent due", k: "amber" } }, { n: 4 }, { n: 5 }, { n: 6 }, { n: 7 }, { n: 8 },
  { n: 9, ev: { t: "Northwind call", k: "accent" } }, { n: 10 }, { n: 11 }, { n: 12, ev: { t: "Team sync", k: "blue" } }, { n: 13 }, { n: 14 }, { n: 15 },
  { n: 16 }, { n: 17 }, { n: 18, today: true, ev: { t: "Invoice #1042 due", k: "accent" } }, { n: 19 }, { n: 20 }, { n: 21 }, { n: 22 },
  { n: 23 }, { n: 24, ev: { t: "Quarterly review", k: "green" } }, { n: 25 }, { n: 26 }, { n: 27 }, { n: 28 }, { n: 29 },
];
export function CalendarMock() {
  return (
    <div className="cb-cal">
      <div className="cb-cal__head">
        <span className="cb-cal__title">June 2026</span>
        <span className="cb-cal__views">
          <span>Day</span>
          <span>Week</span>
          <span className="is-on">Month</span>
        </span>
      </div>
      <div className="cb-cal__grid">
        {CAL_DOW.map((d) => (
          <span key={d} className="cb-cal__dow">{d}</span>
        ))}
        {CAL_CELLS.map((c, i) => (
          <div key={i} className={clsx("cb-cal__cell", c.out && "is-out", c.today && "is-today")}>
            <span className="cb-cal__num">{c.n}</span>
            {c.ev && <span className={clsx("cb-cal__ev", `cb-cal__ev--${c.ev.k}`)}>{c.ev.t}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}

/* ============================================================
   Mock 5 — Skim card (swipe story-mode triage; lives on the dark band)
   ============================================================ */
export function SkimMock() {
  return (
    <div className="cb-skim">
      <span className="cb-skim__peek" aria-hidden="true" />
      <div className="cb-skim-card">
        <div className="cb-skim__top">
          <span className="cb-skim__count">3 / 12</span>
          <span className="cb-skim__dots" aria-hidden="true"><i /><i /><i className="on" /><i /><i /></span>
        </div>
        <div className="cb-skim__avs" aria-hidden="true">
          <span>F</span><span>N</span><span>L</span><span>+9</span>
        </div>
        <div className="cb-skim__title">Promotions</div>
        <div className="cb-skim__sub">Figma, Notion, Linear and 9 more</div>
        <div className="cb-skim__note">Nothing here needs you.</div>
        <div className="cb-skim__acts">
          <span className="cb-skim__act cb-skim__act--archive">Archive all</span>
          <span className="cb-skim__act cb-skim__act--keep">Keep</span>
        </div>
      </div>
    </div>
  );
}

/* ============================================================
   Connectors strip — honest "works with" providers
   ============================================================ */
export function Connectors() {
  const providers = ["Zoho Mail", "Google Workspace", "Microsoft 365"];
  return (
    <div className="cb-connectors">
      <span className="cb-connectors__label">Connects to</span>
      <div className="cb-connectors__items">
        {providers.map((p) => (
          <span className="cb-connectors__item" key={p}>{p}</span>
        ))}
      </div>
    </div>
  );
}
