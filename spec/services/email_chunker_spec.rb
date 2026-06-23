require "rails_helper"

RSpec.describe EmailChunker do
  def chunks_for(body, subject: "Quarterly report")
    email = build_stubbed(:email_message, subject: subject, body: body)
    described_class.new(email).chunk
  end

  def est_tokens(str)
    (str.length / 3.5).ceil
  end

  it "bounds a giant break-less HTML body to chunks within the embedding budget" do
    # The case that 400'd OpenAI: a raw-HTML body with no paragraph breaks for the
    # paragraph splitter to divide, producing one 27k-token chunk.
    html = "<html><body><div>#{'finance ' * 40_000}</div></body></html>"
    chunks = chunks_for(html)

    expect(chunks).not_to be_empty
    expect(chunks.map { |c| est_tokens(c[:content]) }.max).to be <= described_class::MAX_CHUNK_TOKENS
  end

  it "strips HTML so chunks carry text, not markup" do
    chunks = chunks_for("<p>Hello <b>world</b> &amp; goodbye</p>")
    body = chunks.find { |c| c[:chunk_type] == "email_message" }[:content]

    expect(body).to include("Hello world & goodbye")
    expect(body).not_to match(/<[a-z]/i)
  end

  it "keeps a short plain-text email as one bounded chunk" do
    chunks = chunks_for("Thanks — see you tomorrow at 10.")

    expect(chunks.map { |c| c[:content] }.join(" ")).to include("see you tomorrow")
    expect(chunks.map { |c| est_tokens(c[:content]) }.max).to be <= described_class::MAX_CHUNK_TOKENS
  end
end
