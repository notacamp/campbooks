class EmailChunker
  MAX_CHUNK_TOKENS = 2000

  # Common patterns that indicate a reply boundary in email threads
  REPLY_HEADER_PATTERNS = [
    /\A-{3,}\s*Original Message\s*-{3,}/i,
    /\A-{3,}\s*Forwarded Message\s*-{3,}/i,
    /\AOn\s+\w+,\s+\w+\s+\d+,\s+\d{4}.*wrote:/i,
    /\AOn\s+\d{1,2}\/\d{1,2}\/\d{2,4}.*wrote:/i,
    /\AFrom:\s+/i,
    /\ABegin forwarded message:/i,
    /\A-{2,}\s*Forwarded message\s*-{2,}/i
  ].freeze

  # Lines that consist primarily of "> " prefixed quoted text
  QUOTED_LINE = /\A(>{1,})\s?/.freeze

  def initialize(email_message)
    @email = email_message
  end

  # Returns an array of chunk hashes:
  # [{ content:, chunk_type:, position:, metadata: { sender:, timestamp:, is_quoted:, message_index: } }]
  def chunk
    messages = split_into_messages(plain_text_body)
    return [ default_chunk ] if messages.empty?

    chunks = []
    messages.each_with_index do |msg, idx|
      msg_chunks = chunk_message(msg, idx)
      chunks.concat(msg_chunks)
    end

    # Also chunk the subject line as metadata context
    if @email.subject.present?
      chunks.unshift(
        content: "Subject: #{@email.subject}",
        chunk_type: "thread_header",
        position: -1,
        metadata: {
          subject: @email.subject,
          from: @email.from_address,
          received_at: @email.received_at&.iso8601
        }
      )
    end

    enforce_token_budget(chunks).each_with_index.map { |c, i| c.merge(position: i) }
  end

  private

  # Embed readable text, not raw markup. An HTML email body is one break-less blob
  # that the paragraph splitter can't divide and that bloats the token count — a
  # single body was producing a 27k-token chunk that OpenAI's embeddings endpoint
  # 400s on (its limit is 8191). Plain-text bodies have no tags and pass through.
  def plain_text_body
    raw = @email.body.to_s
    CGI.unescapeHTML(ActionController::Base.helpers.strip_tags(raw)).squish
  end

  # Final guarantee that every emitted chunk fits the embedding budget, even a
  # single paragraph with no breaks for #split_long_message to split on (raw HTML,
  # a giant JSON blob). Splits an over-budget chunk into bounded pieces.
  def enforce_token_budget(chunks)
    chunks.flat_map do |c|
      pieces = bounded_pieces(c[:content].to_s)
      next [ c ] if pieces.size <= 1

      pieces.map.with_index do |content, i|
        c.merge(content: content, metadata: (c[:metadata] || {}).merge(split_index: i))
      end
    end
  end

  # Split text into <= MAX_CHUNK_TOKENS pieces: at sentence/line boundaries first,
  # then by hard character windows for a token-dense run with no boundaries at all.
  def bounded_pieces(text)
    return [ text ] if estimate_tokens(text) <= MAX_CHUNK_TOKENS

    max_chars = (MAX_CHUNK_TOKENS * 3.5).floor
    pieces = []
    buffer = +""

    text.split(/(?<=[.!?])\s+|\n+/).each do |part|
      if estimate_tokens(part) > MAX_CHUNK_TOKENS
        pieces << buffer.strip unless buffer.strip.empty?
        buffer = +""
        part.scan(/.{1,#{max_chars}}/m).each { |window| pieces << window.strip }
      elsif estimate_tokens(buffer + part) > MAX_CHUNK_TOKENS && !buffer.empty?
        pieces << buffer.strip
        buffer = +"#{part} "
      else
        buffer << part << " "
      end
    end
    pieces << buffer.strip unless buffer.strip.empty?
    pieces.reject(&:blank?)
  end

  def split_into_messages(body)
    return [] if body.blank?

    lines = body.lines.map(&:chomp)
    messages = []
    current_lines = []

    lines.each do |line|
      if reply_boundary?(line) && current_lines.any?
        messages << clean_quoted_text(current_lines.join("\n"))
        current_lines = []
      end
      current_lines << line
    end

    messages << clean_quoted_text(current_lines.join("\n")) if current_lines.any?
    messages
  end

  def reply_boundary?(line)
    REPLY_HEADER_PATTERNS.any? { |pattern| line.strip.match?(pattern) }
  end

  def clean_quoted_text(text)
    lines = text.lines.map(&:chomp)
    cleaned = lines.map { |line| line.sub(QUOTED_LINE, "").strip }
    cleaned.reject(&:blank?).join("\n")
  end

  def chunk_message(text, message_index)
    is_quoted = message_index > 0 # first message is original, rest are quoted replies

    # Extract sender info from message header if present
    sender = extract_sender(text)

    # Split long messages at paragraph boundaries
    if estimate_tokens(text) > MAX_CHUNK_TOKENS
      split_long_message(text, message_index, sender, is_quoted)
    else
      [ {
        content: text.strip,
        chunk_type: "email_message",
        metadata: {
          sender: sender,
          is_quoted: is_quoted,
          message_index: message_index,
          received_at: @email.received_at&.iso8601
        }
      } ]
    end
  end

  def split_long_message(text, message_index, sender, is_quoted)
    paragraphs = text.split(/\n\n+/)
    chunks = []
    current = ""

    paragraphs.each do |para|
      if estimate_tokens(current + para) > MAX_CHUNK_TOKENS && current.present?
        chunks << current.strip
        current = para
      else
        current += (current.empty? ? "" : "\n\n") + para
      end
    end
    chunks << current.strip if current.present?

    chunks.map.with_index do |content, i|
      {
        content: content,
        chunk_type: "email_message",
        metadata: {
          sender: sender,
          is_quoted: is_quoted,
          message_index: message_index,
          paragraph_index: i,
          received_at: @email.received_at&.iso8601
        }
      }
    end
  end

  def extract_sender(text)
    match = text.match(/\AFrom:\s*(.+)/i)
    return nil unless match
    match[1].strip
  end

  def estimate_tokens(text)
    return 0 if text.blank?
    (text.length / 3.5).ceil
  end

  def default_chunk
    content = [ @email.subject, @email.ai_summary, plain_text_body.presence ].compact.join("\n\n")
    [ {
      content: content.strip,
      chunk_type: "email_message",
      metadata: {
        from: @email.from_address,
        received_at: @email.received_at&.iso8601
      }
    } ]
  end
end
