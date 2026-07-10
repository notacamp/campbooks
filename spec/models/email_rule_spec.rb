# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmailRule, type: :model do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:tag)       { create(:tag, workspace: workspace) }

  # Minimal valid rule (archive action, non-empty criteria)
  def valid_rule(overrides = {})
    build(:email_rule, workspace: workspace, created_by: user,
          archive: true, criteria: { "from" => [ "@stripe.com" ] }, **overrides)
  end

  # -------------------------------------------------------------------------
  # Associations
  # -------------------------------------------------------------------------
  describe "associations" do
    it { is_expected.to belong_to(:workspace) }
    it { is_expected.to belong_to(:created_by).class_name("User").optional }
    it { is_expected.to belong_to(:mail_folder).optional }
    it { is_expected.to have_many(:email_rule_tags).dependent(:destroy) }
    it { is_expected.to have_many(:tags).through(:email_rule_tags) }
    it { is_expected.to have_many(:runs).class_name("EmailRuleRun").dependent(:destroy) }
  end

  # -------------------------------------------------------------------------
  # Validations
  # -------------------------------------------------------------------------
  describe "validations" do
    it "is valid with a name, at least one criterion, and at least one action" do
      rule = valid_rule
      expect(rule).to be_valid
    end

    it "requires a name" do
      rule = valid_rule(name: "")
      expect(rule).not_to be_valid
      expect(rule.errors[:name]).to be_present
    end

    describe "criteria_present" do
      it "rejects a rule with empty criteria hash" do
        rule = valid_rule(criteria: {})
        expect(rule).not_to be_valid
        expect(rule.errors[:criteria]).to be_present
      end

      it "rejects a rule with only blank array values (after normalization)" do
        rule = valid_rule(criteria: { "from" => [ "", "  " ] })
        expect(rule).not_to be_valid
        expect(rule.errors[:criteria]).to be_present
      end

      it "accepts a rule with has_attachment: true as criterion" do
        rule = valid_rule(criteria: { "has_attachment" => true })
        expect(rule).to be_valid
      end

      it "rejects a rule whose only criterion is the account selector" do
        # The account narrows scope but is not a condition — an account-only
        # rule would match that account's entire mailbox.
        rule = valid_rule(criteria: { "email_account_id" => account.id })
        expect(rule).not_to be_valid
        expect(rule.errors[:criteria]).to be_present
      end
    end

    describe "action_present" do
      it "rejects a rule with no action" do
        rule = build(:email_rule, workspace: workspace, created_by: user,
                     criteria: { "from" => [ "@stripe.com" ] },
                     archive: false, mark_read: false, mail_folder: nil)
        # No tags, no archive, no mark_read, no folder
        expect(rule).not_to be_valid
        expect(rule.errors[:base]).to be_present
      end

      it "accepts archive as the sole action" do
        rule = valid_rule(archive: true)
        expect(rule).to be_valid
      end

      it "accepts mark_read as the sole action" do
        rule = valid_rule(archive: false, mark_read: true)
        expect(rule).to be_valid
      end

      it "accepts a mail_folder as the sole action" do
        folder = create(:mail_folder, workspace: workspace)
        rule = valid_rule(archive: false, mark_read: false, mail_folder: folder)
        expect(rule).to be_valid
      end
    end

    describe "cross-workspace tag" do
      it "rejects a tag from a different workspace" do
        other_ws  = create(:workspace)
        other_tag = create(:tag, workspace: other_ws)
        rule = valid_rule
        rule.save!
        rule.tags << other_tag

        rule.validate
        expect(rule.errors[:tags]).to be_present
      end

      it "accepts a tag from the same workspace" do
        rule = valid_rule
        rule.save!
        rule.tags << tag

        rule.validate
        expect(rule.errors[:tags]).to be_empty
      end
    end

    describe "cross-workspace folder" do
      it "rejects a folder from a different workspace" do
        other_ws     = create(:workspace)
        other_folder = create(:mail_folder, workspace: other_ws)
        rule = build(:email_rule, workspace: workspace, created_by: user,
                     criteria: { "from" => [ "@stripe.com" ] },
                     archive: false, mark_read: false, mail_folder: other_folder)
        expect(rule).not_to be_valid
        expect(rule.errors[:mail_folder]).to be_present
      end
    end
  end

  # -------------------------------------------------------------------------
  # Criteria normalisation (before_validation)
  # -------------------------------------------------------------------------
  describe "criteria normalisation" do
    it "strips blank values from array criteria" do
      rule = valid_rule(criteria: { "from" => [ "@stripe.com", "", "  " ] })
      rule.validate
      expect(rule.criteria["from"]).to eq([ "@stripe.com" ])
    end

    it "splits comma-separated inputs" do
      rule = valid_rule(criteria: { "from" => [ "@stripe.com, @acme.com" ] })
      rule.validate
      expect(rule.criteria["from"]).to eq([ "@stripe.com", "@acme.com" ])
    end

    it "downcases from addresses" do
      rule = valid_rule(criteria: { "from" => [ "BILLING@ACME.COM" ] })
      rule.validate
      expect(rule.criteria["from"]).to eq([ "billing@acme.com" ])
    end

    it "downcases to addresses" do
      rule = valid_rule(criteria: { "to" => [ "ME@EXAMPLE.COM" ], "from" => [ "@a.com" ] })
      rule.validate
      expect(rule.criteria["to"]).to eq([ "me@example.com" ])
    end

    it "does not downcase subject or body" do
      rule = valid_rule(criteria: { "subject" => [ "INVOICE" ], "from" => [ "@a.com" ] })
      rule.validate
      expect(rule.criteria["subject"]).to eq([ "INVOICE" ])
    end

    it "preserves has_attachment: true" do
      rule = valid_rule(criteria: { "has_attachment" => true, "from" => [ "@a.com" ] })
      rule.validate
      expect(rule.criteria["has_attachment"]).to eq(true)
    end

    it "preserves email_account_id" do
      rule = valid_rule(criteria: { "email_account_id" => account.id, "from" => [ "@a.com" ] })
      rule.validate
      expect(rule.criteria["email_account_id"]).to eq(account.id)
    end

    it "drops keys that have no values after normalisation" do
      rule = valid_rule(criteria: { "from" => [ "@stripe.com" ], "subject" => [ "" ] })
      rule.validate
      expect(rule.criteria).not_to have_key("subject")
    end
  end

  # -------------------------------------------------------------------------
  # Scope
  # -------------------------------------------------------------------------
  describe ".enabled" do
    it "returns only enabled rules" do
      enabled  = create(:email_rule, workspace: workspace, created_by: user,
                        archive: true, criteria: { "from" => [ "@a.com" ] }, enabled: true)
      disabled = create(:email_rule, workspace: workspace, created_by: user,
                        archive: true, criteria: { "from" => [ "@b.com" ] }, enabled: false)

      expect(EmailRule.enabled).to include(enabled)
      expect(EmailRule.enabled).not_to include(disabled)
    end
  end
end
