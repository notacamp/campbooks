import React from "react";
import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";
import Translate, { translate } from "@docusaurus/Translate";
import {
  AppFrame,
  HomeMock,
  DocsMock,
  ScoutMock,
  CalendarMock,
  SkimMock,
  Connectors,
  IconArrow,
  IconCheck,
  IconClock,
  IconColumns,
  IconGitHub,
  IconSparkle,
  IconWorkflows,
} from "@site/src/components/marketing";
import { useScrollReveal } from "@site/src/lib/useScrollReveal";
import "@site/src/css/landing.css";

const APP_URL = "https://app.campbooks.not-a-camp.com";
const GH_URL = "https://github.com/notacamp/campbooks";

function Hero() {
  return (
    <header className="cb-hero">
      <div className="cb-hero__halo" aria-hidden="true" />
      <div className="cb-hero__content">
        <p className="cb-eyebrow cb-reveal">
          <Translate id="home.hero.eyebrow">Your paperwork, sorted</Translate>
        </p>
        <h1 className="cb-display cb-h1 cb-reveal" style={{ "--cb-delay": "80ms" } as React.CSSProperties}>
          <Translate id="home.hero.title">The inbox that sorts itself.</Translate>
        </h1>
        <p className="cb-hero__lede cb-reveal" style={{ "--cb-delay": "160ms" } as React.CSSProperties}>
          <Translate id="home.hero.lede">
            Campbooks reads your email and attachments, files the invoices, receipts, and
            contracts on its own, and hands you one short list of what actually needs you.
            So you can get back to running the business.
          </Translate>
        </p>
        <div className="cb-actions cb-reveal" style={{ "--cb-delay": "240ms" } as React.CSSProperties}>
          <Link className="cb-btn cb-btn--primary cb-btn--lg" to={`${APP_URL}/registration/new`}>
            <Translate id="home.hero.cta.start">Start the beta</Translate> <IconArrow />
          </Link>
          <Link className="cb-btn cb-btn--ghost cb-btn--lg" to="/self-hosting">
            <Translate id="home.hero.cta.selfhost">Self-host it</Translate>
          </Link>
        </div>
        <div className="cb-trust cb-reveal" style={{ "--cb-delay": "320ms" } as React.CSSProperties}>
          <span><Translate id="home.hero.trust.free">Free to self-host</Translate></span>
          <span className="cb-trust__dot" />
          <span><Translate id="home.hero.trust.mit">Source-available</Translate></span>
          <span className="cb-trust__dot" />
          <span><Translate id="home.hero.trust.data">Your data, your server</Translate></span>
        </div>
      </div>

      <div className="cb-showcase cb-reveal" style={{ "--cb-delay": "380ms" } as React.CSSProperties}>
        <div className="cb-showcase__halo" aria-hidden="true" />
        <div className="cb-showcase__frame">
          <span className="cb-float cb-float--tr">
            <IconCheck /> <Translate id="home.hero.float.sorted">Sorted into 4 groups</Translate>
          </span>
          <AppFrame active="Home">
            <HomeMock />
          </AppFrame>
          <span className="cb-float cb-float--bl">
            <IconSparkle /> <Translate id="home.hero.float.promos">1,381 promos, quietly filed</Translate>
          </span>
        </div>
      </div>
    </header>
  );
}

function ConnectorsSection() {
  return (
    <section className="cb-section" style={{ paddingBlock: "clamp(2.5rem, 4vw, 3.5rem)" }}>
      <div className="cb-container cb-reveal">
        <Connectors />
      </div>
    </section>
  );
}

function FeatureDocuments() {
  return (
    <div className="cb-feature cb-reveal">
      <div className="cb-feature__text">
        <p className="cb-eyebrow">
          <Translate id="home.feature.documents.eyebrow">Documents</Translate>
        </p>
        <h2 className="cb-display cb-h2">
          <Translate id="home.feature.documents.title">Every receipt and contract, filed the moment it lands.</Translate>
        </h2>
        <p className="cb-prose">
          <Translate id="home.feature.documents.prose">
            Attachments become searchable documents automatically, classified by type
            and tagged, ready to review, approve, or export. No folders to maintain, no
            dragging files around.
          </Translate>
        </p>
        <ul className="cb-feature__list">
          <li><IconCheck /> <Translate id="home.feature.documents.list.1">Auto-classified: invoices, receipts, contracts, statements</Translate></li>
          <li><IconCheck /> <Translate id="home.feature.documents.list.2">Full-text search across everything you've received</Translate></li>
          <li><IconCheck /> <Translate id="home.feature.documents.list.3">One-click approve, then export when you need it</Translate></li>
        </ul>
      </div>
      <div className="cb-feature__visual">
        <AppFrame active="Documents">
          <DocsMock />
        </AppFrame>
      </div>
    </div>
  );
}

