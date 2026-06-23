require "rails_helper"

RSpec.describe User, type: :model do
  describe "associations" do
    it { is_expected.to have_many(:sessions).dependent(:destroy) }
    it { is_expected.to have_many(:reviewed_documents) }
  end

  describe "validations" do
    subject { build(:user) }

    it { is_expected.to validate_presence_of(:email_address) }
    it { is_expected.to validate_uniqueness_of(:email_address).case_insensitive }
    it { is_expected.to validate_presence_of(:name) }

    it "rejects passwords shorter than 8 characters" do
      user = build(:user, password: "short7!", password_confirmation: "short7!")
      expect(user).not_to be_valid
      expect(user.errors[:password]).to be_present
    end

    it "accepts passwords of 8 or more characters" do
      expect(build(:user, password: "longeno8", password_confirmation: "longeno8")).to be_valid
    end

    it "skips the length check when the password is unchanged" do
      user = User.find(create(:user).id) # fresh load => password attribute is nil
      user.name = "Renamed"
      expect(user).to be_valid # allow_nil => length validation skipped
    end
  end

  describe "email normalization" do
    it "normalizes email to lowercase" do
      user = create(:user, email_address: " Test@Example.COM ")
      expect(user.email_address).to eq("test@example.com")
    end
  end

  describe "#email_syncing?" do
    let(:user) { create(:user) }

    def grant_read(account)
      create(:email_account_user, user: user, email_account: account, can_read: true)
    end

    it "is true while a readable account has a fresh scan in flight" do
      account = create(:email_account, scanning: true, scan_started_at: 1.minute.ago)
      grant_read(account)
      expect(user.email_syncing?).to be true
    end

    it "is false once the scan claim has gone stale" do
      account = create(:email_account, scanning: true, scan_started_at: 20.minutes.ago)
      grant_read(account)
      expect(user.email_syncing?).to be false
    end

    it "is false when nothing is scanning" do
      account = create(:email_account, scanning: false)
      grant_read(account)
      expect(user.email_syncing?).to be false
    end

    it "ignores a scanning account the user cannot read" do
      create(:email_account, scanning: true, scan_started_at: 1.minute.ago)
      expect(user.email_syncing?).to be false
    end
  end

  describe "section visit tracking" do
    let(:user) { create(:user) }

    describe "#seen_section_at" do
      it "falls back to created_at when the section was never visited" do
        expect(user.seen_section_at(:mail)).to be_within(2.seconds).of(user.created_at)
      end

      it "returns the stored timestamp once recorded" do
        at = 3.hours.ago
        user.mark_section_seen!(:mail, at: at)
        expect(user.seen_section_at(:mail)).to be_within(2.seconds).of(at)
      end
    end

    describe "#mark_section_seen!" do
      it "stamps the section without touching others" do
        user.mark_section_seen!(:mail, at: 1.hour.ago)
        user.mark_section_seen!(:scout)

        expect(user.reload.section_seen_at.keys).to include("mail", "scout")
        expect(user.seen_section_at(:scout)).to be > 5.minutes.ago
      end

      it "ignores unknown sections" do
        expect { user.mark_section_seen!(:bogus) }
          .not_to(change { user.reload.section_seen_at })
      end
    end
  end

  describe "#writing_style_prompt" do
    let(:user) { build(:user, name: "Sam") }

    it "is blank when neither manual nor learned style is set" do
      expect(user.writing_style?).to be(false)
      expect(user.writing_style_prompt).to eq("")
    end

    it "includes the manual style under a 'How <name> writes' heading" do
      user.writing_style = "Breezy, signs off as Sam."
      expect(user.writing_style_prompt).to include("How Sam writes")
      expect(user.writing_style_prompt).to include("Breezy, signs off as Sam.")
    end

    it "includes the learned profile and prioritizes the stated preferences" do
      user.writing_style_learned = "Greets by first name."
      user.writing_style = "Always end with a question."
      prompt = user.writing_style_prompt
      expect(prompt).to include("Greets by first name.")
      expect(prompt).to include("Always end with a question.")
      # stated preferences come after (take priority over) the learned profile
      expect(prompt.index("Always end with a question.")).to be > prompt.index("Greets by first name.")
    end
  end

  describe "first-run tours" do
    subject(:user) { create(:user) }

    it "starts with no tours dismissed" do
      expect(user.dismissed_tours).to eq([])
      expect(user.tour_dismissed?("skim_intro")).to be(false)
    end

    it "#dismiss_tour! records a tour and is idempotent" do
      user.dismiss_tour!("skim_intro")
      user.dismiss_tour!("skim_intro")

      expect(user.reload.dismissed_tours).to eq([ "skim_intro" ])
      expect(user.tour_dismissed?("skim_intro")).to be(true)
    end

    it "accepts symbols and strings interchangeably" do
      user.dismiss_tour!(:doc_skim_intro)

      expect(user.tour_dismissed?(:doc_skim_intro)).to be(true)
      expect(user.tour_dismissed?("doc_skim_intro")).to be(true)
    end
  end
end
