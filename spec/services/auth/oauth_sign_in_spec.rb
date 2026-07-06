require "rails_helper"

RSpec.describe Auth::OauthSignIn do
  def call(**overrides)
    described_class.call(**{ provider: :google, uid: "uid-1", email: "new@example.com", name: "New Person" }.merge(overrides))
  end

  describe "preconditions" do
    it "blocks (:invalid) when uid is blank — never falls through to an email-only match" do
      result = call(uid: "")
      expect(result).to be_blocked
      expect(result.reason).to eq(:invalid)
    end

    it "blocks (:invalid) when email is blank" do
      expect(call(email: "").reason).to eq(:invalid)
    end
  end

  describe "A — an Identity already exists for (provider, uid)" do
    let(:user) { create(:user) }
    let!(:identity) { create(:identity, user: user, provider: "google", uid: "g-1", email: "old@example.com") }

    it "signs in the identity's user without creating a new user" do
      expect { @result = call(uid: "g-1", email: "old@example.com") }.not_to change(User, :count)
      expect(@result).to be_signed_in
      expect(@result.user).to eq(user)
    end

    it "refreshes the stored display email when it changed" do
      call(uid: "g-1", email: "renamed@example.com")
      expect(identity.reload.email).to eq("renamed@example.com")
    end

    it "blocks (:deletion_requested) when the user is pending deletion" do
      user.update!(deletion_requested_at: Time.current)
      result = call(uid: "g-1", email: "old@example.com")
      expect(result).to be_blocked
      expect(result.reason).to eq(:deletion_requested)
    end

    it "matches by uid even if a DIFFERENT provider shares that uid" do
      create(:identity, provider: "microsoft", uid: "g-1", email: "someone@else.com")
      result = call(provider: :google, uid: "g-1", email: "old@example.com")
      expect(result.user).to eq(user)
    end
  end

  describe "B — the email already belongs to a user (no linked identity)" do
    it "blocks (:existing_account) and never signs in by email match" do
      create(:user, email_address: "taken@example.com")
      result = call(email: "taken@example.com")
      expect(result).to be_blocked
      expect(result.reason).to eq(:existing_account)
    end

    it "is the takeover guard: a password account is unreachable via OAuth email match" do
      victim = create(:user, email_address: "victim@example.com", password_set_by_user: true)

      expect {
        @result = call(provider: :google, uid: "attacker-controlled-uid", email: "victim@example.com")
      }.not_to change(Identity, :count)

      expect(@result).to be_blocked
      expect(victim.reload.identities).to be_empty
    end
  end

  describe "C — the email is a connected mailbox (no user, no identity)" do
    it "blocks (:mailbox_has_owner) when the mailbox has an owner" do
      account = create(:email_account, email_address: "team@acme.com")
      create(:email_account_user, :owner, email_account: account)

      result = call(email: "team@acme.com")
      expect(result.reason).to eq(:mailbox_has_owner)
    end

    it "blocks (:mailbox_no_owner) for an ownerless mailbox" do
      create(:email_account, email_address: "orphan@acme.com")
      result = call(email: "orphan@acme.com")
      expect(result.reason).to eq(:mailbox_no_owner)
    end

    it "matches the mailbox case-insensitively" do
      create(:email_account, email_address: "Mixed@Acme.com")
      create(:email_account_user, :owner, email_account: EmailAccount.last)
      expect(call(email: "mixed@acme.com").reason).to eq(:mailbox_has_owner)
    end
  end

  describe "D — a brand-new person" do
    it "creates a user in a FRESH personal workspace (not grouped by email domain)" do
      expect { call(email: "first@gmail.com", name: "First Last") }.to change(User, :count).by(1)

      user = User.find_by(email_address: "first@gmail.com")
      expect(user.name).to eq("First Last")
      expect(user.workspace.slug).to start_with("ws-")
    end

    it "does NOT seat two unrelated same-domain users in one workspace" do
      call(email: "ann@gmail.com", uid: "u-ann", name: "Ann")
      call(email: "bob@gmail.com", uid: "u-bob", name: "Bob")

      ann = User.find_by(email_address: "ann@gmail.com")
      bob = User.find_by(email_address: "bob@gmail.com")
      expect(ann.workspace_id).not_to eq(bob.workspace_id)
    end

    it "creates the matching Identity and returns a sign-in" do
      result = call(provider: :microsoft, uid: "ms-9", email: "fresh@corp.com", name: "Fresh")
      expect(result).to be_signed_in
      identity = result.user.identities.sole
      expect(identity).to have_attributes(provider: "microsoft", uid: "ms-9", email: "fresh@corp.com")
    end

    it "marks the new user as OAuth-only (no real password)" do
      result = call(email: "oauthonly@corp.com")
      expect(result.user.password_set_by_user).to be(false)
    end

    it "makes the founder their workspace's admin, without instance access" do
      create(:user) # the instance already has users — only the very first gets app_admin
      result = call(email: "founder@corp.com")
      expect(result.user).to be_admin
      expect(result.user.app_admin?).to be(false)
    end

    it "provisions the managed AI default for the new workspace" do
      allow(Ai::ProviderSetup).to receive(:apply_managed_default)
      call(email: "ai-provision@corp.com", name: "AI User")
      expect(Ai::ProviderSetup).to have_received(:apply_managed_default).with(
        an_object_having_attributes(slug: starting_with("ws-"))
      )
    end

    it "provisions default tag groups for the new workspace" do
      allow(Tags::DefaultGroups).to receive(:provision!)
      call(email: "tags-provision@corp.com", name: "Tags User")
      expect(Tags::DefaultGroups).to have_received(:provision!).with(
        an_object_having_attributes(slug: starting_with("ws-"))
      )
    end

    it "still signs the user in even when AI provisioning raises" do
      allow(Ai::ProviderSetup).to receive(:apply_managed_default).and_raise(StandardError, "provider down")
      result = call(email: "resilient@corp.com")
      expect(result).to be_signed_in
    end

    it "still signs the user in even when tag provisioning raises" do
      allow(Tags::DefaultGroups).to receive(:provision!).and_raise(StandardError, "groups down")
      result = call(email: "resilient-tags@corp.com")
      expect(result).to be_signed_in
    end
  end
end
