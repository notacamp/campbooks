import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";
import Heading from "@theme/Heading";
import Translate, { translate } from "@docusaurus/Translate";

const tiers = [
  {
    name: translate({ id: "pricing.tier.selfhosted.name", message: "Self-hosted" }),
    price: translate({ id: "pricing.tier.selfhosted.price", message: "Free" }),
    description: translate({ id: "pricing.tier.selfhosted.description", message: "Deploy on your own server. Full control, zero cost." }),
    features: [
      translate({ id: "pricing.tier.selfhosted.feature1", message: "Unlimited email accounts" }),
      translate({ id: "pricing.tier.selfhosted.feature2", message: "AI document classification" }),
      translate({ id: "pricing.tier.selfhosted.feature3", message: "Review and approval workflow" }),
      translate({ id: "pricing.tier.selfhosted.feature4", message: "Full-text search" }),
      translate({ id: "pricing.tier.selfhosted.feature5", message: "Email labels and tags" }),
      translate({ id: "pricing.tier.selfhosted.feature6", message: "Team collaboration" }),
      translate({ id: "pricing.tier.selfhosted.feature7", message: "MIT License" }),
    ],
    cta: { text: translate({ id: "pricing.tier.selfhosted.cta", message: "Installation guide" }), href: "/docs/getting-started/installation" },
  },
  {
    name: translate({ id: "pricing.tier.cloud.name", message: "Cloud" }),
    price: translate({ id: "pricing.tier.cloud.price", message: "Free during beta" }),
    badge: translate({ id: "pricing.tier.cloud.badge", message: "Closed beta" }),
    description: translate({ id: "pricing.tier.cloud.description", message: "Use our hosted instance. No server to run — create an account and start in seconds." }),
    features: [
      translate({ id: "pricing.tier.cloud.feature1", message: "Your own private workspace" }),
      translate({ id: "pricing.tier.cloud.feature2", message: "All features available" }),
      translate({ id: "pricing.tier.cloud.feature3", message: "No setup required" }),
      translate({ id: "pricing.tier.cloud.feature4", message: "Always up to date" }),
      translate({ id: "pricing.tier.cloud.feature5", message: "Access code required" }),
    ],
    cta: { text: translate({ id: "pricing.tier.cloud.cta", message: "Create your account" }), href: "https://app.campbooks.not-a-camp.com/registration/new" },
    highlighted: true,
  },
  {
    name: translate({ id: "pricing.tier.enterprise.name", message: "Enterprise" }),
    price: translate({ id: "pricing.tier.enterprise.price", message: "Coming soon" }),
    description: translate({ id: "pricing.tier.enterprise.description", message: "Managed hosting with priority support for teams." }),
    features: [
      translate({ id: "pricing.tier.enterprise.feature1", message: "Managed hosting" }),
      translate({ id: "pricing.tier.enterprise.feature2", message: "Priority support" }),
      translate({ id: "pricing.tier.enterprise.feature3", message: "Custom integrations" }),
      translate({ id: "pricing.tier.enterprise.feature4", message: "SLA guarantee" }),
      translate({ id: "pricing.tier.enterprise.feature5", message: "Automated backups" }),
      translate({ id: "pricing.tier.enterprise.feature6", message: "Dedicated instance" }),
    ],
    cta: { text: translate({ id: "pricing.tier.enterprise.cta", message: "Contact us" }), href: "mailto:inbox@not-a-camp.com" },
  },
];

