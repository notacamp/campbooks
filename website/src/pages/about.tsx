import Link from "@docusaurus/Link";
import Translate, {translate} from '@docusaurus/Translate';
import Layout from "@theme/Layout";
import Heading from "@theme/Heading";

const stack = [
  "Ruby on Rails 8", "PostgreSQL", "Tailwind CSS", "Hotwire", "Solid Queue", "Phlex",
  "Claude AI", "OpenSearch", "Docker",
];

export default function About() {
  return (
    <Layout
      title={translate({id: 'about.page.title', message: 'About Campbooks'})}
      description={translate({id: 'about.page.description', message: 'Campbooks is a source-available, AI-native email client for professionals and small businesses.'})}
    >
      <section style={{ padding: "5rem 0 4rem" }}>
        <div className="container" style={{ maxWidth: "48rem" }}>
          <Heading as="h1"><Translate id="about.hero.title">About Campbooks</Translate></Heading>
          <div style={{ marginTop: "1.5rem", color: "var(--ifm-color-emphasis-700)", lineHeight: 1.65 }}>
            <p>
              <Translate id="about.hero.p1">
                Campbooks is a source-available, AI-native email client for professionals and small businesses who live in email and paperwork. It reads your inbox and attachments, uses AI to file and surface what matters, and gives you a clear review-and-approval flow — reimagined so it feels nothing like the email you're used to.
              </Translate>
            </p>
            <p>
              <Translate id="about.hero.p2">
                The interface is warm, clear, and human — like a friendly assistant who happens to be very competent. No toolbar overload, no enterprise jargon, no sterile fintech aesthetics. Just a fast, focused place to handle your paperwork and get back to your actual work.
              </Translate>
            </p>
            <p>
              <Translate id="about.hero.p3.prefix">Campbooks is built and maintained by</Translate>{" "}
              <a href="https://not-a-camp.com">Not A Camp</a>
              {", "}<Translate id="about.hero.p3.suffix">a studio that builds tools for small businesses.</Translate>
            </p>
          </div>

          <Heading as="h2" style={{ marginTop: "3rem" }}><Translate id="about.technology.title">Technology</Translate></Heading>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem", marginTop: "1rem" }}>
            {stack.map((tech) => (
              <span key={tech} className="badge badge--primary" style={{ fontSize: "0.75rem" }}>{tech}</span>
            ))}
          </div>

          <Heading as="h2" style={{ marginTop: "3rem" }}><Translate id="about.opensource.title">Source-available</Translate></Heading>
          <div style={{ color: "var(--ifm-color-emphasis-700)", lineHeight: 1.65, marginTop: "1rem" }}>
            <p>
              <Translate id="about.opensource.p1">
                Campbooks is released under the Sustainable Use License — a fair-code, source-available license. You can view the source, report issues, and contribute on GitHub.
              </Translate>
            </p>
            <p>
              <Link href="https://github.com/notacamp/campbooks">
                github.com/notacamp/campbooks
              </Link>
            </p>
          </div>
        </div>
      </section>
    </Layout>
  );
}
