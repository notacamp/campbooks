require "rails_helper"
require "zip"

RSpec.describe AccountExportJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def entries(account_export)
    data = account_export.archive.download
    Zip::File.open_buffer(StringIO.new(data)).map(&:name)
  end

  it "builds a zip with the account JSON, email bodies, and attachments, and marks it generated" do
    email = create(:email_message, email_account: account, subject: "Hello", body: "<p>secret body</p>")
    email.files.attach(io: StringIO.new("PDF"), filename: "invoice.pdf", content_type: "application/pdf")

    account_export = user.account_exports.create!(status: :pending)
    described_class.new.perform(account_export.id)
    account_export.reload

    expect(account_export).to be_generated
    expect(account_export.archive).to be_attached
    names = entries(account_export)
    expect(names).to include("account.json")
    expect(names).to include(a_string_matching(%r{emails/.+/#{email.id}\.json}))
    expect(names).to include(a_string_matching(%r{attachments/.+/#{email.id}/invoice\.pdf}))
  end

  it "scopes the export to the requesting user (another user's email is absent)" do
    mine = create(:email_message, email_account: account, subject: "Mine")

    other_user = create(:user, workspace: workspace)
    other_account = create(:email_account, workspace: workspace)
    create(:email_account_user, user: other_user, email_account: other_account, can_read: true)
    theirs = create(:email_message, email_account: other_account, subject: "Theirs")

    account_export = user.account_exports.create!(status: :pending)
    described_class.new.perform(account_export.id)

    names = entries(account_export.reload)
    expect(names).to include(a_string_matching(/#{mine.id}\.json/))
    expect(names).not_to include(a_string_matching(/#{theirs.id}\.json/))
  end

  it "skips a blob whose underlying file is missing without failing the export" do
    email = create(:email_message, email_account: account, subject: "Has attachment")
    email.files.attach(io: StringIO.new("PDF"), filename: "gone.pdf", content_type: "application/pdf")
    # Only the attachment's file is "missing" — the archive zip still downloads.
    allow_any_instance_of(ActiveStorage::Blob).to receive(:download).and_wrap_original do |original, *args|
      original.receiver.filename.to_s == "gone.pdf" ? raise(ActiveStorage::FileNotFoundError) : original.call(*args)
    end

    account_export = user.account_exports.create!(status: :pending)
    expect { described_class.new.perform(account_export.id) }.not_to raise_error

    account_export.reload
    expect(account_export).to be_generated
    names = entries(account_export)
    expect(names).to include("account.json")
    expect(names).not_to include(a_string_matching(/gone\.pdf/))
  end

  it "marks the export failed and re-raises when the archive can't be built" do
    account_export = user.account_exports.create!(status: :pending)
    allow(Accounts::ArchiveGenerator).to receive(:new).and_raise(StandardError, "boom")

    expect { described_class.new.perform(account_export.id) }.to raise_error(StandardError)
    expect(account_export.reload).to be_failed
  end
end
