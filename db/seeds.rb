puts "Creating workspace..."

org = Workspace.find_or_create_by!(slug: "demo") do |o|
  o.name = "Demo Workspace"
  o.settings = {
    "company_name" => ENV.fetch("SEED_COMPANY_NAME", "Demo Corp"),
    "company_nif" => ENV.fetch("SEED_COMPANY_NIF", "123456789"),
    "app_name" => "Campbooks",
    "workspace_context" => "A consulting firm that helps businesses with financial management, tax compliance, and operations.",
    "default_currency" => "EUR"
  }
end
# Ensure seed org is marked as onboarded
org.settings["onboarding_completed_at"] ||= Time.current.iso8601
org.save! if org.settings_changed?

# Grant accounting to the demo workspace so the full reconciliation workbench
# (resolve panel, confirm/reject/exclude actions) is accessible in development.
accounting_override = { "accounting" => { "allowed" => true, "enabled" => true } }
unless (org.entitlement_overrides || {})["accounting"]&.dig("allowed")
  org.update!(entitlement_overrides: (org.entitlement_overrides || {}).merge(accounting_override))
end

puts "Workspace: #{org.name}"

puts "Creating users..."

User.find_or_create_by!(email_address: "admin@example.com") do |user|
  user.name = "Admin"
  user.password = ENV.fetch("SEED_PASSWORD", "changeme123")
  user.password_confirmation = ENV.fetch("SEED_PASSWORD", "changeme123")
  user.workspace = org
  user.role = :admin # workspace admin
  user.app_admin = true # and instance operator (/admin, /jobs)
end
# Older seeded databases predate the app_admin flag — keep the demo admin an operator.
User.find_by(email_address: "admin@example.com")&.then { |u| u.update!(app_admin: true) unless u.app_admin? }

User.find_or_create_by!(email_address: "partner@example.com") do |user|
  user.name = "Partner"
  user.password = ENV.fetch("SEED_PASSWORD", "changeme123")
  user.password_confirmation = ENV.fetch("SEED_PASSWORD", "changeme123")
  user.workspace = org
  user.role = :member
end

puts "Users: #{User.count}"

# ── Public API: first-party CLI OAuth client ────────────────
# Well-known public client for `campbooks login` (authorization_code + PKCE).
# Idempotent; identical client_id on every instance so one CLI binary works
# against cloud and self-hosted alike.
Api::CliApplication.ensure!
puts "Campbooks CLI OAuth client: #{Api::CliApplication::UID}"

