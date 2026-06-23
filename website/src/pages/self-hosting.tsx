import React from "react";
import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";
import Translate, { translate } from "@docusaurus/Translate";
import { IconArrow, IconCheck, IconGitHub } from "@site/src/components/marketing";
import { useScrollReveal } from "@site/src/lib/useScrollReveal";
import "@site/src/css/landing.css";

const GH_URL = "https://github.com/notacamp/campbooks";
const APP_URL = "https://app.campbooks.not-a-camp.com";
const INSTALL_URL = "/docs/getting-started/installation";

function Terminal() {
  return (
    <div className="cb-terminal" role="img" aria-label={translate({ id: "selfhost.terminal.ariaLabel", message: "Terminal: clone, configure, and run Campbooks" })}>
      <div className="cb-terminal__dots" aria-hidden="true"><i /><i /><i /></div>
      <div className="cb-terminal__line"><span className="cb-terminal__p">$ </span><span className="cb-terminal__c">git clone</span> https://github.com/notacamp/campbooks</div>
      <div className="cb-terminal__line"><span className="cb-terminal__p">$ </span><span className="cb-terminal__c">cd</span> campbooks && <span className="cb-terminal__c">cp</span> .env.example .env</div>
      <div className="cb-terminal__line cb-terminal__o"># add your Zoho + Claude keys to .env</div>
      <div className="cb-terminal__line"><span className="cb-terminal__p">$ </span>bin/setup && bin/rails server</div>
      <div className="cb-terminal__line cb-terminal__o">→ Campbooks running on http://localhost:3000</div>
    </div>
  );
}

const WHY = [
  { nameId: "selfhost.why.item1.name", name: "Your data never leaves", descId: "selfhost.why.item1.desc", desc: "Every email, attachment, and document stays on infrastructure you control. Nothing is sent to us, ever." },
  { nameId: "selfhost.why.item2.name", name: "Free to self-host", descId: "selfhost.why.item2.desc", desc: "No seats, no usage tiers, no per-user pricing. Clone it and run as many workspaces as you like." },
  { nameId: "selfhost.why.item3.name", name: "Bring your own AI", descId: "selfhost.why.item3.desc", desc: "Point Campbooks at Claude or any OpenAI-compatible endpoint. Your keys, your models, your bill." },
];

const GET = [
  { id: "selfhost.get.item1", label: "Unlimited email accounts" },
  { id: "selfhost.get.item2", label: "AI document classification" },
  { id: "selfhost.get.item3", label: "Review & approval workflow" },
  { id: "selfhost.get.item4", label: "Full-text search across everything" },
  { id: "selfhost.get.item5", label: "Email labels & document tags" },
  { id: "selfhost.get.item6", label: "Workflow automations" },
  { id: "selfhost.get.item7", label: "Team collaboration" },
  { id: "selfhost.get.item8", label: "Sustainable Use License, no strings" },
];

const STACK = ["Ruby on Rails 8", "PostgreSQL", "Hotwire", "Solid Queue", "Phlex", "Tailwind CSS", "Claude", "Docker"];

