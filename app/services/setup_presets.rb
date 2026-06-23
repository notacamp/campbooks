# Starter packs offered in the setup wizards so most owners never have to write
# a name, prompt, or pick a color by hand. Colors are a curated, on-brand set
# (anchored on the violet accent) rather than arbitrary hashes. See ADR 0001.
module SetupPresets
  # Curated palette used for preset chips and for auto-coloring custom entries.
  # Harmonious with the electric-violet accent; distinct enough to tell apart.
  PALETTE = %w[
    #7c5cfc #6366f1 #3b82f6 #0ea5e9 #14b8a6 #10b981 #f59e0b #f43f5e #ec4899 #64748b
  ].freeze

  DOCUMENT_TYPES = [
    { name: "invoice",        color: "#7c5cfc", description: "Bills you send or receive",
      prompt: "Extract the invoice number, issue and due dates, supplier, line items, subtotal, tax, and total amount due." },
    { name: "receipt",        color: "#10b981", description: "Proof of a payment made",
      prompt: "Extract the merchant, date, payment method, line items, tax, and total paid." },
    { name: "contract",       color: "#6366f1", description: "Signed agreements",
      prompt: "Extract the parties, effective and end dates, key obligations, payment terms, and renewal or termination clauses." },
    { name: "bank statement", color: "#0ea5e9", description: "Monthly account summaries",
      prompt: "Extract the account holder, account number, statement period, opening and closing balances, and notable transactions." },
    { name: "tax document",   color: "#f59e0b", description: "Filings and tax notices",
      prompt: "Extract the tax year, form or reference number, issuing authority, amounts owed or refunded, and deadlines." },
    { name: "payslip",        color: "#14b8a6", description: "Salary and wage slips",
      prompt: "Extract the employee, employer, pay period, gross pay, deductions, and net pay." }
  ].freeze

  TAGS = [
    { name: "finance",    color: "#10b981", description: "Money in and out",
      prompt: "Apply to emails about invoices, payments, banking, expenses, or anything financial." },
    { name: "legal",      color: "#6366f1", description: "Contracts and compliance",
      prompt: "Apply to emails about contracts, agreements, compliance, disputes, or anything from lawyers." },
    { name: "urgent",     color: "#f43f5e", description: "Needs attention now",
      prompt: "Apply to emails that are time-sensitive or explicitly request a fast response." },
    { name: "newsletter", color: "#0ea5e9", description: "Updates and digests",
      prompt: "Apply to marketing emails, newsletters, and bulk announcements." },
    { name: "receipts",   color: "#f59e0b", description: "Purchase confirmations",
      prompt: "Apply to order confirmations, payment receipts, and proof-of-purchase emails." },
    { name: "personal",   color: "#8b5cf6", description: "Non-business mail",
      prompt: "Apply to personal correspondence unrelated to the business." }
  ].freeze

  def self.document_type(name) = DOCUMENT_TYPES.find { |p| p[:name] == name.to_s.strip.downcase }
  def self.tag(name) = TAGS.find { |p| p[:name] == name.to_s.strip.downcase }

  # Deterministic, on-brand color for a custom (non-preset) name.
  def self.color_for(name)
    PALETTE[name.to_s.bytes.sum % PALETTE.size]
  end
end