# ── Email Tags ──────────────────────────────────────────────
EMAIL_TAGS = {
  "invoice"           => { color: "#3b82f6", prompt: "The email contains content or attachments related to an invoice (fatura, bill), or refers to the sending, receiving, or payment of an invoice. Look for invoice numbers, amounts due, tax fields (IVA/VAT), and vendor NIF." },
  "receipt"           => { color: "#f59e0b", prompt: "The email contains content or attachments related to a receipt (recibo, comprovativo de compra), or refers to a purchase or payment confirmation. Look for receipt numbers, store names, payment methods (MB Way, Multibanco, card), and smaller amounts." },
  "bank_statement"    => { color: "#8b5cf6", prompt: "The email contains content or attachments related to a bank statement (extrato bancário), or refers to account balances, transactions, or bank notifications. Look for account numbers, IBAN, opening/closing balances, bank names." },
  "financial"         => { color: "#06b6d4", prompt: "The email contains content or attachments related to taxes, accounting, fiscal matters, IRS, IVA, Social Security, or financial compliance. Or refers to the filing, payment, or discussion of such matters." },
  "insurance"         => { color: "#a855f7", prompt: "The email contains content or attachments related to insurance policies, claims, or coverage. Or refers to the purchase, renewal, or discussion of insurance. Look for seguro, apólice, cobertura, sinistro, prémio." },
  "cars"              => { color: "#eab308", prompt: "The email contains content or attachments related to vehicles, fleet management, repairs, inspections, or registration. Or refers to maintenance, purchasing, or selling of vehicles. Look for viatura, automóvel, carro, matrícula, inspeção, IPO." },
  "subscriptions"     => { color: "#14b8a6", prompt: "The email contains content or attachments related to recurring service subscriptions, renewals, or cancellations. Or refers to plan changes, billing for subscriptions. Look for subscrição, assinatura, renovação, renewal, plan, upgrade." },
  "software"          => { color: "#6366f1", prompt: "The email contains content or attachments related to software services, SaaS tools, development platforms, APIs, or tech infrastructure. Or refers to setup, configuration, or billing for such services. Look for GitHub, AWS, hosting, domain, DNS." },
  "accounting"        => { color: "#0ea5e9", prompt: "The email contains content or attachments related to accounting, bookkeeping, fiscal declarations, or tax preparation. Or refers to the filing or discussion of such matters. Look for contabilidade, contabilista, SAF-T, e-fatura, declaração." },
  "notifications"     => { color: "#64748b", prompt: "The email is an automated system notification, alert, or status update (not marketing). Sent by a system or service about account activity, changes, or events. Look for alert, notification, status, update, your account, was changed." },
  "auth"              => { color: "#f43f5e", prompt: "The email contains content related to authentication, login, password management, or account security. Or refers to sign-in attempts, password resets, 2FA/MFA codes. Look for sign in, login, password, código, verificação, autenticação." },
  "admin"             => { color: "#84cc16", prompt: "The email relates to administrative tasks, office management, logistics, scheduling, or operational coordination. Look for agendamento, reunião, meeting, office, escritório, supplies, material, correio, shipping, entrega." },
  "legal"             => { color: "#d946ef", prompt: "The email contains content or attachments related to legal matters, contracts, regulatory compliance, or official government correspondence. Look for contrato, contract, legal, advogado, lawyer, tribunal, court, licença, license." },
  "proposals"         => { color: "#fb923c", prompt: "The email contains content or attachments related to business proposals, quotes (orçamentos), bids, or project estimates. Or refers to the submission, review, or follow-up of such documents." },
  "important"         => { color: "#f97316", prompt: "The email contains content that requires urgent attention or action. Deadlines, legal notices, compliance requirements, or critical operational matters that cannot be ignored." },
  "promotional"       => { color: "#ef4444", prompt: "The email is marketing, a newsletter, promotional offer, or automated service notification with no actionable financial or operational content." },
  "personal"          => { color: "#10b981", prompt: "The email is purely personal correspondence with NO business relevance whatsoever. ONLY apply when: (1) sender is a known individual, (2) content is casual/non-professional, and (3) zero references to invoices, payments, contracts, legal, taxes, banking, or business operations. If uncertain, use a different tag." },
  "spam"              => { color: "#6b7280", prompt: "The email is unsolicited junk mail, a phishing attempt, or clearly unwanted commercial content with no relevance to the recipient." },
  "security_flagged"  => { color: "#dc2626", prompt: "The email was flagged by the security pre-screener as potentially containing sensitive information and was NOT sent to the AI for content classification." }
}.freeze

puts "Seeding email tags..."
EMAIL_TAGS.each do |name, attrs|
  Tag.find_or_create_by!(name: name, workspace: org) { |t| t.color = attrs[:color]; t.prompt = attrs[:prompt] }
end
# The four built-in default tag groups (Notifications / Newsletters & promos /
# Social / Updates) that ship with every workspace.
Tags::DefaultGroups.provision!(org)
puts "Email tags: #{Tag.count}"

# ── Document Classifications ─────────────────────────────────
company_nif = org.company_nif || "123456789"

