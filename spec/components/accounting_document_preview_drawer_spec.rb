# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Accounting::DocumentPreviewDrawer, type: :component do
  it "renders the closed drawer with iframe, full-page link and dialog semantics" do
    html = ApplicationController.render(described_class.new, layout: false)

    expect(html).to include('data-controller="document-preview"')
    expect(html).to include('data-document-preview-target="iframe"')
    expect(html).to include('data-document-preview-target="fullPageLink"')
    expect(html).to include('data-document-preview-skip="true"')
    expect(html).to include('role="dialog"')
    expect(html).to include("invisible") # ships closed
  end
end
