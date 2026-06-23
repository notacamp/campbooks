require "rails_helper"

RSpec.describe EmailMessage, type: :model do
  describe "associations" do
    it { is_expected.to belong_to(:email_account) }
  end

  describe "validations" do
    subject { build(:email_message, provider_message_id: "msg_abc123") }

    it { is_expected.to validate_presence_of(:provider_message_id) }
    it { is_expected.to validate_uniqueness_of(:provider_message_id).scoped_to(:email_account_id) }
  end

  describe "enums" do
    it {
      is_expected.to define_enum_for(:status)
        .with_values(fetched: 0, processing: 1, processed: 2, ignored: 3, failed: 4)
    }
  end

  describe ".accessible_to" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }
    let(:readable_account) { create(:email_account, workspace: workspace) }
    let(:other_account) { create(:email_account, workspace: workspace) }
    let!(:readable_message) { create(:email_message, email_account: readable_account) }
    let!(:hidden_message) { create(:email_message, email_account: other_account) }

    before { create(:email_account_user, :viewer, user: user, email_account: readable_account) }

    it "returns only messages on accounts the user can read" do
      expect(EmailMessage.accessible_to(user)).to contain_exactly(readable_message)
    end

    it "excludes an account shared without read access" do
      create(:email_account_user, user: user, email_account: other_account, can_read: false)
      expect(EmailMessage.accessible_to(user)).to contain_exactly(readable_message)
    end

    it "fails closed for a nil user" do
      expect(EmailMessage.accessible_to(nil)).to be_empty
    end
  end

  describe "#searchable_filter_data" do
    subject(:data) do
      build(:email_message,
        from_address: "Jane Doe <jane@Acme.com>", read: false,
        category: "important", provider_folder_id: "F1", has_attachment: true).searchable_filter_data
    end

    it "carries the inbox-search filter keys" do
      expect(data).to include(
        provider_folder_id: "F1",
        read: false,
        category: "important",
        has_attachments: true,
        sender_domain: "acme.com"
      )
    end

    it "lowercases the sender domain and handles the bare email form" do
      bare = build(:email_message, from_address: "billing@Stripe.com").searchable_filter_data
      expect(bare[:sender_domain]).to eq("stripe.com")
    end
  end
end
