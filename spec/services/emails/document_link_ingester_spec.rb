require "rails_helper"

RSpec.describe Emails::DocumentLinkIngester, type: :service do
  let(:workspace) { create(:workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:scan_log) { create(:email_scan_log, email_account: account) }

  # Default body so the email factory works even when a nested context hasn't
  # overridden it yet (tests that set a custom body do so via email.update!).
  let(:body) { '<a href="https://example.com/default.pdf">default</a>' }

  let(:email) do
    create(:email_message,
      email_account: account,
      email_scan_log: scan_log,
      body: body)
  end

  let(:connection) { instance_double(Faraday::Connection) }

  def pdf_response(body: "fake-pdf-content")
    instance_double(Faraday::Response,
      status: 200,
      body: body,
      headers: { "content-type" => "application/pdf" },
      success?: true)
  end

  def plain_response(body: "hello-world")
    instance_double(Faraday::Response,
      status: 200,
      body: body,
      headers: { "content-type" => "text/plain; charset=utf-8" },
      success?: true)
  end

  def not_found_response
    instance_double(Faraday::Response,
      status: 404,
      body: "not found",
      headers: {},
      success?: false)
  end

  before do
    # Allow HttpClient to reach out via the injected connection (UrlGuard will pass
    # public URLs in the test env: localhost is allowed only in dev).
    allow(Workflows::HttpClient).to receive(:call).and_call_original
  end

  describe "#call" do
    context "with a document link in an HTML body" do
      let(:body) { <<~HTML }
        <html><body>
          <p>Please find the invoice <a href="https://example.com/invoice.pdf">here</a>.</p>
        </body></html>
      HTML

      before do
        allow(Workflows::HttpClient).to receive(:call)
          .with(hash_including(url: "https://example.com/invoice.pdf"))
          .and_return(
            ok: true,
            status: 200,
            body: "pdf-bytes",
            headers: { "content-type" => "application/pdf" }
          )
      end

      it "creates a Document from the link" do
        expect {
          described_class.new(email).call
        }.to change(Document, :count).by(1)

        doc = Document.last
        expect(doc.source).to eq("email")
        expect(doc.email_account).to eq(account)
        expect(doc.email_message_id).to eq(email.provider_message_id)
        expect(doc.workspace).to eq(workspace)
        expect(doc.ai_status).to eq("pending")
        expect(doc.review_status).to eq("pending")
        expect(doc.document_type).to eq("other")
        expect(doc.original_file).to be_attached
        expect(doc.original_file.filename.to_s).to eq("invoice.pdf")
      end

      it "enqueues DocumentProcessJob" do
        expect {
          described_class.new(email).call
        }.to have_enqueued_job(DocumentProcessJob)
      end

      it "links the document to the email via document_email_messages" do
        described_class.new(email).call
        doc = Document.last
        expect(doc.document_email_messages.pluck(:email_message_id)).to include(email.id)
      end
    end

    context "with a plain-text body containing a bare link" do
      let(:body) { "Download your statement here: https://example.com/statement.csv" }

      before do
        allow(Workflows::HttpClient).to receive(:call)
          .with(hash_including(url: "https://example.com/statement.csv"))
          .and_return(
            ok: true,
            status: 200,
            body: "csv-bytes",
            headers: { "content-type" => "text/csv; charset=utf-8" }
          )
      end

      it "creates a Document from the bare link" do
        expect {
          described_class.new(email).call
        }.to change(Document, :count).by(1)

        expect(Document.last.original_file.filename.to_s).to eq("statement.csv")
      end
    end

    it "skips non-document content types (e.g. HTML pages)" do
      email.update!(body: '<a href="https://example.com/page">click</a>')

      allow(Workflows::HttpClient).to receive(:call)
        .with(hash_including(url: "https://example.com/page"))
        .and_return(
          ok: true,
          status: 200,
          body: "<html>web page</html>",
          headers: { "content-type" => "text/html; charset=utf-8" }
        )

      expect {
        described_class.new(email).call
      }.not_to change(Document, :count)
    end

    it "skips cloud-share domains (Google Drive)" do
      email.update!(body: '<a href="https://drive.google.com/file/d/abc123/view">file</a>')

      allow(Workflows::HttpClient).to receive(:call).and_call_original

      expect {
        described_class.new(email).call
      }.not_to change(Document, :count)
    end

    it "skips internal/private IPs (UrlGuard)" do
      email.update!(body: '<a href="http://10.0.0.1/report.pdf">report</a>')

      # UrlGuard blocks before HttpClient is called
      allow(Workflows::HttpClient).to receive(:call).and_call_original

      expect {
        described_class.new(email).call
      }.not_to change(Document, :count)
    end

    it "skips oversized downloads" do
      email.update!(body: '<a href="https://example.com/huge.pdf">huge</a>')

      allow(Workflows::HttpClient).to receive(:call)
        .with(hash_including(url: "https://example.com/huge.pdf"))
        .and_return(
          ok: true,
          status: 200,
          body: "x" * (Emails::DocumentLinkIngester::MAX_FILE_BYTES + 1),
          headers: { "content-type" => "application/pdf" }
        )

      expect {
        described_class.new(email).call
      }.not_to change(Document, :count)
    end

    it "deduplicates against existing documents with the same content hash" do
      email.update!(body: '<a href="https://example.com/doc.pdf">doc</a>')

      allow(Workflows::HttpClient).to receive(:call)
        .with(hash_including(url: "https://example.com/doc.pdf"))
        .and_return(
          ok: true,
          status: 200,
          body: "pdf-bytes",
          headers: { "content-type" => "application/pdf" }
        )

      described_class.new(email).call
      expect(Document.count).to eq(1)

      # Re-calling is idempotent
      expect {
        described_class.new(email).call
      }.not_to change(Document, :count)
    end

    it "caps per-email links at MAX_LINKS_PER_EMAIL" do
      links = (1..15).map { |i| "<a href=\"https://example.com/file#{i}.pdf\">file #{i}</a>" }.join("\n")
      email.update!(body: "<html><body>#{links}</body></html>")

      call_count = 0
      allow(Workflows::HttpClient).to receive(:call) do |opts|
        call_count += 1
        {
          ok: true,
          status: 200,
          body: opts[:url],
          headers: { "content-type" => "application/pdf" }
        }
      end

      expect {
        described_class.new(email).call
      }.to change(Document, :count).by(described_class::MAX_LINKS_PER_EMAIL)
    end

    it "isolates per-link failures (one bad link does not affect the rest)" do
      body = <<~HTML
        <a href="https://example.com/good.pdf">good</a>
        <a href="https://example.com/also-good.pdf">also good</a>
      HTML
      email.update!(body: body)

      call_count = 0
      allow(Workflows::HttpClient).to receive(:call) do |opts|
        call_count += 1
        if call_count == 1
          raise Faraday::TimeoutError.new("timed out")
        end
        { ok: true, status: 200, body: opts[:url], headers: { "content-type" => "application/pdf" } }
      end

      expect {
        described_class.new(email).call
      }.to change(Document, :count).by(1)
    end

    it "returns 0 for empty body" do
      email.update!(body: nil)
      expect(described_class.new(email).call).to eq(0)
    end
  end
end
