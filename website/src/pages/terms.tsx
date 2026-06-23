import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";
import Heading from "@theme/Heading";
import Translate, {translate} from '@docusaurus/Translate';

// NOTE: Plain-language terms of service, not legal advice — have it reviewed before
// relying on it. Governing law defaults to Portugal (where Not A Camp LDA is registered);
// change it if you intend a different jurisdiction.
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

export default function Terms() {
  return (
    <Layout
      title={translate({id: 'terms.page.title', message: 'Terms of Service'})}
      description={translate({id: 'terms.page.description', message: 'The terms that govern your use of the Campbooks hosted service.'})}
    >
      <section style={{ padding: "5rem 0 4rem" }}>
        <div className="container" style={{ maxWidth: "48rem" }}>
          <Heading as="h1"><Translate id="terms.hero.title">Terms of Service</Translate></Heading>
          <p style={{ color: "var(--ifm-color-emphasis-600)", marginTop: "0.5rem" }}>
            <Translate id="terms.hero.last_updated">Last updated:</Translate>{" "}{LAST_UPDATED}
          </p>

          <div style={prose}>
            <p>
              <Translate id="terms.intro.p1_before_company">These Terms of Service (&ldquo;Terms&rdquo;) are an agreement between you and</Translate>{" "}
              <strong>Not A Camp LDA</strong>{" "}
              <Translate id="terms.intro.p1_after_company">(&ldquo;Not A Camp&rdquo;, &ldquo;we&rdquo;,
              &ldquo;us&rdquo;), the company that operates Campbooks. They govern your use of the
              hosted Campbooks service at</Translate>{" "}
              <a href="https://app.campbooks.not-a-camp.com">app.campbooks.not-a-camp.com</a>{" "}
              <Translate id="terms.intro.p1_after_link">(the
              &ldquo;Service&rdquo;). By creating an account or using the Service, you agree to these
              Terms. If you don&rsquo;t agree, please don&rsquo;t use the Service.</Translate>
            </p>
          </div>

          <Section title={translate({id: 'terms.section.the_service.title', message: 'The Service'})}>
            <p>
              <Translate id="terms.section.the_service.p1">
                Campbooks is an email and document workspace that ingests your emails and attachments,
                uses AI to help classify and surface them, and provides review and workflow tools. We
                may add, change, or remove features over time.
              </Translate>
            </p>
            <p>
              <Translate id="terms.section.the_service.p2_before_license">Campbooks&rsquo; source code is open source under the</Translate>{" "}
              <strong>MIT License</strong>
              <Translate id="terms.section.the_service.p2_after_license">. If
              you run your own self-hosted copy, that use is governed by the MIT License, not by these
              Terms — these Terms apply only to the hosted Service we operate.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.closed_beta.title', message: 'Closed beta'})}>
            <p>
              <Translate id="terms.section.closed_beta.p1_before_strong1">The hosted Service is currently in a</Translate>{" "}
              <strong><Translate id="terms.section.closed_beta.strong1">closed, invitation-only beta</Translate></strong>
              <Translate id="terms.section.closed_beta.p1_after_strong1">. Access requires an invitation, and we may approve, decline, or revoke beta access at our
              discretion. During the beta the Service is provided</Translate>{" "}
              <strong><Translate id="terms.section.closed_beta.strong2">free of charge</Translate></strong>
              <Translate id="terms.section.closed_beta.p1_after_strong2">, may
              change or be temporarily unavailable, and may contain bugs. We can&rsquo;t guarantee that
              data will be preserved during the beta, so please keep your own copies of anything
              important and avoid storing highly confidential information.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.your_account.title', message: 'Your account'})}>
            <p>
              <Translate id="terms.section.your_account.p1_before_link">You must provide accurate information when you sign up and keep your login credentials
              secure. You are responsible for all activity under your account. You must be at least 16
              years old, or the age of digital consent in your country, to use the Service. Tell us
              promptly at</Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>{" "}
              <Translate id="terms.section.your_account.p1_after_link">if you suspect
              unauthorized use of your account.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.acceptable_use.title', message: 'Acceptable use'})}>
            <p><Translate id="terms.section.acceptable_use.intro">When using the Service, you agree not to:</Translate></p>
            <ul>
              <li><Translate id="terms.section.acceptable_use.li1">break the law or infringe anyone&rsquo;s rights, including privacy and IP rights;</Translate></li>
              <li><Translate id="terms.section.acceptable_use.li2">upload malware, or send spam or unlawful, harmful, or abusive content;</Translate></li>
              <li>
                <Translate id="terms.section.acceptable_use.li3">attempt to disrupt, overload, gain unauthorized access to, or probe the Service or its
                infrastructure;</Translate>
              </li>
              <li>
                <Translate id="terms.section.acceptable_use.li4">reverse engineer, resell, or commercially exploit the hosted Service (the open-source
                code is separately available under the MIT License); or</Translate>
              </li>
              <li><Translate id="terms.section.acceptable_use.li5">use the Service to process data you don&rsquo;t have the right to process.</Translate></li>
            </ul>
          </Section>

          <Section title={translate({id: 'terms.section.your_content.title', message: 'Your content'})}>
            <p>
              <Translate id="terms.section.your_content.p1_before_link">You keep all rights to the emails, documents, and other content you bring into Campbooks.
              You grant us only the limited license needed to host, process, and display that content
              so we can provide the Service to you. You are responsible for having the right to use the
              content you connect. How we handle personal data is described in our</Translate>{" "}
              <Link to="/privacy"><Translate id="terms.section.your_content.privacy_link">Privacy Policy</Translate></Link>
              <Translate id="terms.section.your_content.p1_after_link">.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.third_party.title', message: 'Third-party services you connect'})}>
            <p>
              <Translate id="terms.section.third_party.p1_before_ai">Campbooks lets you connect outside services that you choose and control — your</Translate>{" "}
              <strong><Translate id="terms.section.third_party.strong_ai">AI provider</Translate></strong>{" "}
              <Translate id="terms.section.third_party.p1_after_ai_before_mailbox">(you supply your own API key) and your</Translate>{" "}
              <strong><Translate id="terms.section.third_party.strong_mailbox">mailbox</Translate></strong>{" "}
              <Translate id="terms.section.third_party.p1_after_mailbox">(Zoho, Google, or Microsoft, via OAuth). Your use of those
              services is governed by their own terms, and you are responsible for your accounts with
              them. We are not responsible for third-party services, and connecting them is at your own
              discretion.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.fees.title', message: 'Fees and future subscriptions'})}>
            <p>
              <Translate id="terms.section.fees.p1_before_subscription">The hosted Service is free during the beta. We plan to move to a</Translate>{" "}
              <strong><Translate id="terms.section.fees.strong_subscription">subscription model</Translate></strong>
              <Translate id="terms.section.fees.p1_after_subscription_before_free_tier">, under which we expect to keep a</Translate>{" "}
              <strong><Translate id="terms.section.fees.strong_free_tier">free
              tier</Translate></strong>{" "}
              <Translate id="terms.section.fees.p1_after_free_tier">limited to a single connected email account and a reduced set of features,
              with paid plans unlocking additional email accounts and capabilities. If and when we
              introduce paid plans, we will show you the pricing and any additional terms before you
              are charged — you will never be billed retroactively for your free beta use, and
              you&rsquo;ll be able to decline. The open-source version will always remain free to
              self-host under the MIT License, with no such limits.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.ip.title', message: 'Intellectual property'})}>
            <p>
              <Translate id="terms.section.ip.p1">
                The Campbooks software is open source under the MIT License. The Campbooks and Not A Camp
                names, logos, and brand are owned by Not A Camp LDA, and these Terms don&rsquo;t grant you
                any right to use our trademarks without permission.
              </Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.disclaimer.title', message: 'Disclaimer of warranties'})}>
            <p>
              <Translate id="terms.section.disclaimer.p1_before_as_is">The Service is provided</Translate>{" "}
              <strong><Translate id="terms.section.disclaimer.strong_as_is">&ldquo;as is&rdquo;</Translate></strong>{" "}
              <Translate id="terms.section.disclaimer.p1_and">and</Translate>{" "}
              <strong><Translate id="terms.section.disclaimer.strong_as_available">&ldquo;as available&rdquo;</Translate></strong>
              <Translate id="terms.section.disclaimer.p1_after_strong">, without warranties of any kind, whether
              express or implied, including fitness for a particular purpose and uninterrupted or
              error-free operation. Because the Service is free and reset periodically, we do not
              guarantee that your data will be preserved or available.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.liability.title', message: 'Limitation of liability'})}>
            <p>
              <Translate id="terms.section.liability.p1">
                To the maximum extent permitted by law, Not A Camp will not be liable for any indirect,
                incidental, or consequential damages, or for any loss of data, profits, or business,
                arising from your use of the Service. Nothing in these Terms limits liability that
                cannot be limited under applicable law.
              </Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.indemnification.title', message: 'Indemnification'})}>
            <p>
              <Translate id="terms.section.indemnification.p1">
                You agree to indemnify and hold Not A Camp harmless from claims and costs arising out of
                your misuse of the Service, your content, or your violation of these Terms or of
                applicable law.
              </Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.termination.title', message: 'Termination'})}>
            <p>
              <Translate id="terms.section.termination.p1">
                You can stop using the Service at any time. We may suspend or terminate your access if
                you breach these Terms or use the Service in a way that could harm us, other users, or
                third parties. Provisions that by their nature should survive termination (such as
                intellectual property, disclaimers, and limitation of liability) will continue to apply.
              </Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.changes.title', message: 'Changes to these Terms'})}>
            <p>
              <Translate id="terms.section.changes.p1">
                We may update these Terms from time to time. When we make material changes, we&rsquo;ll
                update the date at the top of this page and, where appropriate, notify you. Continuing to
                use the Service after changes take effect means you accept the updated Terms.
              </Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.governing_law.title', message: 'Governing law'})}>
            <p>
              <Translate id="terms.section.governing_law.p1_before_country">These Terms are governed by the laws of</Translate>{" "}
              <strong><Translate id="terms.section.governing_law.strong_country">Portugal</Translate></strong>
              <Translate id="terms.section.governing_law.p1_after_country">, and the courts of
              Portugal have jurisdiction over any dispute, without prejudice to any mandatory
              consumer-protection rights you have in your country of residence in the EU.</Translate>
            </p>
          </Section>

          <Section title={translate({id: 'terms.section.contact.title', message: 'Contact'})}>
            <p>
              <Translate id="terms.section.contact.p1_before_link">Questions about these Terms? Email</Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>{" "}
              <Translate id="terms.section.contact.p1_after_link">and we&rsquo;ll get back
              to you.</Translate>
            </p>
          </Section>
        </div>
      </section>
    </Layout>
  );
}
