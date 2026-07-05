require "rails_helper"

# Regression guard for the documents-list month picker. The <input type="month">
# submits a single "YYYY-MM" value, so the controller must parse it into the
# year+month pair `for_month` expects. It previously looked for a separate
# params[:year] that no view ever sends (and `"2026-06".to_i` is 2026, not 6), so
# selecting a month silently did nothing.
RSpec.describe "Documents list filters", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  # display_title prefers metadata["title"], so a distinctive title gives each
  # document a stable marker to assert on in the rendered list.
  def titled(title, date)
    create(:document, :approved, workspace: workspace,
           document_date: date, metadata: { "title" => title })
  end

  describe "GET /documents?month=YYYY-MM" do
    it "narrows the list to documents dated in the selected month" do
      titled("JUNE-ALPHA", Date.new(2026, 6, 15))
      titled("JUNE-BETA",  Date.new(2026, 6, 2))
      titled("MAY-DOC",    Date.new(2026, 5, 10))
      titled("APRIL-DOC",  Date.new(2026, 4, 1))

      get files_path(month: "2026-06")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("JUNE-ALPHA", "JUNE-BETA")
      expect(response.body).not_to include("MAY-DOC")
      expect(response.body).not_to include("APRIL-DOC")
    end

    it "shows every document when no month is selected" do
      titled("JUNE-ALPHA", Date.new(2026, 6, 15))
      titled("MAY-DOC",    Date.new(2026, 5, 10))

      get files_path

      expect(response.body).to include("JUNE-ALPHA", "MAY-DOC")
    end

    it "ignores an unparseable month instead of raising" do
      titled("JUNE-ALPHA", Date.new(2026, 6, 15))

      get files_path(month: "garbage")

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("JUNE-ALPHA")
    end
  end
end