function FeatureScout() {
  return (
    <div className="cb-feature cb-feature--reverse cb-reveal">
      <div className="cb-feature__text">
        <p className="cb-eyebrow">
          <Translate id="home.feature.scout.eyebrow">Scout</Translate>
        </p>
        <h2 className="cb-display cb-h2">
          <Translate id="home.feature.scout.title">An assistant that's already read everything.</Translate>
        </h2>
        <p className="cb-prose">
          <Translate id="home.feature.scout.prose">
            Scout briefs you each morning, answers questions about your paperwork in plain
            language, and turns "what do I owe, and to whom" into a one-line answer.
          </Translate>
        </p>
        <ul className="cb-feature__list">
          <li><IconCheck /> <Translate id="home.feature.scout.list.1">A morning briefing of what needs you, nothing more</Translate></li>
          <li><IconCheck /> <Translate id="home.feature.scout.list.2">Ask anything about your inbox and documents</Translate></li>
          <li><IconCheck /> <Translate id="home.feature.scout.list.3">Drafts replies you can read, tweak, and approve</Translate></li>
        </ul>
      </div>
      <div className="cb-feature__visual">
        <AppFrame active="Scout">
          <ScoutMock />
        </AppFrame>
      </div>
    </div>
  );
}

const CAPS = [
  { name: "Workflows", desc: "Trigger Slack pings, webhooks, and emails automatically when documents arrive." },
  { name: "Multi-account", desc: "Connect Zoho, Google Workspace, and Microsoft 365 inboxes side by side." },
  { name: "Tags & types", desc: "Custom document types and tags that match how you actually file things." },
  { name: "Approvals", desc: "Assign reviewers and track status from inbox all the way to signed-off." },
  { name: "Full-text search", desc: "Find any email or attachment by what's inside it, not just the subject." },
  { name: "Self-hosting", desc: "Run the whole thing on your own server under the Sustainable Use License, for free." },
];

const CAPS_TRANSLATED: { nameId: string; descId: string }[] = [
  { nameId: "home.caps.workflows.name", descId: "home.caps.workflows.desc" },
  { nameId: "home.caps.multiaccount.name", descId: "home.caps.multiaccount.desc" },
  { nameId: "home.caps.tags.name", descId: "home.caps.tags.desc" },
  { nameId: "home.caps.approvals.name", descId: "home.caps.approvals.desc" },
  { nameId: "home.caps.search.name", descId: "home.caps.search.desc" },
  { nameId: "home.caps.selfhosting.name", descId: "home.caps.selfhosting.desc" },
];