DOCUMENT_TYPES = {
  # ── Five built-in types: canonical enriched schemas from DocumentTypes::BuiltinSchemas ──
  "expense_invoice"   => {
    category: "accounting",
    color: "#3b82f6",
    prompt: "An invoice FROM a supplier/vendor TO our company. Contains vendor details (name, NIF), invoice number, amounts, tax/IVA.",
    schema: DocumentTypes::BuiltinSchemas.for("expense_invoice")
  },
  "revenue_invoice"   => {
    category: "accounting",
    color: "#22c55e",
    prompt: "An invoice FROM our company TO a client. Our company (NIF #{company_nif}) is the seller/issuer.",
    schema: DocumentTypes::BuiltinSchemas.for("revenue_invoice")
  },
  "bank_statement"    => {
    category: "accounting",
    color: "#8b5cf6",
    prompt: "A bank statement or account statement from a financial institution.",
    schema: DocumentTypes::BuiltinSchemas.for("bank_statement")
  },
  "receipt"           => {
    category: "accounting",
    color: "#f59e0b",
    prompt: "A proof of purchase or payment receipt. Smaller amounts, informal structure.",
    schema: DocumentTypes::BuiltinSchemas.for("receipt")
  },
  "other"             => {
    category: "other",
    color: "#6b7280",
    prompt: "A document that does not fit any of the other specific categories.",
    schema: DocumentTypes::BuiltinSchemas.for("other")
  },
  # ── Custom types: enriched inline schemas with positions, money, date, and enum types ──
  "credit_note"       => {
    category: "accounting",
    color: "#ef4444",
    prompt: "A credit note (Nota de Crédito / NC) that reverses, corrects, or refunds a prior invoice. Issued by a supplier to credit an amount back; references the original invoice.",
    schema: {
      vendor_name:             { type: "string", position: 1, description: "Name of the issuer (supplier/vendor)" },
      vendor_nif:              { type: "string", position: 2, description: "9-digit NIF of the issuer" },
      credit_note_number:      { type: "string", position: 3, description: "Credit note number/identifier" },
      original_invoice_number: { type: "string", position: 4, description: "Number of the original invoice being credited/corrected" },
      amount_cents:            { type: "money",  position: 5, description: "Credited amount in cents (e.g. €123.45 = 12345)" },
      tax_amount_cents:        { type: "money",  position: 6, description: "IVA tax amount in cents" },
      tax_rate:                { type: "number", position: 7, description: "IVA rate (e.g. 23.0 for 23%)" },
      document_date:           { type: "date",   position: 8, description: "Document date YYYY-MM-DD or null" },
      currency:                { type: "string", position: 9, description: "Currency code, default EUR" }
    }
  },
  "insurance_policy"  => {
    category: "insurance",
    color: "#a855f7",
    prompt: "An insurance policy document, certificate of insurance, coverage summary, or insurance claim form.",
    schema: {
      insurer_name:  { type: "string", position: 1, description: "Name of the insurance company" },
      policy_number: { type: "string", position: 2, description: "Policy/apólice number" },
      insured_party: { type: "string", position: 3, description: "Name of the insured person or entity" },
      coverage_type: { type: "string", position: 4, description: "Type of coverage (auto, liability, property, health)" },
      premium_cents: { type: "money",  position: 5, description: "Insurance premium amount in cents" },
      document_date: { type: "date",   position: 6, description: "Document date YYYY-MM-DD or null" },
      currency:      { type: "string", position: 7, description: "Currency code, default EUR" }
    }
  },
  "vehicle_document"  => {
    category: "vehicles",
    color: "#eab308",
    prompt: "A vehicle-related document: registration, inspection report (IPO), title, insurance green card, or vehicle tax document.",
    schema: {
      plate_number:     { type: "string", position: 1, description: "License plate (matrícula) if present" },
      vehicle_make:     { type: "string", position: 2, description: "Vehicle brand/make (e.g. Mercedes-Benz)" },
      vehicle_model:    { type: "string", position: 3, description: "Vehicle model" },
      vin:              { type: "string", position: 4, description: "VIN/chassis number if present" },
      document_subtype: { type: "string", position: 5, description: "Specific sub-type (registration, IPO inspection, IUC tax, green card)" },
      document_date:    { type: "date",   position: 6, description: "Document date YYYY-MM-DD or null" }
    }
  },
  "contract"          => {
    category: "legal",
    color: "#d946ef",
    prompt: "A legal contract, agreement, or terms and conditions between parties.",
    schema: {
      counterparty:  { type: "string", position: 1, description: "Name of the other party to the contract" },
      contract_type: { type: "string", position: 2, description: "Type of contract (lease, service, employment, sale)" },
      effective_date: { type: "date",  position: 3, description: "Contract effective/start date YYYY-MM-DD" },
      expiry_date:   { type: "date",   position: 4, description: "Contract expiry/end date YYYY-MM-DD if present" },
      amount_cents:  { type: "money",  position: 5, description: "Contract value/consideration in cents if present" },
      document_date: { type: "date",   position: 6, description: "Document date YYYY-MM-DD or null" },
      currency:      { type: "string", position: 7, description: "Currency code, default EUR" }
    }
  },
  "certificate"       => {
    category: "identification",
    color: "#06b6d4",
    prompt: "An official certificate, license, permit, or registration issued by an authority.",
    schema: {
      issuing_authority:  { type: "string", position: 1, description: "Authority that issued the certificate" },
      certificate_type:   { type: "string", position: 2, description: "Type of certificate (conformity, training, registration, license)" },
      certificate_number: { type: "string", position: 3, description: "Certificate identifier/number" },
      subject_name:       { type: "string", position: 4, description: "Name of the person/entity the certificate is about" },
      issue_date:         { type: "date",   position: 5, description: "Date of issuance YYYY-MM-DD" },
      expiry_date:        { type: "date",   position: 6, description: "Expiry date YYYY-MM-DD if present" },
      document_date:      { type: "date",   position: 7, description: "Document date YYYY-MM-DD or null" }
    }
  },
  "tax_document"      => {
    category: "accounting",
    color: "#f97316",
    prompt: "A tax authority document: IRS, IRC, IVA, tax assessment, payment slip, or fiscal declaration.",
    schema: {
      tax_type:      { type: "string", position: 1, description: "Type of tax (IRS, IRC, IVA, IMI, etc.)" },
      tax_period:    { type: "string", position: 2, description: "Tax period (e.g. 2025)" },
      taxpayer_nif:  { type: "string", position: 3, description: "NIF of the taxpayer" },
      amount_cents:  { type: "money",  position: 4, description: "Tax amount in cents" },
      document_date: { type: "date",   position: 5, description: "Document date YYYY-MM-DD or null" },
      currency:      { type: "string", position: 6, description: "Currency code, default EUR" }
    }
  },
  "identification"    => {
    category: "identification",
    color: "#f43f5e",
    prompt: "A personal or company identification document: citizen card, passport, NIF card, company registration certificate.",
    schema: {
      id_type:       { type: "string", position: 1, description: "Type of ID (citizen_card, passport, nif_card, company_registration)" },
      person_name:   { type: "string", position: 2, description: "Full name of the person or company name" },
      id_number:     { type: "string", position: 3, description: "Document number (passport number, citizen card number, NIPC)" },
      nif:           { type: "string", position: 4, description: "NIF if present on the document" },
      issue_date:    { type: "date",   position: 5, description: "Date of issuance YYYY-MM-DD" },
      expiry_date:   { type: "date",   position: 6, description: "Expiry date YYYY-MM-DD if present" },
      document_date: { type: "date",   position: 7, description: "Document date YYYY-MM-DD or null" }
    }
  },
  "proposal"          => {
    category: "legal",
    color: "#fb923c",
    prompt: "A business proposal, quote (orçamento), bid, or project estimate.",
    schema: {
      proposer_name:  { type: "string", position: 1, description: "Name of the entity making the proposal" },
      proposal_type:  { type: "string", position: 2, description: "Type (quote, bid, estimate, proposal)" },
      amount_cents:   { type: "money",  position: 3, description: "Total proposed amount in cents" },
      validity_date:  { type: "date",   position: 4, description: "Proposal validity date YYYY-MM-DD if present" },
      scope_summary:  { type: "string", position: 5, description: "Brief summary of what the proposal covers" },
      document_date:  { type: "date",   position: 6, description: "Document date YYYY-MM-DD or null" },
      currency:       { type: "string", position: 7, description: "Currency code, default EUR" }
    }
  },
  "correspondence"    => {
    category: "correspondence",
    color: "#64748b",
    prompt: "General correspondence, letters, or communications. Not a structured formal document.",
    schema: {
      sender:        { type: "string", position: 1, description: "Name of the sender/organization" },
      recipient:     { type: "string", position: 2, description: "Name of the recipient" },
      subject:       { type: "string", position: 3, description: "Subject/topic of the correspondence" },
      document_date: { type: "date",   position: 4, description: "Document date YYYY-MM-DD or null" }
    }
  }
}.freeze

