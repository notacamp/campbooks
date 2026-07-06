require "rails_helper"

RSpec.describe Campbooks::ProductTour, type: :component do
  # Render the component in isolation, capturing the HTML output.
  def render_tour(**kwargs)
    ApplicationController.render(
      described_class.new(**kwargs),
      layout: false
    )
  end

  describe "slide count" do
    context "when tasks are disabled (default)" do
      before { allow(Features).to receive(:tasks?).and_return(false) }

      it "includes 5 progress segments" do
        html = render_tour
        # One segment button per slide
        expect(html.scan(/tour-seg-btn/).size).to eq(5)
      end

      it "does NOT include a tasks slide" do
        html = render_tour
        expect(html).not_to include('data-tour-slide-type="tasks"')
      end
    end

    context "when tasks are enabled" do
      before { allow(Features).to receive(:tasks?).and_return(true) }

      it "includes 6 progress segments" do
        html = render_tour
        expect(html.scan(/tour-seg-btn/).size).to eq(6)
      end

      it "includes a tasks slide" do
        html = render_tour
        expect(html).to include('data-tour-slide-type="tasks"')
      end
    end
  end

  describe "slide structure" do
    before { allow(Features).to receive(:tasks?).and_return(false) }

    it "renders all five non-tasks slides" do
      html = render_tour
      %w[intro inbox calendar docs more].each do |type|
        expect(html).to include("data-tour-slide-type=\"#{type}\"")
      end
    end

    it "renders the segmented progress bar" do
      html = render_tour
      expect(html).to include("tour-seg-btn")
      expect(html).to include("tour-seg-fill")
    end

    it "renders prev and next nav buttons" do
      html = render_tour
      expect(html).to include('data-product-tour-target="prevBtn"')
      expect(html).to include('data-product-tour-target="nextBtn"')
    end

    it "renders the rotation items on the intro slide" do
      html = render_tour
      # 5 rotation items (index 0 to 4)
      (0..4).each do |i|
        expect(html).to include("data-tour-rot-item=\"#{i}\"")
      end
    end
  end

  describe "slide 6 connect CTA" do
    before { allow(Features).to receive(:tasks?).and_return(false) }

    context "when no inbox is connected (default)" do
      it "shows the 'Connect your inbox' CTA pointing to onboarding" do
        html = render_tour
        expect(html).to include("/onboarding")
        expect(html).to include(I18n.t("components.product_tour.connect_cta"))
      end
    end

    context "when the user has an active inbox" do
      let(:workspace) { create(:workspace) }
      let(:user) { create(:user, workspace: workspace) }
      let!(:email_account) { create(:email_account, workspace: workspace) }

      it "shows the 'Back to your inbox' CTA pointing to root" do
        # Stub helpers.current_user to return a user with an email account
        allow_any_instance_of(described_class).to receive(:user_has_active_inbox?).and_return(true)
        html = render_tour
        expect(html).to include('data-tour-connect-path="/"')
        expect(html).to include(I18n.t("components.product_tour.done_cta"))
      end
    end
  end

  describe "autostart data attribute" do
    before { allow(Features).to receive(:tasks?).and_return(false) }

    it "sets data-tour-autostart to 'true' when autostart is true" do
      html = render_tour(autostart: true)
      expect(html).to include('data-tour-autostart="true"')
    end

    it "sets data-tour-autostart to 'false' by default" do
      html = render_tour
      expect(html).to include('data-tour-autostart="false"')
    end
  end

  describe "DOCS_URL" do
    it "uses the docs URL constant" do
      html = render_tour
      expect(html).to include(Campbooks::ProductTour::DOCS_URL)
    end
  end
end