const faqs = [
  {
    q: translate({ id: "pricing.faq1.q", message: "Is Campbooks really free?" }),
    a: translate({ id: "pricing.faq1.a", message: "Yes. Campbooks is open source under the MIT License. You can download, modify, and deploy it for free on your own infrastructure." }),
  },
  {
    q: translate({ id: "pricing.faq2.q", message: "What does the cloud tier include?" }),
    a: translate({ id: "pricing.faq2.a", message: "Your own private, persistent workspace on our hosted instance — the full product, no setup. It's in closed beta, so signup needs an access code: email inbox@not-a-camp.com to request one." }),
  },
  {
    q: translate({ id: "pricing.faq3.q", message: "Can I connect any email provider?" }),
    a: translate({ id: "pricing.faq3.a", message: "Campbooks supports Zoho Mail, Google Workspace, and Microsoft 365 via OAuth. IMAP support is planned for a future release." }),
  },
  {
    q: translate({ id: "pricing.faq4.q", message: "What AI services does it use?" }),
    a: translate({ id: "pricing.faq4.a", message: "Campbooks uses Claude (Anthropic) for document analysis, email classification, and chat. You can also configure OpenAI-compatible providers like DeepSeek." }),
  },
];

export default function Pricing() {
  return (
    <Layout
      title={translate({ id: "pricing.page.title", message: "Pricing" })}
      description={translate({ id: "pricing.page.description", message: "Campbooks is open source and free to self-host." })}
    >
      <section style={{ padding: "5rem 0 3rem" }}>
        <div className="container">
          <div style={{ textAlign: "center", marginBottom: "3rem" }}>
            <Heading as="h1">
              <Translate id="pricing.hero.title">Simple pricing</Translate>
            </Heading>
            <p style={{ color: "var(--ifm-color-emphasis-700)", marginTop: "0.75rem" }}>
              <Translate id="pricing.hero.subtitle">
                Self-host for free, use our cloud instance, or let us manage it for you.
              </Translate>
            </p>
          </div>
          <div className="row" style={{ justifyContent: "center" }}>
            {tiers.map((tier) => (
              <div key={tier.name} className="col col--4" style={{ marginBottom: "1.5rem" }}>
                <div className={tier.highlighted ? "pricing-card pricing-card-highlighted" : "pricing-card"}>
                  <Heading as="h3" style={{ fontSize: "0.875rem" }}>{tier.name}</Heading>
                  {tier.badge && (
                    <span style={{ display: "inline-block", marginTop: "0.5rem", padding: "0.125rem 0.5rem", fontSize: "0.6875rem", fontWeight: 600, textTransform: "uppercase", letterSpacing: "0.05em", borderRadius: "0.375rem", background: "var(--ifm-color-primary-lightest)", color: "var(--ifm-color-primary-darkest)" }}>{tier.badge}</span>
                  )}
                  <p style={{ fontSize: "2rem", fontWeight: 700, margin: "0.75rem 0 0" }}>{tier.price}</p>
                  <p style={{ fontSize: "0.875rem", color: "var(--ifm-color-emphasis-700)" }}>{tier.description}</p>
                  <ul style={{ paddingLeft: "1.25rem", marginTop: "1.5rem", fontSize: "0.875rem" }}>
                    {tier.features.map((f) => (
                      <li key={f} style={{ marginBottom: "0.5rem", color: "var(--ifm-color-emphasis-700)" }}>{f}</li>
                    ))}
                  </ul>
                  <Link
                    className={tier.highlighted ? "button button--primary button--block" : "button button--outline button--block"}
                    to={tier.cta.href}
                    style={{ marginTop: "1.5rem" }}
                  >
                    {tier.cta.text}
                  </Link>
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>
      <section style={{ padding: "2rem 0 5rem" }}>
        <div className="container" style={{ maxWidth: "48rem" }}>
          <Heading as="h2" style={{ textAlign: "center", marginBottom: "2rem" }}>
            <Translate id="pricing.faq.title">Frequently asked questions</Translate>
          </Heading>
          {faqs.map((faq) => (
            <div key={faq.q} className="card" style={{ padding: "1.25rem", marginBottom: "1rem" }}>
              <Heading as="h3" style={{ fontSize: "0.875rem" }}>{faq.q}</Heading>
              <p style={{ fontSize: "0.875rem", color: "var(--ifm-color-emphasis-700)", marginTop: "0.5rem" }}>{faq.a}</p>
            </div>
          ))}
        </div>
      </section>
    </Layout>
  );
}