puts "Seeding document classifications..."
DOCUMENT_TYPES.each do |name, attrs|
  DocumentType.find_or_create_by!(name: name, workspace: org) do |t|
    t.category = attrs[:category]
    t.color = attrs[:color]
    t.prompt = attrs[:prompt]
    t.extraction_schema = attrs[:schema]
  end
  # Update category on existing records (find_or_create_by! only sets attrs on create)
  DocumentType.where(name: name, workspace: org).update_all(category: attrs[:category])
end
# Remove any stale types not in the seed list for this org
DocumentType.where(workspace: org).where.not(name: DOCUMENT_TYPES.keys).destroy_all
puts "Document types: #{DocumentType.count}"

# ── Email Account Access ─────────────────────────────────────
puts "Granting seed users access to email accounts..."
seed_users = User.all.to_a
EmailAccount.find_each do |account|
  account.update_column(:workspace_id, org.id) if account.workspace_id.nil?
  seed_users.each do |user|
    account.email_account_users.find_or_create_by!(user: user) do |entry|
      entry.owner = true
      entry.can_read = true
      entry.can_send = true
      entry.can_manage = true
    end
  end
end
puts "Email account users: #{EmailAccountUser.count}"

# ── Template ─────────────────────────────────────────────────
puts "Creating default template..."
Template.find_or_create_by!(name: "Business Default") do |t|
  t.description = "Default setup for document types, email tags, and AI configuration. Designed for a Portuguese consulting or financial management business, but applies to most small and medium companies."
  t.data = {
    tags: EMAIL_TAGS.map { |name, attrs| { name: name, color: attrs[:color], prompt: attrs[:prompt] } },
    document_types: DOCUMENT_TYPES.map { |name, attrs|
      entry = { name: name, category: attrs[:category], color: attrs[:color], prompt: attrs[:prompt], extraction_schema: attrs[:schema] }
      # Generalize the template prompt — replace seed-specific NIF with a placeholder
      entry[:prompt] = entry[:prompt].sub(company_nif, "YOUR_COMPANY_NIF")
      entry
    },
    ai_adapters: [
      { name: "DeepSeek Chat", provider: "deepseek" },
      { name: "OpenAI Vision", provider: "openai" }
    ],
    ai_configurations: [
      { purpose: "document_analysis",   ai_adapter_name: "OpenAI Vision", model: "gpt-4o-mini", max_tokens: 4000, temperature: 0.0,
        system_prompt: "You are a document analysis assistant for a financial consulting firm. Extract structured data from the provided document image or PDF. Identify the document type, vendor/client names, amounts, dates, tax IDs (NIF), invoice numbers, and any other relevant financial information. Be precise with numbers and currency values. If you're uncertain about any field, note your confidence level." },
      { purpose: "email_classification", ai_adapter_name: "DeepSeek Chat", model: "deepseek-v4-pro", max_tokens: 4000, temperature: 0.0,
        system_prompt: "You classify incoming emails into tags and document types. Analyze the email subject and body to determine the most appropriate classification. Consider the sender, the content, and the context. Use the workspace's existing tags and document types as your classification target. If an email could fit multiple categories, pick the most specific one." },
      { purpose: "email_analysis",      ai_adapter_name: "DeepSeek Chat", model: "deepseek-v4-pro", max_tokens: 4000, temperature: 0.0,
        system_prompt: "You analyze email content to extract actionable insights. Identify key information: who is sending, what they want, deadlines mentioned, amounts or financial figures, and any requests that need a response. Flag urgent items, legal or tax implications, and follow-ups needed. Summarize the email in 1-2 sentences." },
      { purpose: "reminder_extraction", ai_adapter_name: "DeepSeek Chat", model: "deepseek-v4-pro", max_tokens: 4000, temperature: 0.0,
        system_prompt: "Extract concrete dated commitments — payment due dates, deliveries, deadlines, renewals, appointments, trips, events — the reader needs on their calendar. Resolve relative dates to absolute ones. Ignore promotional or marketing dates." },
      { purpose: "email_chat",          ai_adapter_name: "DeepSeek Chat", model: "deepseek-v4-pro", max_tokens: 4000, temperature: 0.0,
        system_prompt: "You are Scout, an AI assistant helping a consultant manage their email. You have access to the current email thread and the workspace's context. Answer questions about the email content conversationally. Help the user understand the email, draft responses, identify action items, or find related information. Be concise and direct — the user is busy." },
      { purpose: "draft_reply",         ai_adapter_name: "DeepSeek Chat", model: "deepseek-v4-pro", max_tokens: 4000, temperature: 0.0,
        system_prompt: "You draft professional email replies in the user's voice. Match the tone of the original email — formal for business correspondence, casual for familiar contacts. Keep replies concise and actionable. Include relevant context from previous emails in the thread. If a decision or action is needed, state it clearly. Default to Portuguese if the original email is in Portuguese, otherwise use the same language as the original." },
      { purpose: "global_chat",         ai_adapter_name: "DeepSeek Chat", model: "deepseek-v4-pro", max_tokens: 4000, temperature: 0.0,
        system_prompt: "You are Scout, an AI assistant for a financial consulting firm. You help with document management, email processing, financial analysis, and business operations. You have access to the workspace's documents, emails, contacts, and financial data. Answer questions conversationally, provide insights from the data available to you, and help the user make informed decisions. Be proactive — if you notice something important, mention it." }
    ]
  }
