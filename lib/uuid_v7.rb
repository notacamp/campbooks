# frozen_string_literal: true

module UuidV7
  def self.generate
    timestamp_ms = (Time.now.to_f * 1000).to_i
    bytes = [
      (timestamp_ms >> 40) & 0xFF, (timestamp_ms >> 32) & 0xFF,
      (timestamp_ms >> 24) & 0xFF, (timestamp_ms >> 16) & 0xFF,
      (timestamp_ms >> 8) & 0xFF, timestamp_ms & 0xFF
    ].pack("C*") + SecureRandom.random_bytes(10)
    bytes.setbyte(6, (bytes.getbyte(6) & 0x0F) | 0x70)
    bytes.setbyte(8, (bytes.getbyte(8) & 0x3F) | 0x80)
    hex = bytes.unpack1("H*")
    "#{hex[0..7]}-#{hex[8..11]}-#{hex[12..15]}-#{hex[16..19]}-#{hex[20..31]}"
  end
end
