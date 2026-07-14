require "rails_helper"

# Regression: the Files list renders inside the `files_results` Turbo Frame
# (app/views/files/index.html.erb). Every link that leaves the list (open a
# document / internal doc / email, download) must carry data-turbo-frame="_top",
# or Turbo navigates the frame to a page without it and shows "Content missing".
RSpec.describe "Files list frame escape", type: :component do
  def render_component(component)
    ApplicationController.render(component, layout: false)
  end

  # The outbound href must sit on an <a> tag that also carries the frame escape.
  def expect_top_frame_link(html, href)
    anchors = html.scan(/<a\s[^>]*>/)
    matching = anchors.select { |a| a.include?(%(href="#{href}")) }
    expect(matching).not_to be_empty, "no <a> with href=#{href.inspect} found"
    matching.each do |anchor|
      expect(anchor).to include('data-turbo-frame="_top"'),
        "expected #{anchor} to carry data-turbo-frame=\"_top\""
    end
  end

  describe Campbooks::Files::FileRow do
    it "opens the document outside the files_results frame" do
      doc = create(:document)
      html = render_component(described_class.new(doc: doc))
      expect_top_frame_link(html, "/documents/#{doc.id}")
    end
  end

  describe Campbooks::Files::FileCard do
    it "opens the document outside the files_results frame" do
      doc = create(:document)
      html = render_component(described_class.new(doc: doc))
      expect_top_frame_link(html, "/documents/#{doc.id}")
    end
  end

  describe Campbooks::Files::FileTile do
    it "opens the document outside the files_results frame" do
      doc = create(:document)
      html = render_component(described_class.new(doc: doc))
      expect_top_frame_link(html, "/documents/#{doc.id}")
    end
  end

  describe Campbooks::Files::FileActionsMenu do
    it "opens and downloads outside the files_results frame" do
      doc = create(:document)
      html = render_component(described_class.new(doc: doc))
      expect_top_frame_link(html, "/documents/#{doc.id}")
      expect_top_frame_link(html, "/documents/#{doc.id}/file?disposition=attachment")
    end
  end

  describe Campbooks::Files::DocRow do
    it "opens and edits the internal document outside the files_results frame" do
      doc = create(:authored_document)
      html = render_component(described_class.new(doc: doc))
      expect_top_frame_link(html, "/documents/write/#{doc.id}")
      expect_top_frame_link(html, "/documents/write/#{doc.id}/edit")
    end
  end

  describe Campbooks::Files::EmailRow do
    it "opens the email outside the files_results frame" do
      email = create(:email_message)
      html = render_component(described_class.new(email: email))
      expect_top_frame_link(html, "/email_messages/#{email.id}")
    end
  end
end