function Capabilities() {
  return (
    <section className="cb-section">
      <div className="cb-container">
        <div className="cb-head cb-reveal" style={{ marginBottom: "2.5rem" }}>
          <p className="cb-eyebrow">
            <Translate id="home.caps.eyebrow">More in the box</Translate>
          </p>
          <h2 className="cb-display cb-h2">
            <Translate id="home.caps.title">Quietly capable underneath the calm.</Translate>
          </h2>
        </div>
        <div className="cb-caps cb-reveal">
          {CAPS.map((c, i) => (
            <div className="cb-caps__item" key={c.name}>
              <span className="cb-caps__name">
                <Translate id={CAPS_TRANSLATED[i].nameId}>{c.name}</Translate>
              </span>
              <p className="cb-caps__desc">
                <Translate id={CAPS_TRANSLATED[i].descId}>{c.desc}</Translate>
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function WorkspaceBand() {
  return (
    <section className="cb-band cb-on-night">
      <div className="cb-container cb-section">
        <div className="cb-band__grid">
          <div className="cb-band__text cb-reveal">
            <p className="cb-eyebrow">
              <Translate id="home.workspace.eyebrow">The whole workspace</Translate>
            </p>
            <h2 className="cb-display cb-h2">
              <Translate id="home.workspace.title">Far more than an inbox.</Translate>
            </h2>
            <p className="cb-prose">
              <Translate id="home.workspace.prose">
                What started as an inbox is now a whole workspace: a two-way calendar,
                swipe-fast triage, a Kanban board, and reminders Scout pulls straight from
                your email.
              </Translate>
            </p>
            <ul className="cb-band__pills">
              <li className="cb-pill-ws"><IconColumns /> <Translate id="home.workspace.pill.board">Kanban board</Translate></li>
              <li className="cb-pill-ws"><kbd>⌘K</kbd> <Translate id="home.workspace.pill.palette">Command bar</Translate></li>
              <li className="cb-pill-ws"><IconClock /> <Translate id="home.workspace.pill.reminders">Smart reminders</Translate></li>
              <li className="cb-pill-ws"><IconWorkflows /> <Translate id="home.workspace.pill.integrations">Notion, Drive, Slack</Translate></li>
            </ul>
            <p className="cb-soon">
              <Translate id="home.workspace.soon">iOS and Android apps, coming soon</Translate>
            </p>
            <div className="cb-actions">
              <Link className="cb-btn cb-btn--primary" to={`${APP_URL}/registration/new`}>
                <Translate id="home.workspace.cta">Try the cloud version</Translate> <IconArrow />
              </Link>
            </div>
          </div>
          <div className="cb-band__visual cb-reveal" style={{ "--cb-delay": "120ms" } as React.CSSProperties}>
            <div className="cb-band__stack">
              <AppFrame active="Calendar" dark>
                <CalendarMock />
              </AppFrame>
              <div className="cb-band__skim" aria-hidden="true">
                <SkimMock />
              </div>
            </div>
            <p className="cb-band__caption">
              <Translate id="home.workspace.caption">Shown in dark mode. There's a light one too.</Translate>
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function OpenSource() {
  return (
    <section className="cb-os">
      <div className="cb-container cb-section">
        <div className="cb-os__grid">
          <div className="cb-os__text cb-reveal">
            <p className="cb-eyebrow">
              <Translate id="home.os.eyebrow">Source-available</Translate>
            </p>
            <h2 className="cb-display cb-h2">
              <Translate id="home.os.title">Yours to run. Forever.</Translate>
            </h2>
            <p className="cb-prose">
              <Translate id="home.os.prose">
                Campbooks is source-available under the Sustainable Use License. Start in seconds on our hosted
                cloud, or self-host it and keep every byte on your own server.
              </Translate>
            </p>
            <div className="cb-actions" style={{ marginTop: "1.75rem" }}>
              <Link className="cb-btn cb-btn--primary" to="/self-hosting">
                <Translate id="home.os.cta.selfhosting">Self-hosting guide</Translate> <IconArrow />
              </Link>
              <Link className="cb-btn cb-btn--ghost" to={GH_URL}>
                <IconGitHub /> <Translate id="home.os.cta.github">Star on GitHub</Translate>
              </Link>
            </div>
            <p className="cb-os__note">
              <Translate id="home.os.note">
                The hosted cloud resets periodically, so it's perfect for kicking the tires
                before you deploy your own.
              </Translate>
            </p>
          </div>
          <div className="cb-os__visual cb-reveal" style={{ "--cb-delay": "120ms" } as React.CSSProperties}>
            <div
              className="cb-terminal"
              role="img"
              aria-label={translate({
                id: "home.os.terminal.arialabel",
                message: "Terminal showing how to install and run Campbooks",
              })}
            >
              <div className="cb-terminal__dots" aria-hidden="true"><i /><i /><i /></div>
              <div className="cb-terminal__line"><span className="cb-terminal__p">$ </span><span className="cb-terminal__c">git clone</span> https://github.com/notacamp/campbooks</div>
              <div className="cb-terminal__line"><span className="cb-terminal__p">$ </span><span className="cb-terminal__c">cd</span> campbooks && bin/setup</div>
              <div className="cb-terminal__line"><span className="cb-terminal__p">$ </span><span className="cb-terminal__c">bin/rails</span> server</div>
              <div className="cb-terminal__line cb-terminal__o">
                <Translate id="home.os.terminal.output">→ Campbooks running on http://localhost:3000</Translate>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function FinalCTA() {
  return (
    <section className="cb-section cb-cta">
      <div className="cb-container">
        <div className="cb-cta__inner cb-reveal">
          <h2 className="cb-display cb-h2">
            <Translate id="home.cta.title">Clear the pile today.</Translate>
          </h2>
          <p className="cb-prose" style={{ margin: "1rem auto 0" }}>
            <Translate id="home.cta.prose">
              Open the hosted app and connect your first inbox in under two minutes.
            </Translate>
          </p>
          <div className="cb-actions">
            <Link className="cb-btn cb-btn--primary cb-btn--lg" to={`${APP_URL}/registration/new`}>
              <Translate id="home.cta.start">Start the beta</Translate> <IconArrow />
            </Link>
            <Link className="cb-btn cb-btn--ghost cb-btn--lg" to="/docs/getting-started/overview">
              <Translate id="home.cta.docs">Read the docs</Translate>
            </Link>
          </div>
        </div>
      </div>
    </section>
  );
}

export default function Home(): React.ReactElement {
  useScrollReveal();
  return (
    <Layout
      title={translate({ id: "home.meta.title", message: "The inbox that sorts itself" })}
      description={translate({
        id: "home.meta.description",
        message:
          "Campbooks reads your email and attachments, files the invoices, receipts, and contracts on its own, and hands you one short list of what needs you. Source-available and free to self-host, for professionals and small businesses.",
      })}
    >
      <div className="cb-page">
        <Hero />
        <ConnectorsSection />
        <section className="cb-section">
          <div className="cb-container">
            <FeatureDocuments />
            <FeatureScout />
          </div>
        </section>
        <Capabilities />
        <WorkspaceBand />
        <OpenSource />
        <FinalCTA />
      </div>
    </Layout>
  );
}