end
puts "Templates: #{Template.count}"

# ── AI Adapters for Demo Org ────────────────────────────────────
puts "Creating default AI adapters..."
text_adapter = org.ai_adapters.find_or_create_by!(name: "DeepSeek Chat") do |a|
  a.provider = "deepseek"
  a.enabled = true
end

docs_adapter = org.ai_adapters.find_or_create_by!(name: "OpenAI Vision") do |a|
  a.provider = "openai"
  a.enabled = true
end

puts "Assigning adapters to services..."

DEFAULT_SYSTEM_PROMPTS = {
  document_analysis:   "Extract structured data from documents. Identify document type, vendor/client names, amounts, dates, tax IDs (NIF), invoice numbers, and relevant financial information. Use the exact field names from the extraction schema. For amounts, use integer cents. Be precise with numbers.",
  email_classification: "Classify incoming emails into tags. Analyze subject and body to determine the most appropriate classification. Assign 1–3 tags that genuinely match the content. If nothing fits, return an empty list. Consider the sender, content, and context.",
  email_analysis:      "Analyze email content to extract actionable insights. Identify who sent it, what they want, deadlines, amounts, and requests needing a response. Flag urgent items, legal or tax implications, and follow-ups. Summarize in 1–2 sentences from the recipient's perspective.",
  reminder_extraction: "Extract concrete dated commitments — payment due dates, deliveries, deadlines, renewals, appointments, trips, events — the reader needs on their calendar. Resolve relative dates to absolute ones. Ignore promotional or marketing dates.",
  email_chat:          "You are Scout, helping the user manage their email. Answer questions about the email thread conversationally. Help the user understand emails, draft responses, identify action items, or find related information. Be concise and direct. Match tool suggestions to the user's exact request.",
  draft_reply:         "Draft professional email replies in the user's voice. Match the tone of the original email. Keep replies concise and actionable. Default to Portuguese if the original email is in Portuguese. If you need specific values not provided, use {{variable_name}} placeholders.",
  global_chat:         "You are Scout, an AI assistant for a financial consulting firm. You help with document management, email processing, financial analysis, and business operations. Answer questions conversationally, provide insights from available data, and help the user make informed decisions."
}.freeze

