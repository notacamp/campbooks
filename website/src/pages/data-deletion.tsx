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

export default function DataDeletion() {
  return (
    <Layout
      title={translate({ id: "dataDeletion.page.title", message: "Deleting your account and data" })}
      description={translate({
        id: "dataDeletion.page.description",
        message: "How to delete your Campbooks account and all associated data, immediately and permanently.",
      })}
    >
      <section style={{ padding: "5rem 0 4rem" }}>
        <div className="container" style={{ maxWidth: "48rem" }}>
          <Heading as="h1">
            <Translate id="dataDeletion.hero.title">Deleting your account and data</Translate>
          </Heading>
          <div style={prose}>
            <p>
              <Translate id="dataDeletion.hero.intro">
                You can delete your Campbooks account and all of its data at any time. Deletion is
                self-serve and takes effect immediately. This page explains what gets deleted, what we
                may be required to keep, and how to request deletion if you can&rsquo;t access the app.
              </Translate>
            </p>
          </div>

          <Section title={translate({ id: "dataDeletion.section.in_app.title", message: "Delete from within the app" })}>
            <p>
              <Translate id="dataDeletion.section.in_app.intro">
                To delete your account and data from inside Campbooks:
              </Translate>
            </p>
            <ol>
              <li><Translate id="dataDeletion.section.in_app.step1">Open Campbooks and sign in.</Translate></li>
              <li><Translate id="dataDeletion.section.in_app.step2">Go to Settings &rarr; Account.</Translate></li>
              <li><Translate id="dataDeletion.section.in_app.step3">Scroll to the bottom and click Delete account.</Translate></li>
              <li><Translate id="dataDeletion.section.in_app.step4">Confirm the deletion when prompted.</Translate></li>
            </ol>
            <p>
              <Translate id="dataDeletion.section.in_app.effect">
                Deletion is self-serve and takes effect immediately.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "dataDeletion.section.what_deleted.title", message: "What gets deleted" })}>
            <p>
              <Translate id="dataDeletion.section.what_deleted.intro">
                When you delete your account, we permanently remove:
              </Translate>
            </p>
            <ul>
              <li><Translate id="dataDeletion.section.what_deleted.item1">your account and profile;</Translate></li>
              <li><Translate id="dataDeletion.section.what_deleted.item2">workspace data you own;</Translate></li>
              <li><Translate id="dataDeletion.section.what_deleted.item3">emails and documents ingested into Campbooks;</Translate></li>
              <li><Translate id="dataDeletion.section.what_deleted.item4">connected mailbox OAuth tokens;</Translate></li>
              <li><Translate id="dataDeletion.section.what_deleted.item5">any AI provider API keys you added; and</Translate></li>
              <li><Translate id="dataDeletion.section.what_deleted.item6">your workflows and settings.</Translate></li>
            </ul>
          </Section>

          <Section title={translate({ id: "dataDeletion.section.what_retained.title", message: "What we may retain" })}>
            <p>
              <Translate id="dataDeletion.section.what_retained.body">
                We retain only the limited records we are legally required to keep &mdash; for example,
                for tax or accounting purposes &mdash; and only for the minimum period required by law.
                Residual copies that may exist in encrypted backups are purged within 30 days of
                account deletion.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "dataDeletion.section.by_email.title", message: "Request deletion by email" })}>
            <p>
              <Translate id="dataDeletion.section.by_email.intro">
                If you are unable to access the app, you can request deletion by email. Send a message
                to
              </Translate>{" "}
              <a href="mailto:inbox@not-a-camp.com">inbox@not-a-camp.com</a>{" "}
              <Translate id="dataDeletion.section.by_email.instructions">
                from the email address associated with your Campbooks account. We will process the
                deletion for you and confirm once it is complete.
              </Translate>
            </p>
          </Section>

          <Section title={translate({ id: "dataDeletion.section.timeline.title", message: "Timeline" })}>
            <p>
              <Translate id="dataDeletion.section.timeline.body">
                Data is removed from active systems immediately upon deletion. Residual copies in
                encrypted backups are purged within 30 days.
              </Translate>
            </p>
          </Section>
        </div>
      </section>
    </Layout>
  );
}
