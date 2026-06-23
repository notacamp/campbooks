import Layout from "@theme/Layout";
import Heading from "@theme/Heading";
import Translate, { translate } from "@docusaurus/Translate";

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

export default function Support() {
  return (
    <Layout
      title={translate({ id: "support.page.title", message: "Support" })}
      description={translate({
        id: "support.page.description",
        message: "Get help with Campbooks. Contact us by email, browse common questions, or report a problem.",
      })}
    >
      <section style={{ padding: "5rem 0 4rem" }}>
        <div className="container" style={{ maxWidth: "48rem" }}>
          <Heading as="h1">
            <Translate id="support.hero.title">Support</Translate>
          </Heading>
          <div style={prose}>
            <p>
              <Translate id="support.hero.intro">
                Need help with Campbooks? You&rsquo;re in the right place. Browse the common questions
                below, or reach out to us directly &mdash; we&rsquo;re a small team and we read every message.
              </Translate>
            </p>
          </div>

          <Section title={translate({ id: "support.section.contact.title", message: "Get in touch" })}>
            <p>
              <Translate id="support.section.contact.intro">
                The best way to reach us is by email:
              </Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>.{" "}
              <Translate id="support.section.contact.response_time">
                We aim to reply within 2 business days.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "support.section.faq.title", message: "Common questions" })}>
            <Heading as="h3" style={{ marginTop: "1.75rem", fontSize: "1rem" }}>
              <Translate id="support.section.faq.q1.question">
                How do I get access?
              </Translate>
            </Heading>
            <p>
              <Translate id="support.section.faq.q1.answer">
                Campbooks cloud is currently in closed beta. Email
              </Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>{" "}
              <Translate id="support.section.faq.q1.answer_after">
                to request an access code.
              </Translate>
            </p>

            <Heading as="h3" style={{ marginTop: "1.75rem", fontSize: "1rem" }}>
              <Translate id="support.section.faq.q2.question">
                How do I connect my mailbox?
              </Translate>
            </Heading>
            <p>
              <Translate id="support.section.faq.q2.answer">
                In the app, go to your account settings and connect Zoho, Google, or Microsoft over
                OAuth. The connection grants Campbooks read access to your inbox so it can ingest and
                organise your emails and attachments.
              </Translate>
            </p>

            <Heading as="h3" style={{ marginTop: "1.75rem", fontSize: "1rem" }}>
              <Translate id="support.section.faq.q3.question">
                Where is my data stored?
              </Translate>
            </Heading>
            <p>
              <Translate id="support.section.faq.q3.answer_before">
                All customer data is hosted in the European Union (Hetzner, Germany). See our
              </Translate>{" "}
              <a href="/privacy">
                <Translate id="support.section.faq.q3.privacy_link">Privacy Policy</Translate>
              </a>{" "}
              <Translate id="support.section.faq.q3.answer_after">
                for full details.
              </Translate>
            </p>

            <Heading as="h3" style={{ marginTop: "1.75rem", fontSize: "1rem" }}>
              <Translate id="support.section.faq.q4.question">
                Can I self-host Campbooks?
              </Translate>
            </Heading>
            <p>
              <Translate id="support.section.faq.q4.answer_before">
                Yes &mdash; Campbooks is source-available. You can find installation instructions on the
              </Translate>{" "}
              <a href="/self-hosting">
                <Translate id="support.section.faq.q4.selfhosting_link">self-hosting page</Translate>
              </a>{" "}
              <Translate id="support.section.faq.q4.answer_middle">
                and the source code on
              </Translate>{" "}
              <a href="https://github.com/notacamp/campbooks">GitHub</a>.
            </p>

            <Heading as="h3" style={{ marginTop: "1.75rem", fontSize: "1rem" }}>
              <Translate id="support.section.faq.q5.question">
                How do I delete my account?
              </Translate>
            </Heading>
            <p>
              <Translate id="support.section.faq.q5.answer_before">
                You can delete your account and all associated data from within the app. See our
              </Translate>{" "}
              <a href="/data-deletion">
                <Translate id="support.section.faq.q5.data_deletion_link">data deletion page</Translate>
              </a>{" "}
              <Translate id="support.section.faq.q5.answer_after">
                for step-by-step instructions.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "support.section.bug.title", message: "Report a problem" })}>
            <p>
              <Translate id="support.section.bug.intro">
                Found something broken? The quickest way to report it is through the in-app bug reporter
                (the folder icon in the bottom-right corner of the screen). You can also open an issue
                directly on GitHub:
              </Translate>{" "}
              <a href="https://github.com/notacamp/campbooks/issues">
                github.com/notacamp/campbooks/issues
              </a>.
            </p>
          </Section>

          <Section title={translate({ id: "support.section.company.title", message: "Company" })}>
            <p>
              <Translate id="support.section.company.body">
                Campbooks is operated by Not A Camp LDA, Rua de S. Pedro 24, Branca CCH,
                2100-607 Santarém, Portugal.
              </Translate>
            </p>
          </Section>
        </div>
      </section>
    </Layout>
  );
}
