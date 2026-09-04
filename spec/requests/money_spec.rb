# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Money", type: :request do
  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user) { create(:user, workspace:) }

  def revenue(**attrs)
    create(:document, :approved, :revenue_invoice, workspace:, currency: "EUR", **attrs)
  end

  def expense(**attrs)
    create(:document, :approved, workspace:, document_type: :expense_invoice, currency: "EUR", **attrs)
  end

  def with_flags(accounting: "1", &)
    with_env("ENABLE_ACCOUNTING" => accounting, &)
  end

  describe "gating" do
    it "404s when accounting is off" do
      with_flags(accounting: nil) { sign_in(user); get money_path }
      expect(response).to have_http_status(:not_found)
    end

    it "blocks a workspace without the accounting entitlement" do
      free = create(:workspace, plan: "free")
      free_user = create(:user, workspace: free)
      with_flags { sign_in(free_user); get money_path }
      expect(response).to have_http_status(:redirect)
    end
  end

  describe "GET /money" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "renders the sections and the timeline" do
      revenue(client_name: "Acme", invoice_number: "0234", amount_cents: 222_000, due_date: Date.current + 12)
      expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: Date.current - 20)

      get money_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Cloudhost").and include("Acme")
      expect(response.body).to include("viewBox") # the inline SVG timeline
      expect(response.body).to include("what the bank says")
    end

    it "shows the empty state when nothing is owed or due" do
      get money_path
      expect(response.body).to include(I18n.t("money.index.empty_title"))
    end
  end

  describe "GET /money/export.csv" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "streams a CSV of the ledger with headers and a row" do
      expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: Date.current - 20)
      get money_export_path(format: :csv)

      expect(response.media_type).to eq("text/csv")
      expect(response.body).to include("Counterpart,What,Direction,Amount")
      expect(response.body).to include("Cloudhost")
      expect(response.body).to include("you owe")
    end
  end

  describe "row actions" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "remind creates a payment_due reminder for the day after due" do
      doc = revenue(client_name: "Acme", invoice_number: "0234", amount_cents: 222_000, due_date: Date.current + 12)
      expect {
        post money_obligation_remind_path("doc:#{doc.id}"), as: :turbo_stream
      }.to change(Reminder, :count).by(1)
      reminder = Reminder.last
      expect(reminder.reminder_type).to eq("payment_due")
      expect(reminder.due_at.to_date).to eq(Date.current + 13)
    end

    it "chase opens the compose Dock prefilled with the reminder draft" do
      doc = revenue(client_name: "Brightloop", invoice_number: "0231", amount_cents: 120_000, due_date: Date.current - 12)
      post money_obligation_chase_path("doc:#{doc.id}"), as: :turbo_stream

      expect(response.body).to include("compose_dock")
      expect(response.body).to include("payment reminder")
    end

    it "settle marks the document paid" do
      doc = expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: Date.current - 20)
      post money_obligation_settle_path("doc:#{doc.id}"), as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(doc.reload).to be_settled
      expect(doc.settled_source).to eq("manual")
    end

    it "unsettle clears a manual settlement" do
      doc = expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: Date.current - 20,
                    settled_at: Time.current, settled_source: "manual")
      delete money_obligation_settle_path("doc:#{doc.id}"), as: :turbo_stream
      expect(doc.reload).not_to be_settled
    end

    it "404s for an obligation outside the workspace" do
      other = create(:document, :approved, document_type: :expense_invoice, amount_cents: 1_000, due_date: Date.current - 1)
      post money_obligation_settle_path("doc:#{other.id}"), as: :turbo_stream
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "ledger sorting" do
    before { sign_in(user) }

    it "sorts by the column asked for, marks it with aria-sort, and remembers it for the session" do
      with_flags do
        expense(vendor_name: "Bravo", amount_cents: 5_000, due_date: Date.current - 3)
        expense(vendor_name: "Alpha", amount_cents: 9_000, due_date: Date.current - 2)

        get money_path(sort: "amount", dir: "asc")
        expect(response).to have_http_status(:ok)
        href = response.body[/aria-sort="ascending"[^>]*>\s*<a[^>]*href="([^"]+)"/, 1]
        expect(href).to include("sort=amount").and include("dir=desc") # clicking the active column flips it
        expect(table_order).to eq(%w[Bravo Alpha])

        get money_path
        expect(response.body).to include('aria-sort="ascending"')
        expect(table_order).to eq(%w[Bravo Alpha])

        get money_path(order: "counterpart_desc")
        expect(response.body).to include('aria-sort="descending"')
        expect(table_order).to eq(%w[Bravo Alpha])
      end
    end

    # The counterparts in the ledger table's row order (the timeline above it lists
    # the same names in date order, so the whole page is the wrong thing to scan).
    def table_order
      table = response.body[%r{<table.*?</table>}m]
      table.scan(/<td class="px-3 py-3 font-semibold text-foreground">([^<]+)</).flatten
    end
  end
end
