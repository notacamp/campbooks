import Layout from "@theme/Layout";
import Heading from "@theme/Heading";
import Translate, { translate } from "@docusaurus/Translate";

// NOTE: Plain-language privacy policy, not legal advice — have it reviewed before
// relying on it.
const LAST_UPDATED = "June 20, 2026";

const prose: React.CSSProperties = {
  color: "var(--ifm-color-emphasis-700)",
  lineHeight: 1.65,
  marginTop: "1rem",
};

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <>
      <Heading as="h2" style={{ marginTop: "3rem" }}>{title}</Heading>
      <div style={prose}>{children}</div>
    </>
  );
}

export default function Privacy() {
  return (
    <Layout
      title={translate({ id: "privacy.page.title", message: "Privacy Policy" })}
      description={translate({
        id: "privacy.page.description",
        message: "How Campbooks collects, uses, and protects your data. EU-hosted, never sold.",
      })}
    >
      <section style={{ padding: "5rem 0 4rem" }}>
        <div className="container" style={{ maxWidth: "48rem" }}>
          <Heading as="h1">
            <Translate id="privacy.hero.title">Privacy Policy</Translate>
          </Heading>
          <p style={{ color: "var(--ifm-color-emphasis-600)", marginTop: "0.5rem" }}>
            <Translate id="privacy.hero.last_updated">Last updated:</Translate> {LAST_UPDATED}
          </p>

          <div style={prose}>
            <p>
              <Translate id="privacy.intro.body">
                Campbooks is an email and document workspace operated by Not A Camp
                (&ldquo;we&rdquo;, &ldquo;us&rdquo;). This policy explains what personal data we
                collect, why we collect it, where it lives, and the rights you have over it. We&rsquo;ve
                tried to keep it short and readable.
              </Translate>
            </p>
            <p style={{ fontWeight: 600, color: "var(--ifm-color-emphasis-800)" }}>
              <Translate id="privacy.intro.summary">
                In one line: your data is hosted in the European Union, we never sell it, and we never
                share it with third parties for their own purposes.
              </Translate>
            </p>
          </div>

          <Section title={translate({ id: "privacy.section.controller.title", message: "Who is responsible for your data" })}>
            <p>
              <Translate id="privacy.section.controller.body_before">
                The data controller for your personal data is
              </Translate>{" "}
              <strong>Not A Camp LDA</strong>,{" "}
              <Translate id="privacy.section.controller.body_after">
                registered at Rua de S. Pedro 24, Branca CCH, 2100-607 Santarém, Portugal. If you have
                any question about this policy or your data, contact us at
              </Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>.
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.collect.title", message: "What data we collect" })}>
            <ul>
              <li>
                <strong>
                  <Translate id="privacy.section.collect.item1.label">Account information</Translate>
                </strong>{" "}
                <Translate id="privacy.section.collect.item1.body">
                  — your name, email address, and password (stored
                  only as a salted hash), plus workspace settings you configure.
                </Translate>
              </li>
              <li>
                <strong>
                  <Translate id="privacy.section.collect.item2.label">Email and documents you connect</Translate>
                </strong>{" "}
                <Translate id="privacy.section.collect.item2.body">
                  — when you link a mailbox, we ingest
                  the emails and attachments you choose to bring into Campbooks so we can organize,
                  classify, and display them to you. This content belongs to you.
                </Translate>
              </li>
              <li>
                <strong>
                  <Translate id="privacy.section.collect.item3.label">Integration credentials</Translate>
                </strong>{" "}
                <Translate id="privacy.section.collect.item3.body">
                  — OAuth refresh tokens for connected mail
                  providers (Zoho, Google, Microsoft) and the API key for any AI provider you connect,
                  all stored encrypted at rest. We never see or store your provider password.
                </Translate>
              </li>
              <li>
                <strong>
                  <Translate id="privacy.section.collect.item4.label">Usage and technical data</Translate>
                </strong>{" "}
                <Translate id="privacy.section.collect.item4.body">
                  — basic logs (IP address, browser type,
                  timestamps) needed to operate the service, keep it secure, and debug problems.
                </Translate>
              </li>
            </ul>
          </Section>

          <Section title={translate({ id: "privacy.section.use.title", message: "How we use your data" })}>
            <p>
              <Translate id="privacy.section.use.intro">
                We use your data only to provide and improve Campbooks, specifically to:
              </Translate>
            </p>
            <ul>
              <li><Translate id="privacy.section.use.item1">operate your account and workspace;</Translate></li>
              <li><Translate id="privacy.section.use.item2">ingest, classify, and surface your emails and documents;</Translate></li>
              <li><Translate id="privacy.section.use.item3">run the AI analysis and workflows you ask for;</Translate></li>
              <li><Translate id="privacy.section.use.item4">keep the service secure and reliable; and</Translate></li>
              <li><Translate id="privacy.section.use.item5">communicate with you about your account and service updates.</Translate></li>
            </ul>
            <p>
              <Translate id="privacy.section.use.no_train_before">We do</Translate>{" "}
              <strong>not</strong>{" "}
              <Translate id="privacy.section.use.no_train_after">use your email or document content to train AI models, and we do</Translate>{" "}
              <strong>not</strong>{" "}
              <Translate id="privacy.section.use.no_ads">use it for advertising.</Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.gdpr.title", message: "Legal basis (GDPR)" })}>
            <p>
              <Translate id="privacy.section.gdpr.intro">
                If you are in the European Economic Area or the UK, we process your data on the basis of
              </Translate>
              <strong> <Translate id="privacy.section.gdpr.contract">contract</Translate></strong>{" "}
              <Translate id="privacy.section.gdpr.middle">(to deliver the service you signed up for),</Translate>{" "}
              <strong><Translate id="privacy.section.gdpr.legitimate_interests">legitimate interests</Translate></strong>{" "}
              <Translate id="privacy.section.gdpr.middle2">(to keep the service secure and working), and your</Translate>{" "}
              <strong><Translate id="privacy.section.gdpr.consent">consent</Translate></strong>{" "}
              <Translate id="privacy.section.gdpr.outro">
                where required (for example, when you connect a mailbox). You can
                withdraw consent at any time by disconnecting an integration or closing your account.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.storage.title", message: "Where your data is stored" })}>
            <p>
              <Translate id="privacy.section.storage.intro">All customer data is hosted in the</Translate>{" "}
              <strong><Translate id="privacy.section.storage.eu">European Union</Translate></strong>,{" "}
              <Translate id="privacy.section.storage.provider_before">on infrastructure provided by</Translate>{" "}
              <strong>Hetzner Online GmbH</strong>{" "}
              <Translate id="privacy.section.storage.provider_after">
                (Germany). We don&rsquo;t transfer your
                data outside the EU ourselves. Data only leaves the EU if
              </Translate>{" "}
              <em><Translate id="privacy.section.storage.you">you</Translate></em>{" "}
              <Translate id="privacy.section.storage.connect_before">
                choose to connect a service located elsewhere (for example, a US-based AI provider) — see
              </Translate>{" "}
              <em><Translate id="privacy.section.storage.connect_link">Services you connect</Translate></em>{" "}
              <Translate id="privacy.section.storage.connect_after">below.</Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.sharing.title", message: "Who we share data with" })}>
            <p>
              <strong>
                <Translate id="privacy.section.sharing.no_sell">
                  We do not sell your data, and we do not share it with anyone for their own
                  purposes.
                </Translate>
              </strong>{" "}
              <Translate id="privacy.section.sharing.sub_processor">
                To run the service, we rely on a single infrastructure sub-processor,
                who acts only on our instructions:
              </Translate>
            </p>
            <ul>
              <li>
                <strong>Hetzner Online GmbH</strong>{" "}
                <Translate id="privacy.section.sharing.hetzner">
                  (Germany, EU) — hosting and infrastructure. Your
                  data stays within the EU.
                </Translate>
              </li>
            </ul>
            <p>
              <Translate id="privacy.section.sharing.disclosure">
                We may also disclose data if required by law, to protect our legal rights, or as part of
                a business transfer (in which case this policy continues to apply).
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.connected.title", message: "Services you connect (AI and mailboxes)" })}>
            <p>
              <Translate id="privacy.section.connected.intro_before">
                Campbooks lets you connect outside services that
              </Translate>{" "}
              <em><Translate id="privacy.section.connected.you">you</Translate></em>{" "}
              <Translate id="privacy.section.connected.intro_after">
                choose and control. We only
                send your data to these when you&rsquo;ve connected them, and only to do what you asked:
              </Translate>
            </p>
            <ul>
              <li>
                <strong>
                  <Translate id="privacy.section.connected.ai.label">Your AI provider</Translate>
                </strong>{" "}
                <Translate id="privacy.section.connected.ai.body_before">
                  — AI features are optional and configured by you. You
                  choose the provider (such as OpenAI, Anthropic, DeepSeek, or Gemini) and supply your own
                  API key. When you use an AI feature, the relevant email or document content is sent to
                  the provider
                </Translate>{" "}
                <em><Translate id="privacy.section.connected.ai.you">you</Translate></em>{" "}
                <Translate id="privacy.section.connected.ai.body_after">
                  selected, under your own account and that provider&rsquo;s
                  terms. We don&rsquo;t pick the provider for you, and we never send your content to an AI
                  service you haven&rsquo;t set up.
                </Translate>
              </li>
              <li>
                <strong>
                  <Translate id="privacy.section.connected.mailbox.label">Your mailbox</Translate>
                </strong>{" "}
                <Translate id="privacy.section.connected.mailbox.body">
                  — when you connect Zoho, Google, or Microsoft over OAuth,
                  we access only the mailbox you linked, on your instruction, to bring its messages into
                  Campbooks.
                </Translate>
              </li>
            </ul>
            <p style={{ fontSize: "0.875rem", color: "var(--ifm-color-emphasis-600)" }}>
              <Translate id="privacy.section.connected.disclaimer">
                For the data you send them, these providers act as independent controllers under their
                own terms — we encourage you to review their privacy policies.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.retention.title", message: "How long we keep it" })}>
            <p>
              <Translate id="privacy.section.retention.body">
                We keep your data for as long as your account is active. When you delete content or close
                your account, we delete the associated personal data within a reasonable period, except
                where we must retain limited records to meet legal obligations.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.rights.title", message: "Your rights" })}>
            <p><Translate id="privacy.section.rights.intro">You have the right to:</Translate></p>
            <ul>
              <li><Translate id="privacy.section.rights.item1">access the personal data we hold about you;</Translate></li>
              <li><Translate id="privacy.section.rights.item2">correct inaccurate data;</Translate></li>
              <li><Translate id="privacy.section.rights.item3">delete your data (&ldquo;right to be forgotten&rdquo;);</Translate></li>
              <li><Translate id="privacy.section.rights.item4">export your data in a portable format;</Translate></li>
              <li><Translate id="privacy.section.rights.item5">object to or restrict certain processing; and</Translate></li>
              <li><Translate id="privacy.section.rights.item6">withdraw consent at any time.</Translate></li>
            </ul>
            <p>
              <Translate id="privacy.section.rights.contact_before">To exercise any of these, email</Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>.{" "}
              <Translate id="privacy.section.rights.contact_after">
                If you are in the EEA
                or UK, you also have the right to lodge a complaint with your local data protection
                supervisory authority.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.security.title", message: "Security" })}>
            <p>
              <Translate id="privacy.section.security.body">
                We protect your data with encryption in transit (HTTPS) and encryption at rest for
                sensitive fields such as integration tokens. Access to production systems is restricted
                and logged. No system is perfectly secure, but we work to keep your data safe and to
                notify you promptly if a breach affects you.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.cookies.title", message: "Cookies" })}>
            <p>
              <Translate id="privacy.section.cookies.body">
                The Campbooks app uses a single essential cookie to keep you signed in. We do not use
                advertising or third-party tracking cookies. This marketing site does not set tracking
                cookies.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.children.title", message: "Children" })}>
            <p>
              <Translate id="privacy.section.children.body">
                Campbooks is a business tool and is not directed at children under 16. We do not
                knowingly collect data from children.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.changes.title", message: "Changes to this policy" })}>
            <p>
              <Translate id="privacy.section.changes.body">
                We may update this policy from time to time. When we make material changes, we&rsquo;ll
                update the date at the top of this page and, where appropriate, notify you in the app or
                by email.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "privacy.section.contact.title", message: "Contact" })}>
            <p>
              <Translate id="privacy.section.contact.body_before">Questions about your privacy? Email</Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>{" "}
              <Translate id="privacy.section.contact.body_after">and we&rsquo;ll get back to you.</Translate>
            </p>
          </Section>
        </div>
      </section>
    </Layout>
  );
}