AiConfiguration::PURPOSES.each do |purpose|
  adapter = purpose == "document_analysis" ? docs_adapter : text_adapter
  default_model = AiConfiguration::DEFAULT_MODEL[adapter.provider] || "gpt-4o-mini"
  config = org.ai_configurations.find_or_create_by!(purpose: purpose) do |c|
    c.ai_adapter = adapter
    c.enabled = true
    c.model = default_model
    c.max_tokens = 4000
    c.temperature = 0.0
  end
  config.update(system_prompt: DEFAULT_SYSTEM_PROMPTS[purpose.to_sym]) if config.system_prompt.blank?
end
puts "AI adapters: #{org.ai_adapters.count}, assignments: #{org.ai_configurations.count}"

# ── Accounting Demo Data ─────────────────────────────────────
puts "Seeding accounting demo data..."

admin_user = User.find_by(email_address: "admin@example.com")

if admin_user && !Reconciliation.exists?(workspace: org)
  expense_type  = DocumentType.find_by(name: "expense_invoice", workspace: org)
  revenue_type  = DocumentType.find_by(name: "revenue_invoice",  workspace: org)
  statement_type = DocumentType.find_by(name: "bank_statement",  workspace: org)
  company_nif   = org.company_nif || "123456789"

  # Helper: build a Document with file attached before save (bypasses on: :create validation)
  def seed_document(workspace, attrs, filename:, content:, content_type:)
    doc = workspace.documents.build(attrs)
    doc.original_file.attach(
      io: StringIO.new(content),
      filename: filename,
      content_type: content_type
    )
    doc.save!
    doc
  end

  # ── Invoice documents (expense)
  vodafone_doc = seed_document(org,
    { document_type: :expense_invoice, document_type_id: expense_type&.id,
      ai_status: :completed, review_status: :approved, source: :manual_upload,
      vendor_name: "Vodafone Portugal S.A.", vendor_nif: "502626750",
      buyer_nif: company_nif, invoice_number: "FT2024/00045",
      amount_cents: 4590, tax_amount_cents: 888, tax_rate: 23.0,
      document_date: Date.new(2024, 1, 3), currency: "EUR", company_vat_present: true },
    filename: "vodafone_jan2024.pdf",
    content: "Vodafone Portugal - Fatura FT2024/00045",
    content_type: "application/pdf"
  )

  edp_doc = seed_document(org,
    { document_type: :expense_invoice, document_type_id: expense_type&.id,
      ai_status: :completed, review_status: :approved, source: :manual_upload,
      vendor_name: "EDP - Energias de Portugal S.A.", vendor_nif: "500697256",
      buyer_nif: company_nif, invoice_number: "FT2024/00112",
      amount_cents: 11207, tax_amount_cents: 2166, tax_rate: 23.0,
      document_date: Date.new(2024, 1, 8), currency: "EUR", company_vat_present: true },
    filename: "edp_jan2024.pdf",
    content: "EDP Energia - Fatura FT2024/00112",
    content_type: "application/pdf"
  )

  staples_doc = seed_document(org,
    { document_type: :expense_invoice, document_type_id: expense_type&.id,
      ai_status: :completed, review_status: :pending, source: :manual_upload,
      vendor_name: "Staples Portugal, Lda.", vendor_nif: "503415040",
      buyer_nif: nil, # intentionally missing to demo NIF flag
      invoice_number: "FT2024/00221",
      amount_cents: 8432, tax_amount_cents: 1629, tax_rate: 23.0,
      document_date: Date.new(2024, 1, 12), currency: "EUR", company_vat_present: false },
    filename: "staples_jan2024.pdf",
    content: "Staples Portugal - Fatura FT2024/00221",
    content_type: "application/pdf"
  )

  # ── Revenue invoice (client paid us)
  revenue_doc = seed_document(org,
    { document_type: :revenue_invoice, document_type_id: revenue_type&.id,
      ai_status: :completed, review_status: :approved, source: :manual_upload,
      client_name: "Acme Consulting Lda.", client_nif: "509876543",
      invoice_number: "FR2024/00008",
      amount_cents: 150000, tax_amount_cents: 28980, tax_rate: 23.0,
      document_date: Date.new(2024, 1, 15), currency: "EUR", company_vat_present: true },
    filename: "revenue_jan2024.pdf",
    content: "Demo Corp - Fatura FR2024/00008",
    content_type: "application/pdf"
  )

  # ── Bank statement document (the CSV the reconciliation is based on)
  statement_csv = <<~CSV
    Date,Description,Counterparty,Amount,Balance
    2024-01-05,Debito direto telecomunicacoes,VODAFONE PORTUGAL S.A.,-45.90,8454.10
    2024-01-10,Energia eletrica janeiro,EDP ENERGIAS DE PORTUGAL,-112.07,8342.03
    2024-01-14,Material de escritorio,STAPLES PORTUGAL LDA,-84.32,8257.71
    2024-01-16,Transferencia recebida Acme,ACME CONSULTING LDA,1500.00,9757.71
    2024-01-20,Combustivel frota,REPSOL PORTUGUESA S.A.,-60.00,9697.71
    2024-01-25,Comissao manutencao conta,MILLENNIUM BCP,-3.50,9694.21
    2024-01-28,Pagamento fornecedor,DISTRIBUIDORA NORTE LDA,-200.00,9494.21
  CSV

  statement_doc = seed_document(org,
    { document_type: :bank_statement, document_type_id: statement_type&.id,
      ai_status: :completed, review_status: :approved, source: :manual_upload,
      bank_name: "Millennium BCP",
      period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31),
      opening_balance_cents: 850000, closing_balance_cents: 949421,
      document_date: Date.new(2024, 1, 31), currency: "EUR", company_vat_present: false },
    filename: "millennium_jan2024.csv",
    content: statement_csv,
    content_type: "text/csv"
  )

  # ── Reconciliation
  recon = Reconciliation.create!(
    workspace: org,
    created_by: admin_user,
    statement_document: statement_doc,
    bank_name: "Millennium BCP",
    currency: "EUR",
    period_start: Date.new(2024, 1, 1),
    period_end: Date.new(2024, 1, 31),
    opening_balance_cents: 850000,
    closing_balance_cents: 949421,
    status: :ready,
    export_status: :export_none
  )

  # ── Bank transactions
  def create_tx(recon, position:, booked_on:, description:, counterparty:, amount_cents:, balance_after_cents:, status: :unmatched)
    BankTransaction.create!(
      reconciliation: recon,
      workspace: recon.workspace,
      position: position,
      booked_on: booked_on,
      description: description,
      counterparty: counterparty,
      amount_cents: amount_cents,
      balance_after_cents: balance_after_cents,
      currency: recon.currency,
      status: status
    )
  end

  tx1 = create_tx(recon, position: 1, booked_on: Date.new(2024, 1, 5),
    description: "Debito direto telecomunicacoes", counterparty: "VODAFONE PORTUGAL S.A.",
    amount_cents: -4590, balance_after_cents: 845410, status: :matched)

  tx2 = create_tx(recon, position: 2, booked_on: Date.new(2024, 1, 10),
    description: "Energia eletrica janeiro", counterparty: "EDP ENERGIAS DE PORTUGAL",
    amount_cents: -11207, balance_after_cents: 834203, status: :matched)

  tx3 = create_tx(recon, position: 3, booked_on: Date.new(2024, 1, 14),
    description: "Material de escritorio", counterparty: "STAPLES PORTUGAL LDA",
    amount_cents: -8432, balance_after_cents: 825771, status: :suggested)

  tx4 = create_tx(recon, position: 4, booked_on: Date.new(2024, 1, 16),
    description: "Transferencia recebida Acme", counterparty: "ACME CONSULTING LDA",
    amount_cents: 150000, balance_after_cents: 975771, status: :matched)

  tx5 = create_tx(recon, position: 5, booked_on: Date.new(2024, 1, 20),
    description: "Combustivel frota", counterparty: "REPSOL PORTUGUESA S.A.",
    amount_cents: -6000, balance_after_cents: 969771, status: :requested)
  tx5.update_columns(requested_at: 2.days.ago, requested_by_id: admin_user.id)

  _tx6 = create_tx(recon, position: 6, booked_on: Date.new(2024, 1, 25),
    description: "Comissao manutencao conta", counterparty: "MILLENNIUM BCP",
    amount_cents: -350, balance_after_cents: 969421, status: :excluded)
  _tx6.update_columns(exclusion_reason: "bank_fee")

  _tx7 = create_tx(recon, position: 7, booked_on: Date.new(2024, 1, 28),
    description: "Pagamento fornecedor", counterparty: "DISTRIBUIDORA NORTE LDA",
    amount_cents: -20000, balance_after_cents: 949421, status: :unmatched)

  # ── Matches
  TransactionMatch.create!(
    bank_transaction: tx1, document: vodafone_doc,
    status: :confirmed, matched_by: :ai,
    confidence: 0.97,
    match_reasons: { "amount_exact" => true, "date_diff_days" => 2, "name_similarity" => 88 }
  )
  TransactionMatch.create!(
    bank_transaction: tx2, document: edp_doc,
    status: :confirmed, matched_by: :ai,
    confidence: 0.95,
    match_reasons: { "amount_exact" => true, "date_diff_days" => 2, "name_similarity" => 79 }
  )
  TransactionMatch.create!(
    bank_transaction: tx3, document: staples_doc,
    status: :suggested, matched_by: :ai,
    confidence: 0.78,
    match_reasons: { "amount_exact" => true, "date_diff_days" => 2, "name_similarity" => 72 }
  )
  TransactionMatch.create!(
    bank_transaction: tx4, document: revenue_doc,
    status: :confirmed, matched_by: :ai,
    confidence: 0.99,
    match_reasons: { "amount_exact" => true, "date_diff_days" => 1, "name_similarity" => 91 }
  )

  puts "Accounting: reconciliation seeded (#{recon.id})"
else
  puts "Accounting: reconciliation already seeded — skipping"
end

puts "Done!"