export default function SelfHosting(): React.ReactElement {
  useScrollReveal();
  return (
    <Layout
      title={translate({ id: "selfhost.page.title", message: "Self-hosting Campbooks" })}
      description={translate({ id: "selfhost.page.description", message: "Run Campbooks on your own server. One Rails app, one Postgres database, your own AI keys. Source-available under the Sustainable Use License." })}
    >
      <div className="cb-page">
        {/* Hero */}
        <header className="cb-hero" style={{ textAlign: "left", paddingBottom: "1rem" }}>
          <div className="cb-hero__halo" aria-hidden="true" />
          <div className="cb-container">
            <div className="cb-sh-hero">
              <div className="cb-sh-hero__text">
                <p className="cb-eyebrow cb-reveal"><Translate id="selfhost.hero.eyebrow">Self-hosting</Translate></p>
                <h1 className="cb-display cb-h1 cb-reveal" style={{ "--cb-delay": "80ms", fontSize: "clamp(2.4rem, 1.6rem + 3.4vw, 4rem)" } as React.CSSProperties}>
                  <Translate id="selfhost.hero.title">Run Campbooks on your own metal.</Translate>
                </h1>
                <p className="cb-prose cb-reveal" style={{ "--cb-delay": "160ms", marginTop: "1.4rem" } as React.CSSProperties}>
                  <Translate id="selfhost.hero.prose">One Rails app, one Postgres database, your own AI keys. Clone it, set a few environment variables, and own your paperwork stack end to end.</Translate>
                </p>
                <div className="cb-actions cb-reveal" style={{ "--cb-delay": "240ms", marginTop: "1.9rem" } as React.CSSProperties}>
                  <Link className="cb-btn cb-btn--primary cb-btn--lg" to={INSTALL_URL}>
                    <Translate id="selfhost.hero.cta.install">Installation guide</Translate> <IconArrow />
                  </Link>
                  <Link className="cb-btn cb-btn--ghost cb-btn--lg" to={GH_URL}>
                    <IconGitHub /> <Translate id="selfhost.hero.cta.github">View on GitHub</Translate>
                  </Link>
                </div>
                <div className="cb-trust cb-reveal" style={{ "--cb-delay": "320ms" } as React.CSSProperties}>
                  <span><Translate id="selfhost.hero.trust.mit">Source-available</Translate></span>
                  <span className="cb-trust__dot" />
                  <span><Translate id="selfhost.hero.trust.seats">No seats, no limits</Translate></span>
                  <span className="cb-trust__dot" />
                  <span><Translate id="selfhost.hero.trust.schedule">Update on your own schedule</Translate></span>
                </div>
              </div>
              <div className="cb-reveal" style={{ "--cb-delay": "300ms" } as React.CSSProperties}>
                <Terminal />
              </div>
            </div>
          </div>
        </header>

        {/* Why self-host */}
        <section className="cb-section">
          <div className="cb-container">
            <div className="cb-head cb-reveal" style={{ marginBottom: "2.5rem" }}>
              <p className="cb-eyebrow"><Translate id="selfhost.why.eyebrow">Why self-host</Translate></p>
              <h2 className="cb-display cb-h2"><Translate id="selfhost.why.title">Keep the whole thing under your roof.</Translate></h2>
            </div>
            <div className="cb-why cb-reveal">
              {WHY.map((w) => (
                <div className="cb-why__item" key={w.name}>
                  <div className="cb-why__name"><Translate id={w.nameId}>{w.name}</Translate></div>
                  <p className="cb-why__desc"><Translate id={w.descId}>{w.desc}</Translate></p>
                </div>
              ))}
            </div>
          </div>
        </section>

        {/* What you get */}
        <section className="cb-os">
          <div className="cb-container cb-section">
            <div className="cb-head cb-reveal">
              <p className="cb-eyebrow"><Translate id="selfhost.get.eyebrow">In the box</Translate></p>
              <h2 className="cb-display cb-h2"><Translate id="selfhost.get.title">Every feature, nothing held back.</Translate></h2>
              <p className="cb-prose">
                <Translate id="selfhost.get.prose">The self-hosted edition is the full product. There is no "pro" tier kept behind a paywall, the cloud simply saves you the deploy.</Translate>
              </p>
            </div>
            <ul className="cb-get cb-reveal">
              {GET.map((g) => (
                <li key={g.id}><IconCheck /> <Translate id={g.id}>{g.label}</Translate></li>
              ))}
            </ul>
            <div className="cb-reveal">
              <p className="cb-eyebrow" style={{ marginTop: "3rem" }}><Translate id="selfhost.stack.eyebrow">Built on</Translate></p>
              <div className="cb-stackrow">
                {STACK.map((s) => (
                  <span className="cb-stack-badge" key={s}>{s}</span>
                ))}
              </div>
            </div>
          </div>
        </section>

        {/* CTA back to cloud */}
        <section className="cb-section cb-cta">
          <div className="cb-container">
            <div className="cb-cta__inner cb-reveal">
              <h2 className="cb-display cb-h2"><Translate id="selfhost.cta.title">Prefer we run it for you?</Translate></h2>
              <p className="cb-prose" style={{ margin: "1rem auto 0" }}>
                <Translate id="selfhost.cta.prose">Skip the server. Open the hosted cloud and start sorting in seconds, then self-host whenever you're ready.</Translate>
              </p>
              <div className="cb-actions">
                <Link className="cb-btn cb-btn--primary cb-btn--lg" to={APP_URL}>
                  <Translate id="selfhost.cta.cloud">Use the cloud</Translate> <IconArrow />
                </Link>
                <Link className="cb-btn cb-btn--ghost cb-btn--lg" to={INSTALL_URL}>
                  <Translate id="selfhost.cta.install">Read the install guide</Translate>
                </Link>
              </div>
            </div>
          </div>
        </section>
      </div>
    </Layout>
  );
}
