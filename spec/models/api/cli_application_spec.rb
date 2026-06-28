require "rails_helper"

RSpec.describe Api::CliApplication do
  describe ".ensure!" do
    it "creates a public, ownerless, PKCE CLI client carrying the full scope catalog" do
      app = described_class.ensure!

      expect(app).to be_persisted
      expect(app.uid).to eq(described_class::UID)
      expect(app.confidential).to be(false)
      expect(app.workspace_id).to be_nil
      expect(app.created_by_id).to be_nil
      expect(app.scopes.to_s.split).to match_array(Api::Scopes.all)
      expect(app.redirect_uri).to include("http://127.0.0.1/callback")
      expect(app.redirect_uri).to include("urn:ietf:wg:oauth:2.0:oob")
    end

    it "is idempotent — repeated calls don't create duplicates" do
      first = described_class.ensure!

      expect { described_class.ensure! }
        .not_to change { Doorkeeper::Application.where(uid: described_class::UID).count }
      expect(described_class.record.id).to eq(first.id)
    end
  end
end
