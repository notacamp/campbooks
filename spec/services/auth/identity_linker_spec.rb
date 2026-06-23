require "rails_helper"

RSpec.describe Auth::IdentityLinker do
  let(:user) { create(:user) }

  def link(**overrides)
    described_class.call(**{ user: user, provider: :google, uid: "g-1", email: "me@example.com" }.merge(overrides))
  end

  it "links a new identity to the user" do
    expect { @r = link }.to change { user.identities.count }.by(1)
    expect(@r.ok?).to be(true)
    expect(@r.status).to eq(:linked)
    expect(@r.identity).to have_attributes(provider: "google", uid: "g-1", email: "me@example.com")
  end

  it "is idempotent when the same account is already linked to this user" do
    link
    expect { @r = link }.not_to change(Identity, :count)
    expect(@r.status).to eq(:already_linked)
    expect(@r.ok?).to be(true)
  end

  it "refreshes the stored email on re-link" do
    link(email: "old@example.com")
    link(email: "new@example.com")
    expect(user.identities.sole.email).to eq("new@example.com")
  end

  it "blocks linking an account already tied to a DIFFERENT user" do
    other = create(:user)
    create(:identity, user: other, provider: "google", uid: "g-1")

    expect { @r = link }.not_to change { user.identities.count }
    expect(@r.ok?).to be(false)
    expect(@r.reason).to eq(:linked_to_other_user)
  end

  it "blocks when uid is blank" do
    expect(link(uid: "").reason).to eq(:invalid)
  end
end
