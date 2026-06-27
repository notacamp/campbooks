class OrganizationCardPreview < Lookbook::Preview
  def default
    org = Organization.new(name: "Acme Corp", domain: "acme.com")
    allow(org).to receive(:id).and_return(SecureRandom.uuid)
    allow(org).to receive(:persisted?).and_return(true)
    allow(org).to receive(:member_count).and_return(12)
    allow(org).to receive(:email_count).and_return(145)
    render Campbooks::OrganizationCard.new(organization: org)
  end
end
