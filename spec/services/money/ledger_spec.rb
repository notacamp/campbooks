# frozen_string_literal: true

require "rails_helper"

RSpec.describe Money::Ledger do
  let(:today) { Date.new(2026, 9, 3) }
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def revenue(**attrs)
    create(:document, :approved, :revenue_invoice, workspace: workspace, currency: "EUR", **attrs)
  end

  def expense(**attrs)
    create(:document, :approved, workspace: workspace, document_type: :expense_invoice, currency: "EUR", **attrs)
  end

  def ledger = described_class.for(workspace, user, today: today)

  describe "documents in both directions" do
    it "reads a revenue invoice as a receivable and an expense as a payable" do
      revenue(client_name: "Acme", invoice_number: "0234", amount_cents: 222_000, due_date: today + 12)
      expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - 20)

      receivable = ledger.obligations.find(&:receivable?)
      payable = ledger.obligations.find(&:payable?)

      expect(receivable.counterpart).to eq("Acme")
      expect(receivable.status).to eq(:due)
      expect(receivable.what).to eq("Invoice #0234 you sent")
      expect(receivable.amount.format).to eq("€2,220.00")
      expect(receivable.actions).to eq(%i[remind_on])

      expect(payable.status).to eq(:late)
      expect(payable.days_late(today)).to eq(20)
      expect(payable.actions).to eq(%i[mark_paid])
    end

    it "marks a late receivable's actions with mark_paid + send_reminder" do
      revenue(client_name: "Brightloop", invoice_number: "0231", amount_cents: 120_000, due_date: today - 12)
      expect(ledger.late.first.actions).to eq(%i[mark_paid send_reminder])
    end

    it "offers Pay when a payment URL is present" do
      doc = expense(vendor_name: "Staples", amount_cents: 36_400, due_date: today + 5)
      doc.update!(metadata: doc.metadata.merge("payment_url" => "https://pay.example.com/x"))
      obligation = ledger.due.find { |o| o.counterpart == "Staples" }
      expect(obligation.pay_url).to eq("https://pay.example.com/x")
      expect(obligation.actions).to eq(%i[mark_paid pay])
    end
  end

  describe "estimated due dates" do
    it "estimates 30 days after the document date for an invoice with no due date" do
      expense(vendor_name: "NoDue", amount_cents: 5_000, due_date: nil, document_date: today - 3)
      obligation = ledger.obligations.find { |o| o.counterpart == "NoDue" }

      expect(obligation.due_on).to eq((today - 3) + 30.days)
      expect(obligation).to be_due_estimated
    end

    it "drops a non-invoice money document with no due date" do
      create(:document, :approved, :receipt, workspace: workspace, amount_cents: 5_000, due_date: nil, document_date: today - 3)
      expect(ledger.obligations).to be_empty
    end
  end

  describe "settlement window" do
    it "keeps a document settled within the lookback and drops an older one" do
      expense(vendor_name: "Recent", amount_cents: 9_600, due_date: today - 10,
              settled_at: (today - 6).to_time, settled_source: "manual")
      expense(vendor_name: "Ancient", amount_cents: 9_600, due_date: today - 90,
              settled_at: (today - 60).to_time, settled_source: "manual")

      expect(ledger.settled.map(&:counterpart)).to eq([ "Recent" ])
      expect(ledger.settled.first.status).to eq(:settled)
      expect(ledger.settled.first.actions).to be_empty
    end
  end

  describe "reminders" do
    it "turns a renewal reminder into a decide obligation" do
      policy = create(:document, :approved, workspace: workspace, document_type: :insurance_policy)
      create(:reminder, workspace: workspace, source: policy, reminder_type: :renewal,
                        title: "Seguro renews", amount_cents: 41_200, currency: "EUR", due_at: (today + 28).to_time)

      decide = ledger.obligations.find(&:decide?)
      expect(decide.direction).to eq(:payable)
      expect(decide.what).to eq("Policy renewal · yearly")
      expect(decide.actions).to eq(%i[keep cancel])
      expect(ledger.due).to include(decide) # decide rides in the Due section
    end

    it "dedupes a payment_due reminder that a document obligation already covers" do
      invoice = expense(vendor_name: "Dup", amount_cents: 5_000, due_date: today + 4)
      create(:reminder, workspace: workspace, source: invoice, reminder_type: :payment_due,
                        amount_cents: 5_000, currency: "EUR", due_at: (today + 4).to_time)

      expect(ledger.obligations.count).to eq(1)
      expect(ledger.obligations.first.reminder).to be_nil
    end
  end

  describe "sections + sorting" do
    it "orders the sections late, due, settled and each by date" do
      expense(vendor_name: "LateA", amount_cents: 1_000, due_date: today - 5)
      revenue(client_name: "DueB", invoice_number: "9", amount_cents: 2_000, due_date: today + 2)
      expense(vendor_name: "Paid", amount_cents: 3_000, due_date: today - 30,
              settled_at: (today - 2).to_time, settled_source: "manual")

      expect(ledger.sections.map(&:first)).to eq(%i[late due settled])
    end

    it "puts the newest first within a section by default" do
      expense(vendor_name: "Older", amount_cents: 1_000, due_date: today - 20)
      expense(vendor_name: "Newer", amount_cents: 1_000, due_date: today - 2)
      revenue(client_name: "Soon", invoice_number: "1", amount_cents: 1_000, due_date: today + 2)
      revenue(client_name: "Later", invoice_number: "2", amount_cents: 1_000, due_date: today + 20)

      late = ledger.sections.to_h[:late].map(&:counterpart)
      due  = ledger.sections.to_h[:due].map(&:counterpart)
      expect(late).to eq(%w[Newer Older])
      expect(due).to eq(%w[Later Soon])
      expect(ledger.sort).to eq(:date)
      expect(ledger.dir).to eq(:desc)
    end

    it "sorts by amount and by counterpart in either direction" do
      expense(vendor_name: "Bravo", amount_cents: 5_000, due_date: today - 3)
      expense(vendor_name: "Alpha", amount_cents: 9_000, due_date: today - 2)
      expense(vendor_name: "Charlie", amount_cents: 1_000, due_date: today - 1)

      by_amount = described_class.for(workspace, user, today: today, sort: :amount)
      expect(by_amount.sections.to_h[:late].map(&:counterpart)).to eq(%w[Alpha Bravo Charlie])
      expect(by_amount.dir).to eq(:desc)

      by_amount_asc = described_class.for(workspace, user, today: today, sort: :amount, dir: :asc)
      expect(by_amount_asc.sections.to_h[:late].map(&:counterpart)).to eq(%w[Charlie Bravo Alpha])

      by_name = described_class.for(workspace, user, today: today, sort: :counterpart)
      expect(by_name.sections.to_h[:late].map(&:counterpart)).to eq(%w[Alpha Bravo Charlie])
      expect(by_name.dir).to eq(:asc)

      by_name_desc = described_class.for(workspace, user, today: today, sort: "counterpart", dir: "desc")
      expect(by_name_desc.sections.to_h[:late].map(&:counterpart)).to eq(%w[Charlie Bravo Alpha])
    end

    it "falls back to date, newest first, for an unknown sort" do
      unknown = described_class.for(workspace, user, today: today, sort: :bogus, dir: :sideways)
      expect(unknown.sort).to eq(:date)
      expect(unknown.dir).to eq(:desc)
    end
  end

  describe "recurrence" do
    it "tags a recurring counterpart's obligation with the subscription suffix" do
      3.times { |i| expense(vendor_name: "Cloudhost", amount_cents: 24_800, due_date: today - (65 - i * 30)) }
      current = ledger.obligations.find { |o| o.counterpart == "Cloudhost" && !o.settled? }
      expect(current.what).to include("· subscription")
      expect(current).to be_recurring
    end
  end

  describe "permissions" do
    it "excludes documents from another workspace" do
      other_ws = create(:workspace)
      create(:document, :approved, workspace: other_ws, document_type: :expense_invoice, amount_cents: 9_999, due_date: today - 1)
      expect(ledger.obligations).to be_empty
    end

    it "hides a document filed only in a restricted folder from a user who cannot read it" do
      other = create(:user, workspace: workspace)
      restricted = create(:mail_folder, workspace: workspace, restricted: true)
      restricted.mail_folder_users.create!(user: user, can_read: true)
      doc = expense(vendor_name: "Secret", amount_cents: 1_234, due_date: today - 1)
      restricted.folder_memberships.create!(folderable: doc)

      expect(described_class.for(workspace, user, today: today).obligations.map(&:counterpart)).to include("Secret")
      expect(described_class.for(workspace, other, today: today).obligations.map(&:counterpart)).not_to include("Secret")
    end
  end

  it "finds an obligation by its id" do
    doc = expense(vendor_name: "Findable", amount_cents: 1_000, due_date: today - 1)
    expect(ledger.find("doc:#{doc.id}").counterpart).to eq("Findable")
  end
end
